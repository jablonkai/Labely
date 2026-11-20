//! BLE transport over btleplug for desktop and CLI (ADR-0006). The public API is blocking;
//! each call drives a private tokio runtime. Do not call it from inside another tokio
//! runtime's async context (use `spawn_blocking`), because it blocks on its own runtime.

use std::fmt;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, channel};
use std::time::{Duration, Instant};

use btleplug::api::{
    Central as _, CentralEvent, CharPropFlags, Characteristic, Manager as _, Peripheral as _,
    ScanFilter, WriteType,
};
use btleplug::platform::{Adapter, Manager, Peripheral};
use futures::StreamExt;
use tokio::runtime::Runtime;
use tokio::task::JoinHandle;

use crate::gatt::{
    BLE_PACKET_GAP_MS, CharCandidate, payload_for_att_mtu, scan_matches, select_link,
};
use crate::{Discovered, Transport, TransportError, TransportInfo, TransportKind, recv_while_open};

/// Upper bound for one write-without-response call.
const WRITE_TIMEOUT: Duration = Duration::from_secs(2);
/// Minimum time granted to connect / discover even when the scan used up `timeout`.
const MIN_PHASE: Duration = Duration::from_secs(2);
const CLOSE_TIMEOUT: Duration = Duration::from_secs(2);
const POLL: Duration = Duration::from_millis(100);

fn bt(e: btleplug::Error) -> TransportError {
    match e {
        btleplug::Error::NotConnected => TransportError::Disconnected,
        btleplug::Error::DeviceNotFound => TransportError::NotFound(e.to_string()),
        btleplug::Error::NotSupported(s) => TransportError::Unsupported(s),
        other => TransportError::Bluetooth(other.to_string()),
    }
}

fn runtime() -> Result<Runtime, TransportError> {
    Ok(tokio::runtime::Builder::new_multi_thread()
        .worker_threads(1)
        .thread_name("labely-ble")
        .enable_all()
        .build()?)
}

async fn first_adapter() -> Result<(Manager, Adapter), TransportError> {
    let manager = Manager::new().await.map_err(bt)?;
    let adapter = manager
        .adapters()
        .await
        .map_err(bt)?
        .into_iter()
        .next()
        .ok_or_else(|| TransportError::NotFound("no Bluetooth adapter".into()))?;
    Ok((manager, adapter))
}

/// Advertised name and services of a peripheral, plus RSSI.
async fn describe(
    p: &Peripheral,
) -> Result<(Option<String>, Vec<String>, Option<i16>), TransportError> {
    let props = p.properties().await.map_err(bt)?.unwrap_or_default();
    let name = props.local_name.or(props.advertisement_name);
    let services = props.services.iter().map(|u| u.to_string()).collect();
    Ok((name, services, props.rssi))
}

/// Scan for `timeout` and return the devices that advertise the NIIMBOT service or whose
/// name starts with one of `name_prefixes` (empty = no name filter). Never connects.
pub fn scan(timeout: Duration, name_prefixes: &[&str]) -> Result<Vec<Discovered>, TransportError> {
    let rt = runtime()?;
    rt.block_on(async {
        let (_manager, adapter) = first_adapter().await?;
        adapter
            .start_scan(ScanFilter::default())
            .await
            .map_err(bt)?;
        tokio::time::sleep(timeout).await;
        let peripherals = adapter.peripherals().await.map_err(bt);
        // Best effort: a failing stop must not hide the results.
        let _ = adapter.stop_scan().await;
        let mut out = Vec::new();
        for p in peripherals? {
            let (name, services, rssi) = describe(&p).await?;
            if !scan_matches(name.as_deref(), &services, name_prefixes) {
                continue;
            }
            out.push(Discovered {
                info: TransportInfo {
                    kind: TransportKind::Ble,
                    address: p.id().to_string(),
                    name: name.unwrap_or_default(),
                },
                rssi,
                services,
            });
        }
        // Strongest signal first; unknown RSSI last.
        out.sort_by_key(|d| std::cmp::Reverse(d.rssi.unwrap_or(i16::MIN)));
        Ok(out)
    })
}

