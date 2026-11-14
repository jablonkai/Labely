//! `DeviceHub` implementation: one connection (and driver) per address.
//!
//! Address schemes: `fake:<dir>[?model=<id>]`, `sim:[<model id>]`, `ble:<peripheral id>`,
//! `usb:<serial port>`.

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use labely_core::services::{
    DeviceError, DeviceHub, DeviceInfo, DeviceStatus, ModelInfo, PrintReport, PrintRequest,
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

/// The default device hub.
#[derive(Default)]
pub struct Hub {
    registry: Registry,
    conns: Mutex<BTreeMap<String, Arc<Mutex<Connection>>>>,
}

impl std::fmt::Debug for Hub {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Hub")
            .field("models", &self.registry.models().len())
            .finish_non_exhaustive()
    }
}

fn locked<T>(m: &Mutex<T>) -> Result<std::sync::MutexGuard<'_, T>, DeviceError> {
    m.lock()
        .map_err(|_| DeviceError::Other("device lock poisoned".into()))
}

impl Hub {
    pub fn new() -> Self {
        Self {
            registry: Registry::builtin().clone(),
            conns: Mutex::default(),
        }
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
                let model_id = query.strip_prefix("model=");
                let model = self.model_or_default(model_id)?;
                let dir = if dir.is_empty() {
                    "labely-fake-out"
                } else {
                    dir
                };
                (Box::new(FakePrinter::new(dir, model)), None)
            }
            "sim" => {
                let model = self.model_or_default(Some(rest))?;
                let mut cfg = SimConfig::b21_pro();
                cfg.model_code = model.model_codes.first().copied().unwrap_or(0);
                let (sim, transport) = Simulator::start(cfg);
                (Box::new(NiimbotDriver::new(Box::new(transport))), Some(sim))
            }
            "ble" => (self.open_ble(rest)?, None),
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
        };
        Ok(Connection {
            driver,
            info,
            _sim: sim,
        })
    }

    #[cfg(feature = "ble")]
    fn open_ble(&self, id: &str) -> Result<Box<dyn Driver>, DeviceError> {
        use std::time::Duration;
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
            .cloned()
            .ok_or_else(|| DeviceError::NotFound(format!("{address} is not connected")))
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
        #[cfg(feature = "ble")]
        {
            let prefixes: Vec<&str> = self
                .registry
                .models()
                .iter()
                .flat_map(|m| m.ble_name_prefixes.iter().map(String::as_str))
                .collect();
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
        conns
            .values()
            .filter_map(|c| c.lock().ok().map(|c| c.info.clone()))
            .collect()
    }

    fn connect(&self, address: &str) -> Result<DeviceInfo, DeviceError> {
        if let Ok(c) = self.conn(address) {
            return Ok(locked(&c)?.info.clone());
        }
        let conn = self.open(address)?;
        let info = conn.info.clone();
        locked(&self.conns)?.insert(address.to_owned(), Arc::new(Mutex::new(conn)));
        Ok(info)
    }

    fn disconnect(&self, address: &str) -> Result<(), DeviceError> {
        let removed = locked(&self.conns)?.remove(address);
        match removed {
            Some(c) => {
                locked(&c)?.driver.close();
                Ok(())
            }
            None => Err(DeviceError::NotFound(format!("{address} is not connected"))),
        }
    }

    fn status(&self, address: &str) -> Result<DeviceStatus, DeviceError> {
        let c = self.conn(address)?;
        let mut c = locked(&c)?;
        c.driver.status()
    }

    fn print(&self, address: &str, job: PrintRequest) -> Result<PrintReport, DeviceError> {
        let c = self.conn(address)?;
        let mut c = locked(&c)?;
        c.driver.print(&job, &mut |p| {
            tracing::info!(done = p.pages_done, total = p.pages_total, "print progress")
        })
    }
}
