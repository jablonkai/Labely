//! The shell's BLE stack as UniFFI foreign traits, adapted to
//! [`labely_transport::callback`] (ADR-0006, ADR-0025).

use std::sync::Arc;
use std::time::Duration;

use labely_transport::callback::{NativeBleHost, NativeLink, NotifySink};
use labely_transport::{Discovered, TransportError, TransportInfo, TransportKind};

/// A BLE failure reported by the shell. Fields are `detail`, since Kotlin exceptions already
/// have `message`.
#[derive(Debug, thiserror::Error, uniffi::Error)]
pub enum BleError {
    /// Bluetooth is off or the app lacks the permission.
    #[error("bluetooth unavailable: {detail}")]
    Unavailable { detail: String },
    /// No such device, or it went away.
    #[error("device not found: {detail}")]
    NotFound { detail: String },
    /// Any other GATT or platform error.
    #[error("{detail}")]
    Failed { detail: String },
}

impl From<uniffi::UnexpectedUniFFICallbackError> for BleError {
    fn from(e: uniffi::UnexpectedUniFFICallbackError) -> Self {
        Self::Failed { detail: e.reason }
    }
}

impl From<BleError> for TransportError {
    fn from(e: BleError) -> Self {
        match e {
            BleError::Unavailable { detail } => TransportError::Bluetooth(detail),
            BleError::NotFound { detail } => TransportError::NotFound(detail),
            BleError::Failed { detail } => TransportError::Other(detail),
        }
    }
}

/// A peripheral found by [`NativeBle::scan`].
#[derive(Debug, Clone, uniffi::Record)]
pub struct BleDevice {
    /// Platform id: the MAC address on Android, the peripheral UUID on iOS.
    pub id: String,
    /// Advertised name.
    pub name: String,
    pub rssi: Option<i16>,
}

/// The shell's BLE stack.
#[uniffi::export(with_foreign)]
pub trait NativeBle: Send + Sync {
    /// Scan for `timeout_ms`, returning devices whose advertised name starts with one of
    /// `name_prefixes` (all devices when empty).
    fn scan(&self, timeout_ms: u64, name_prefixes: Vec<String>)
    -> Result<Vec<BleDevice>, BleError>;

    /// Connect to `id`, discover the printer's GATT service, enable notifications and
    /// negotiate the MTU. Every notification goes to `sink.push`, and a dropped link to
    /// `sink.disconnected`.
    fn connect(
        &self,
        id: String,
        sink: Arc<BleNotifySink>,
    ) -> Result<Arc<dyn NativeBleLink>, BleError>;
}

/// One connected printer link, implemented by the shell.
#[uniffi::export(with_foreign)]
pub trait NativeBleLink: Send + Sync {
    /// Write one chunk of at most `mtu()` bytes (write-without-response, queued in order).
    fn write(&self, bytes: Vec<u8>) -> Result<(), BleError>;
    /// Maximum write payload: negotiated ATT MTU − 3.
    fn mtu(&self) -> u32;
    /// Advertised name of the connected device.
    fn name(&self) -> String;
    /// Disconnect. Called once. (Not `close`, which Kotlin's `AutoCloseable` already uses.)
    fn disconnect(&self);
}

/// Where the shell delivers notifications of a connected link.
#[derive(Debug, uniffi::Object)]
pub struct BleNotifySink(NotifySink);

#[uniffi::export]
impl BleNotifySink {
    /// Bytes of one notification.
    pub fn push(&self, bytes: Vec<u8>) {
        self.0.push(bytes);
    }

    /// The link dropped.
    pub fn disconnected(&self) {
        self.0.disconnected();
    }
}

/// [`NativeBleHost`] over the foreign [`NativeBle`].
pub(crate) struct Host(Arc<dyn NativeBle>);

impl Host {
    pub(crate) fn new(ble: Arc<dyn NativeBle>) -> Self {
        Self(ble)
    }
}

struct Link(Arc<dyn NativeBleLink>);

impl NativeLink for Link {
    fn write(&self, bytes: Vec<u8>) -> Result<(), String> {
        self.0.write(bytes).map_err(|e| e.to_string())
    }

    fn mtu(&self) -> u32 {
        self.0.mtu()
    }

    fn close(&self) {
        self.0.disconnect();
    }
}

impl NativeBleHost for Host {
    fn scan(
        &self,
        timeout: Duration,
        name_prefixes: &[String],
    ) -> Result<Vec<Discovered>, TransportError> {
        let ms = u64::try_from(timeout.as_millis()).unwrap_or(u64::MAX);
        Ok(self
            .0
            .scan(ms, name_prefixes.to_vec())?
            .into_iter()
            .map(|d| Discovered {
                info: TransportInfo {
                    kind: TransportKind::Ble,
                    address: d.id,
                    name: d.name,
                },
                rssi: d.rssi,
                services: Vec::new(),
            })
            .collect())
    }

    fn connect(
        &self,
        address: &str,
        sink: NotifySink,
    ) -> Result<(Arc<dyn NativeLink>, String), TransportError> {
        let link = self
            .0
            .connect(address.to_owned(), Arc::new(BleNotifySink(sink)))?;
        let name = link.name();
        Ok((Arc::new(Link(link)), name))
    }
}
