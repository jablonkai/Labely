//! `DeviceHub` implementation: one connection (and driver) per address.
//!
//! Address schemes: `fake:<dir>[?model=<id>&delay_ms=<n>]`, `sim:[<model id>]`, `ble:<peripheral id>`,
//! `usb:<serial port>`.
//!
//! On mobile, `ble:` goes through the shell's [`NativeBleHost`] ([`Hub::with_native_ble`],
//! ADR-0025) instead of btleplug.

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use labely_transport::callback::{CallbackTransport, NativeBleHost};
use labely_transport::{TransportInfo, TransportKind};

use labely_core::document::MediaType;
use labely_core::services::{
    DeviceError, DeviceHub, DeviceInfo, DeviceSettings, DeviceStatus, JobControl, ModelInfo,
    PrintReport, PrintRequest,
};

use crate::driver::Driver;
use crate::fake::FakePrinter;
use crate::niimbot::NiimbotDriver;
use crate::registry::{Model, Registry};
use crate::sim::{SimConfig, Simulator};

/// Model used by `fake:` and `sim:` when none is given.
pub const DEFAULT_VIRTUAL_MODEL: &str = "niimbot.b21_pro";

struct Connection {
    driver: Box<dyn Driver>,
    info: DeviceInfo,
    _sim: Option<Simulator>,
}

type Shared = Arc<Mutex<Connection>>;

/// The default device hub.
#[derive(Default)]
pub struct Hub {
    registry: Registry,
    /// Device info lives beside the connection lock, so listing devices never waits for a
    /// running print.
    conns: Mutex<BTreeMap<String, (DeviceInfo, Shared)>>,
    /// Platform BLE stack; when set it replaces btleplug for `ble:` addresses and scans.
    native_ble: Option<Arc<dyn NativeBleHost>>,
}

impl std::fmt::Debug for Hub {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Hub")
            .field("models", &self.registry.models().len())
            .field("native_ble", &self.native_ble.is_some())
            .finish_non_exhaustive()
    }
}

fn locked<T>(m: &Mutex<T>) -> Result<std::sync::MutexGuard<'_, T>, DeviceError> {
    m.lock()
        .map_err(|_| DeviceError::Other("device lock poisoned".into()))
}

/// Lock a connection without waiting: it is busy while a job prints on it.
fn try_locked<'a>(
    m: &'a Mutex<Connection>,
    address: &str,
) -> Result<std::sync::MutexGuard<'a, Connection>, DeviceError> {
    m.try_lock().map_err(|e| match e {
        std::sync::TryLockError::WouldBlock => DeviceError::Unavailable(format!(
            "{address} is busy printing; try again when the job is done"
        )),
        std::sync::TryLockError::Poisoned(_) => DeviceError::Other("device lock poisoned".into()),
    })
}

impl Hub {
    pub fn new() -> Self {
        Self {
            registry: Registry::builtin().clone(),
            conns: Mutex::default(),
            native_ble: None,
        }
    }

    /// Use the shell's BLE stack for `ble:` addresses and scans (Android, iOS).
    pub fn with_native_ble(mut self, host: Arc<dyn NativeBleHost>) -> Self {
        self.native_ble = Some(host);
        self
    }

    fn ble_prefixes(&self) -> Vec<String> {
        self.registry
            .models()
            .iter()
            .flat_map(|m| m.ble_name_prefixes.iter().cloned())
            .collect()
    }

    fn open_native_ble(
        &self,
        host: &Arc<dyn NativeBleHost>,
        id: &str,
    ) -> Result<Box<dyn Driver>, DeviceError> {
        let mut name = String::new();
        let t = CallbackTransport::connect_with(
            TransportInfo {
                kind: TransportKind::Ble,
                address: id.to_owned(),
                name: String::new(),
            },
            |sink| {
                let (link, n) = host.connect(id, sink)?;
                name = n;
                Ok::<_, labely_transport::TransportError>(link)
            },
        )
        .map_err(|e| DeviceError::Unavailable(e.to_string()))?;
        let fallback = self.registry.guess_by_name(&name).cloned();
        Ok(Box::new(
            NiimbotDriver::new(Box::new(t)).with_fallback_model(fallback),
        ))
    }

