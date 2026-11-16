//! Adapter from a platform-native link (Kotlin `BluetoothGatt`, Swift CoreBluetooth) to
//! [`Transport`] (ADR-0006). Free of UniFFI so the shells can wrap it as they like.

use std::fmt;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, Sender, channel};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use crate::gatt::BLE_PACKET_GAP_MS;
use crate::{Discovered, Transport, TransportError, TransportInfo, recv_while_open};

/// Native side of a connected link. Calls may come from any thread.
pub trait NativeLink: Send + Sync {
    /// Write one chunk (at most [`Self::mtu`] bytes). Errors are reported as text.
    fn write(&self, bytes: Vec<u8>) -> Result<(), String>;
    /// Maximum write payload in bytes (for BLE: negotiated ATT MTU − 3).
    fn mtu(&self) -> u32;
    /// Tear the link down. Called once by [`Transport::close`].
    fn close(&self);
}

/// Platform BLE stack that discovers printers and opens [`NativeLink`]s (Android
/// `BluetoothGatt`, iOS CoreBluetooth). The device hub uses it for `ble:` addresses when set.
pub trait NativeBleHost: Send + Sync {
    /// Scan for up to `timeout`, keeping devices whose advertised name starts with one of
    /// `name_prefixes` (all devices when empty). `info.kind` should be [`crate::TransportKind::Ble`].
    fn scan(
        &self,
        timeout: Duration,
        name_prefixes: &[String],
    ) -> Result<Vec<Discovered>, TransportError>;

    /// Connect to `address` (without the `ble:` scheme). Notifications and the disconnect
    /// event must go to `sink`, which exists before the link so nothing is lost.
    /// Returns the link and the device's advertised name.
    fn connect(
        &self,
        address: &str,
        sink: NotifySink,
    ) -> Result<(Arc<dyn NativeLink>, String), TransportError>;
}

/// Handle the native side uses to feed received bytes into Rust.
#[derive(Clone)]
pub struct NotifySink {
    tx: Arc<Mutex<Sender<Vec<u8>>>>,
    open: Arc<AtomicBool>,
}

impl NotifySink {
    /// Deliver received bytes (one notification). Ignored after disconnect.
    pub fn push(&self, bytes: Vec<u8>) {
        if !self.open.load(Ordering::Acquire) {
            return;
        }
        if let Ok(tx) = self.tx.lock() {
            // A send error only means the transport was dropped; nothing to do.
            let _ = tx.send(bytes);
        }
    }

    /// Report that the link dropped; subsequent transport calls return `Disconnected`.
    pub fn disconnected(&self) {
        self.open.store(false, Ordering::Release);
    }
}

impl fmt::Debug for NotifySink {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("NotifySink")
            .field("open", &self.open.load(Ordering::Relaxed))
            .finish()
    }
}

/// [`Transport`] over a [`NativeLink`].
pub struct CallbackTransport {
    info: TransportInfo,
    link: Arc<dyn NativeLink>,
    rx: Receiver<Vec<u8>>,
    open: Arc<AtomicBool>,
    closed: bool,
    packet_gap: Duration,
}

impl CallbackTransport {
    /// Wrap `link`. The returned sink must be handed to the native side for notifications.
    /// The packet gap defaults to the BLE value (10 ms).
    pub fn new(info: TransportInfo, link: Arc<dyn NativeLink>) -> (Self, NotifySink) {
        let (sink, rx, open) = Self::channel();
        (Self::from_parts(info, link, rx, open), sink)
    }

    /// Create the notification sink first, hand it to `connect`, then wrap the link it
    /// returns. Use this when notifications may arrive while the link is still opening.
    pub fn connect_with<E>(
        info: TransportInfo,
        connect: impl FnOnce(NotifySink) -> Result<Arc<dyn NativeLink>, E>,
    ) -> Result<Self, E> {
        let (sink, rx, open) = Self::channel();
        let link = connect(sink)?;
        Ok(Self::from_parts(info, link, rx, open))
    }

    fn channel() -> (NotifySink, Receiver<Vec<u8>>, Arc<AtomicBool>) {
        let (tx, rx) = channel();
        let open = Arc::new(AtomicBool::new(true));
        let sink = NotifySink {
            tx: Arc::new(Mutex::new(tx)),
            open: Arc::clone(&open),
        };
        (sink, rx, open)
    }

    fn from_parts(
        info: TransportInfo,
        link: Arc<dyn NativeLink>,
        rx: Receiver<Vec<u8>>,
        open: Arc<AtomicBool>,
    ) -> Self {
        Self {
            info,
            link,
            rx,
            open,
            closed: false,
            packet_gap: Duration::from_millis(BLE_PACKET_GAP_MS),
        }
    }

    /// Override the recommended inter-packet gap (e.g. zero for a native USB link).
    pub fn with_packet_gap(mut self, gap: Duration) -> Self {
        self.packet_gap = gap;
        self
    }

    fn is_open(&self) -> bool {
        self.open.load(Ordering::Acquire)
    }
}

impl fmt::Debug for CallbackTransport {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CallbackTransport")
            .field("info", &self.info)
            .field("open", &self.is_open())
            .field("packet_gap", &self.packet_gap)
            .finish_non_exhaustive()
    }
}