/// Connect to the peripheral whose btleplug `PeripheralId` string is `address` (a UUID on
/// macOS/iOS, a MAC-based id elsewhere). Scans until it is seen, connects, discovers
/// services, picks the data characteristic and subscribes to notifications.
pub fn open(address: &str, timeout: Duration) -> Result<BleTransport, TransportError> {
    let rt = runtime()?;
    let deadline = Instant::now() + timeout;
    let remaining = move || {
        deadline
            .saturating_duration_since(Instant::now())
            .max(MIN_PHASE)
    };
    let connected = Arc::new(AtomicBool::new(false));
    let (tx, rx) = channel();

    let (manager, adapter, peripheral, characteristic, name, att_mtu, tasks) =
        rt.block_on(async {
            let (manager, adapter) = first_adapter().await?;
            let peripheral = find(&adapter, address, deadline).await?;
            let (name, _, _) = describe(&peripheral).await?;

            peripheral
                .connect_with_timeout(remaining())
                .await
                .map_err(bt)?;
            connected.store(true, Ordering::Release);
            let setup = async {
                peripheral
                    .discover_services_with_timeout(remaining())
                    .await
                    .map_err(bt)?;
                let chars: Vec<Characteristic> = peripheral.characteristics().into_iter().collect();
                let candidates: Vec<CharCandidate> = chars
                    .iter()
                    .map(|c| CharCandidate {
                        service: c.service_uuid.to_string(),
                        characteristic: c.uuid.to_string(),
                        notify: c.properties.contains(CharPropFlags::NOTIFY),
                        write_without_response: c
                            .properties
                            .contains(CharPropFlags::WRITE_WITHOUT_RESPONSE),
                    })
                    .collect();
                let unsupported = || {
                    TransportError::Unsupported(
                        "no characteristics to write to and be notified on".into(),
                    )
                };
                let link = select_link(&candidates).ok_or_else(unsupported)?;
                let characteristic = chars.get(link.write).cloned().ok_or_else(unsupported)?;
                let listen: Vec<Characteristic> = link
                    .notify
                    .iter()
                    .filter_map(|&i| chars.get(i).cloned())
                    .collect();

                // Open both streams before subscribing so no early notification is lost.
                let mut notifications = peripheral.notifications().await.map_err(bt)?;
                let mut events = adapter.events().await.map_err(bt)?;
                let uuids: Vec<_> = listen.iter().map(|c| c.uuid).collect();
                let forward = tokio::spawn(async move {
                    while let Some(n) = notifications.next().await {
                        if uuids.contains(&n.uuid) && tx.send(n.value).is_err() {
                            break;
                        }
                    }
                });
                let id = peripheral.id();
                let flag = Arc::clone(&connected);
                let watch = tokio::spawn(async move {
                    while let Some(ev) = events.next().await {
                        if matches!(&ev, CentralEvent::DeviceDisconnected(d) if *d == id) {
                            flag.store(false, Ordering::Release);
                            break;
                        }
                    }
                });
                for c in &listen {
                    peripheral.subscribe(c).await.map_err(bt)?;
                }
                Ok::<_, TransportError>((characteristic, vec![forward, watch]))
            };
            match setup.await {
                Ok((characteristic, tasks)) => {
                    let att_mtu = peripheral.mtu();
                    Ok((
                        manager,
                        adapter,
                        peripheral,
                        characteristic,
                        name,
                        att_mtu,
                        tasks,
                    ))
                }
                Err(e) => {
                    let _ = tokio::time::timeout(CLOSE_TIMEOUT, peripheral.disconnect()).await;
                    Err(e)
                }
            }
        })?;

    let info = TransportInfo {
        kind: TransportKind::Ble,
        address: peripheral.id().to_string(),
        name: name.unwrap_or_default(),
    };
    Ok(BleTransport {
        info,
        mtu: payload_for_att_mtu(att_mtu),
        rx,
        connected,
        closed: false,
        tasks,
        characteristic,
        peripheral,
        _adapter: adapter,
        _manager: manager,
        rt,
    })
}