    fn scan_native_ble(
        &self,
        host: &Arc<dyn NativeBleHost>,
        timeout_ms: u64,
    ) -> Result<Vec<DeviceInfo>, DeviceError> {
        let found = host
            .scan(Duration::from_millis(timeout_ms), &self.ble_prefixes())
            .map_err(|e| DeviceError::Unavailable(e.to_string()))?;
        Ok(found
            .into_iter()
            .map(|d| DeviceInfo {
                // Always `ble:`, whatever kind the shell reported, so saved addresses match.
                address: format!("ble:{}", d.info.address),
                model: self.registry.guess_by_name(&d.info.name).map(Model::info),
                name: d.info.name,
                connected: false,
                firmware: None,
                hardware: None,
                serial: None,
            })
            .collect())
    }

    pub fn registry(&self) -> &Registry {
        &self.registry
    }

    fn model_or_default(&self, id: Option<&str>) -> Result<Model, DeviceError> {
        let id = id
            .filter(|s| !s.is_empty())
            .unwrap_or(DEFAULT_VIRTUAL_MODEL);
        self.registry
            .by_id(id)
            .cloned()
            .ok_or_else(|| DeviceError::NotFound(format!("model '{id}' (see `labely models`)")))
    }

    fn open(&self, address: &str) -> Result<Connection, DeviceError> {
        let (scheme, rest) = address
            .split_once(':')
            .ok_or_else(|| DeviceError::NotFound(format!("'{address}': expected scheme:target")))?;
        let (mut driver, sim): (Box<dyn Driver>, Option<Simulator>) = match scheme {
            "fake" => {
                let (dir, query) = rest.split_once('?').unwrap_or((rest, ""));
                let mut model_id = None;
                let mut delay_ms = 0u64;
                for pair in query.split('&').filter(|p| !p.is_empty()) {
                    match pair.split_once('=') {
                        Some(("model", v)) => model_id = Some(v),
                        Some(("delay_ms", v)) => {
                            delay_ms = v.parse().map_err(|_| {
                                DeviceError::NotFound(format!("'{address}': bad delay_ms"))
                            })?;
                        }
                        _ => {
                            return Err(DeviceError::NotFound(format!(
                                "'{address}': unknown option '{pair}' (use model=, delay_ms=)"
                            )));
                        }
                    }
                }
                let model = self.model_or_default(model_id)?;
                let dir = if dir.is_empty() {
                    "labely-fake-out"
                } else {
                    dir
                };
                let fake = FakePrinter::new(dir, model)
                    .with_page_delay(std::time::Duration::from_millis(delay_ms.min(10_000)));
                (Box::new(fake), None)
            }
            "sim" => {
                let model = self.model_or_default(Some(rest))?;
                let mut cfg = SimConfig::b21_pro();
                cfg.model_code = model.model_codes.first().copied().unwrap_or(0);
                let (sim, transport) = Simulator::start(cfg);
                (Box::new(NiimbotDriver::new(Box::new(transport))), Some(sim))
            }
            "ble" => match &self.native_ble {
                Some(host) => (self.open_native_ble(host, rest)?, None),
                None => (self.open_ble(rest)?, None),
            },
            "usb" => (self.open_serial(rest)?, None),
            other => {
                return Err(DeviceError::NotFound(format!(
                    "unknown scheme '{other}' (use fake:, sim:, ble:, usb:)"
                )));
            }
        };
        let id = driver.connect()?;
        let info = DeviceInfo {
            address: address.to_owned(),
            name: id.name,
            connected: true,
            model: id.model.map(|m| m.info()),
            firmware: id.firmware,
            hardware: id.hardware,
            serial: id.serial,
        };
        Ok(Connection {
            driver,
            info,
            _sim: sim,
        })
    }

    #[cfg(feature = "ble")]
    fn open_ble(&self, id: &str) -> Result<Box<dyn Driver>, DeviceError> {
        let t = labely_transport::ble::open(id, Duration::from_secs(15))
            .map_err(|e| DeviceError::Unavailable(e.to_string()))?;
        let fallback = self
            .registry
            .guess_by_name(&labely_transport::Transport::info(&t).name)
            .cloned();
        Ok(Box::new(
            NiimbotDriver::new(Box::new(t)).with_fallback_model(fallback),
        ))
    }

    #[cfg(not(feature = "ble"))]
    fn open_ble(&self, _id: &str) -> Result<Box<dyn Driver>, DeviceError> {
        Err(DeviceError::Unavailable(
            "built without Bluetooth support (feature `ble`)".into(),
        ))
    }

    #[cfg(feature = "serial")]
    fn open_serial(&self, path: &str) -> Result<Box<dyn Driver>, DeviceError> {
        let t = labely_transport::serial::open(path, labely_transport::serial::DEFAULT_BAUD)
            .map_err(|e| DeviceError::Unavailable(e.to_string()))?;
        Ok(Box::new(NiimbotDriver::new(Box::new(t))))
    }