impl Transport for CallbackTransport {
    fn send(&mut self, bytes: &[u8]) -> Result<(), TransportError> {
        if !self.is_open() {
            return Err(TransportError::Disconnected);
        }
        for chunk in bytes.chunks(self.mtu()) {
            self.link
                .write(chunk.to_vec())
                .map_err(TransportError::Other)?;
        }
        Ok(())
    }

    fn recv(&mut self, timeout: Duration) -> Result<Option<Vec<u8>>, TransportError> {
        recv_while_open(&self.rx, &self.open, timeout)
    }

    fn mtu(&self) -> usize {
        usize::try_from(self.link.mtu())
            .unwrap_or(usize::MAX)
            .max(1)
    }

    fn packet_gap(&self) -> Duration {
        self.packet_gap
    }

    fn info(&self) -> &TransportInfo {
        &self.info
    }

    fn close(&mut self) {
        self.open.store(false, Ordering::Release);
        if !self.closed {
            self.closed = true;
            self.link.close();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::TransportKind;

    #[derive(Default)]
    struct FakeLink {
        writes: Mutex<Vec<Vec<u8>>>,
        closed: AtomicBool,
        fail: AtomicBool,
    }

    impl NativeLink for FakeLink {
        fn write(&self, bytes: Vec<u8>) -> Result<(), String> {
            if self.fail.load(Ordering::Relaxed) {
                return Err("gatt busy".into());
            }
            self.writes.lock().unwrap().push(bytes);
            Ok(())
        }
        fn mtu(&self) -> u32 {
            3
        }
        fn close(&self) {
            self.closed.store(true, Ordering::Relaxed);
        }
    }

    fn setup() -> (CallbackTransport, NotifySink, Arc<FakeLink>) {
        let link = Arc::new(FakeLink::default());
        let info = TransportInfo {
            kind: TransportKind::Callback,
            address: "dev1".into(),
            name: "B21 Pro".into(),
        };
        let (t, sink) = CallbackTransport::new(info, link.clone());
        (t, sink, link)
    }

    #[test]
    fn chunks_and_receives() {
        let (mut t, sink, link) = setup();
        assert_eq!(t.info().uri(), "cb:dev1");
        assert_eq!(t.mtu(), 3);
        assert_eq!(t.packet_gap(), Duration::from_millis(10));
        t.send(&[1, 2, 3, 4, 5]).unwrap();
        assert_eq!(
            *link.writes.lock().unwrap(),
            vec![vec![1, 2, 3], vec![4, 5]]
        );

        let s2 = sink.clone();
        std::thread::spawn(move || s2.push(vec![7, 8]))
            .join()
            .unwrap();
        assert_eq!(t.recv(Duration::from_millis(50)).unwrap(), Some(vec![7, 8]));
        assert_eq!(t.recv(Duration::from_millis(5)).unwrap(), None);
        assert_eq!(t.recv(Duration::ZERO).unwrap(), None);
    }

    #[test]
    fn write_error_is_reported() {
        let (mut t, _sink, link) = setup();
        link.fail.store(true, Ordering::Relaxed);
        assert!(matches!(t.send(&[1]), Err(TransportError::Other(m)) if m == "gatt busy"));
    }

    #[test]
    fn remote_disconnect_and_close() {
        let (mut t, sink, link) = setup();
        sink.disconnected();
        assert!(matches!(
            t.recv(Duration::from_millis(5)),
            Err(TransportError::Disconnected)
        ));
        assert!(matches!(t.send(&[1]), Err(TransportError::Disconnected)));
        sink.push(vec![1]); // ignored, must not panic

        let (mut t2, _s, link2) = setup();
        t2.close();
        t2.close();
        assert!(link2.closed.load(Ordering::Relaxed));
        assert!(matches!(t2.send(&[1]), Err(TransportError::Disconnected)));
        // A remote disconnect does not close the native link; close() still does, once.
        assert!(!link.closed.load(Ordering::Relaxed));
        t.close();
        assert!(link.closed.load(Ordering::Relaxed));
    }

    #[test]
    fn connect_with_keeps_early_notifications() {
        let info = TransportInfo {
            kind: TransportKind::Ble,
            address: "dev2".into(),
            name: "B21".into(),
        };
        let mut t = CallbackTransport::connect_with(info, |sink| {
            sink.push(vec![9]); // arrives before the link is returned
            Ok::<_, String>(Arc::new(FakeLink::default()) as Arc<dyn NativeLink>)
        })
        .unwrap();
        assert_eq!(t.info().uri(), "ble:dev2");
        assert_eq!(t.recv(Duration::from_millis(50)).unwrap(), Some(vec![9]));

        let failed = CallbackTransport::connect_with(t.info().clone(), |_| {
            Err::<Arc<dyn NativeLink>, _>("no device".to_string())
        });
        assert_eq!(failed.unwrap_err(), "no device");
    }

    #[test]
    fn with_packet_gap_overrides() {
        let (t, _s, _l) = setup();
        assert_eq!(
            t.with_packet_gap(Duration::ZERO).packet_gap(),
            Duration::ZERO
        );
    }
}