async fn find(
    adapter: &Adapter,
    address: &str,
    deadline: Instant,
) -> Result<Peripheral, TransportError> {
    let lookup = async || -> Result<Option<Peripheral>, TransportError> {
        Ok(adapter
            .peripherals()
            .await
            .map_err(bt)?
            .into_iter()
            .find(|p| p.id().to_string().eq_ignore_ascii_case(address)))
    };
    if let Some(p) = lookup().await? {
        return Ok(p);
    }
    adapter
        .start_scan(ScanFilter::default())
        .await
        .map_err(bt)?;
    let found = loop {
        match lookup().await {
            Ok(Some(p)) => break Ok(p),
            Ok(None) if Instant::now() >= deadline => {
                break Err(TransportError::NotFound(format!("BLE device {address}")));
            }
            Ok(None) => tokio::time::sleep(POLL).await,
            Err(e) => break Err(e),
        }
    };
    let _ = adapter.stop_scan().await;
    found
}

/// A connected BLE peripheral. Created by [`open`].
pub struct BleTransport {
    info: TransportInfo,
    mtu: usize,
    rx: Receiver<Vec<u8>>,
    connected: Arc<AtomicBool>,
    closed: bool,
    tasks: Vec<JoinHandle<()>>,
    characteristic: Characteristic,
    peripheral: Peripheral,
    _adapter: Adapter,
    _manager: Manager,
    // Declared last so it is dropped after everything that may hold runtime handles.
    rt: Runtime,
}

impl BleTransport {
    /// Override the write payload size (bytes per GATT write). The default comes from the
    /// MTU btleplug reports after discovery (ATT MTU − 3, at least 20).
    pub fn set_mtu(&mut self, payload: usize) {
        self.mtu = payload.max(1);
    }

    /// The ATT MTU btleplug currently reports (23 if nothing was negotiated or exposed).
    pub fn att_mtu(&self) -> u16 {
        self.peripheral.mtu()
    }

    fn is_open(&self) -> bool {
        !self.closed && self.connected.load(Ordering::Acquire)
    }
}

impl fmt::Debug for BleTransport {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("BleTransport")
            .field("info", &self.info)
            .field("mtu", &self.mtu)
            .field("characteristic", &self.characteristic.uuid)
            .field("open", &self.is_open())
            .finish_non_exhaustive()
    }
}

impl Transport for BleTransport {
    fn send(&mut self, bytes: &[u8]) -> Result<(), TransportError> {
        if !self.is_open() {
            return Err(TransportError::Disconnected);
        }
        for chunk in bytes.chunks(self.mtu) {
            let write =
                self.peripheral
                    .write(&self.characteristic, chunk, WriteType::WithoutResponse);
            // The timer must be created inside the runtime: `timeout` registers with the
            // time driver when constructed, so building it outside `block_on` panics.
            match self
                .rt
                .block_on(async { tokio::time::timeout(WRITE_TIMEOUT, write).await })
            {
                Ok(Ok(())) => {}
                Ok(Err(e)) => {
                    let e = bt(e);
                    if matches!(e, TransportError::Disconnected) {
                        self.connected.store(false, Ordering::Release);
                    }
                    return Err(e);
                }
                Err(_) => return Err(TransportError::Bluetooth("write timed out".into())),
            }
        }
        Ok(())
    }

    fn recv(&mut self, timeout: Duration) -> Result<Option<Vec<u8>>, TransportError> {
        if self.closed {
            return Err(TransportError::Disconnected);
        }
        recv_while_open(&self.rx, &self.connected, timeout)
    }

    fn mtu(&self) -> usize {
        self.mtu
    }

    fn packet_gap(&self) -> Duration {
        Duration::from_millis(BLE_PACKET_GAP_MS)
    }

    fn info(&self) -> &TransportInfo {
        &self.info
    }

    fn close(&mut self) {
        if self.closed {
            return;
        }
        self.closed = true;
        let was_connected = self.connected.swap(false, Ordering::AcqRel);
        let (p, c) = (&self.peripheral, &self.characteristic);
        self.rt.block_on(async {
            let _ = tokio::time::timeout(CLOSE_TIMEOUT, async {
                if was_connected {
                    let _ = p.unsubscribe(c).await;
                }
                let _ = p.disconnect().await;
            })
            .await;
        });
        for t in self.tasks.drain(..) {
            t.abort();
        }
    }
}

impl Drop for BleTransport {
    fn drop(&mut self) {
        self.close();
    }
}