    #[cfg(not(feature = "serial"))]
    fn open_serial(&self, _path: &str) -> Result<Box<dyn Driver>, DeviceError> {
        Err(DeviceError::Unavailable(
            "built without USB serial support (feature `serial`)".into(),
        ))
    }

    fn conn(&self, address: &str) -> Result<Arc<Mutex<Connection>>, DeviceError> {
        locked(&self.conns)?
            .get(address)
            .map(|(_, c)| c.clone())
            .ok_or_else(|| {
                DeviceError::Unavailable(format!("{address} is not connected; run device.connect"))
            })
    }
}

impl DeviceHub for Hub {
    fn models(&self) -> Vec<ModelInfo> {
        self.registry.models().iter().map(Model::info).collect()
    }

    fn scan(&self, timeout_ms: u64) -> Result<Vec<DeviceInfo>, DeviceError> {
        #[allow(unused_mut)]
        let mut out: Vec<DeviceInfo> = Vec::new();
        let _ = timeout_ms;
        if let Some(host) = &self.native_ble {
            out = self.scan_native_ble(host, timeout_ms)?;
        }
        #[cfg(feature = "ble")]
        if self.native_ble.is_none() {
            let prefixes = self.ble_prefixes();
            let prefixes: Vec<&str> = prefixes.iter().map(String::as_str).collect();
            let found = labely_transport::ble::scan(
                std::time::Duration::from_millis(timeout_ms),
                &prefixes,
            )
            .map_err(|e| DeviceError::Unavailable(e.to_string()))?;
            for d in found {
                let model = self.registry.guess_by_name(&d.info.name).map(Model::info);
                out.push(DeviceInfo {
                    address: d.info.uri(),
                    name: d.info.name,
                    connected: false,
                    model,
                    firmware: None,
                    hardware: None,
                    serial: None,
                });
            }
        }
        #[cfg(feature = "serial")]
        {
            if let Ok(ports) = labely_transport::serial::list() {
                for d in ports {
                    out.push(DeviceInfo {
                        address: d.info.uri(),
                        name: d.info.name,
                        connected: false,
                        model: None,
                        firmware: None,
                        hardware: None,
                        serial: None,
                    });
                }
            }
        }
        let conns = locked(&self.conns)?;
        for d in &mut out {
            d.connected = conns.contains_key(&d.address);
        }
        Ok(out)
    }

    fn connected(&self) -> Vec<DeviceInfo> {
        let Ok(conns) = self.conns.lock() else {
            return Vec::new();
        };
        conns.values().map(|(info, _)| info.clone()).collect()
    }

    fn connect(&self, address: &str) -> Result<DeviceInfo, DeviceError> {
        if let Some((info, _)) = locked(&self.conns)?.get(address) {
            return Ok(info.clone());
        }
        let conn = self.open(address)?;
        let info = conn.info.clone();
        locked(&self.conns)?.insert(
            address.to_owned(),
            (info.clone(), Arc::new(Mutex::new(conn))),
        );
        Ok(info)
    }

    fn disconnect(&self, address: &str) -> Result<(), DeviceError> {
        let c = self.conn(address)?;
        let mut conn = try_locked(&c, address)?;
        conn.driver.close();
        locked(&self.conns)?.remove(address);
        Ok(())
    }

    fn status(&self, address: &str) -> Result<DeviceStatus, DeviceError> {
        let c = self.conn(address)?;
        let mut c = try_locked(&c, address)?;
        c.driver.status()
    }

    fn print(
        &self,
        address: &str,
        job: PrintRequest,
        control: &JobControl,
    ) -> Result<PrintReport, DeviceError> {
        let c = self.conn(address)?;
        let mut c = locked(&c)?;
        c.driver.print(&job, control)
    }

    fn calibrate(&self, address: &str, media: MediaType) -> Result<(), DeviceError> {
        let c = self.conn(address)?;
        let mut c = try_locked(&c, address)?;
        c.driver.calibrate(media)
    }

    fn settings(&self, address: &str) -> Result<DeviceSettings, DeviceError> {
        let c = self.conn(address)?;
        let mut c = try_locked(&c, address)?;
        c.driver.settings()
    }

    fn set_settings(
        &self,
        address: &str,
        patch: &DeviceSettings,
    ) -> Result<DeviceSettings, DeviceError> {
        let c = self.conn(address)?;
        let mut c = try_locked(&c, address)?;
        c.driver.set_settings(patch)?;
        c.driver.settings()
    }
}
