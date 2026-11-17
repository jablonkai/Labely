//! Printer and job state pushed to the shell instead of polling (ADR-0031).
//!
//! Every source (the print queue, the device commands, a dropped BLE link) only sends an
//! [`Event`] down a channel. One dispatcher thread per engine calls the shell's
//! [`DeviceEvents`], so a callback may call back into the engine without deadlocking it.

use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::mpsc::{Receiver, Sender};
use std::sync::{Arc, Mutex, PoisonError, Weak};

use labely_core::DeviceHub;
use labely_core::jobs::JobInfo;
use serde_json::{Value, json};

/// Receives printer and job changes. Implemented by the shell and set with
/// [`crate::LabelyEngine::set_events`]. Calls come on one engine thread, in order. They must
/// not throw.
#[uniffi::export(with_foreign)]
pub trait DeviceEvents: Send + Sync {
    /// A job was queued, started, printed a page, finished or was cancelled. `job_json` is
    /// the `job.get` result.
    fn job_changed(&self, job_json: String);

    /// A printer connected, disconnected, dropped its link, or reported its status.
    /// `event_json` is `{"address", "connected", "device"?, "status"?, "lost"?}`: `device`
    /// after `device.connect`, `status` after `device.status`, `lost: true` when the link
    /// dropped without `device.disconnect`.
    fn device_changed(&self, event_json: String);
}

pub(crate) enum Event {
    Job(Box<JobInfo>),
    Device(Value),
    /// The BLE link of this address dropped.
    Lost(String),
}

pub(crate) type Listener = Arc<Mutex<Option<Arc<dyn DeviceEvents>>>>;

/// Start the dispatcher. It ends when every sender is gone, i.e. with the engine.
pub(crate) fn spawn(
    rx: Receiver<Event>,
    listener: Listener,
    hub: Weak<dyn DeviceHub>,
) -> std::io::Result<()> {
    std::thread::Builder::new()
        .name("labely-events".into())
        .spawn(move || {
            for ev in rx {
                deliver(ev, &listener, &hub);
            }
        })
        .map(drop)
}

fn deliver(ev: Event, listener: &Listener, hub: &Weak<dyn DeviceHub>) {
    let (job, device) = match ev {
        Event::Job(info) => match serde_json::to_string(&info) {
            Ok(s) => (Some(s), None),
            Err(_) => return,
        },
        Event::Device(v) => (None, Some(v.to_string())),
        Event::Lost(address) => {
            // Forget the dead connection, so the next device.connect opens a new link.
            if let Some(hub) = hub.upgrade() {
                let _ = hub.disconnect(&address);
            }
            let v = json!({"address": address, "connected": false, "lost": true});
            (None, Some(v.to_string()))
        }
    };
    let Some(l) = listener
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .clone()
    else {
        return;
    };
    // A failing shell callback must not stop later events.
    let _ = catch_unwind(AssertUnwindSafe(|| {
        if let Some(j) = job {
            l.job_changed(j);
        }
        if let Some(d) = device {
            l.device_changed(d);
        }
    }));
}

/// The device event a successful command result implies, if any.
pub(crate) fn from_command(id: &str, result: &Value) -> Option<Value> {
    let address = result.get("address")?.clone();
    match id {
        "device.connect" => Some(json!({"address": address, "connected": true, "device": result})),
        "device.disconnect" => Some(json!({"address": address, "connected": false})),
        "device.status" => Some(json!({
            "address": address,
            "connected": true,
            "status": result.get("status")?,
        })),
        _ => None,
    }
}

/// A sender the print queue's observer and the BLE sinks can share.
pub(crate) type EventTx = Sender<Event>;
