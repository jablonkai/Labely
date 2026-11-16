//! Printer drivers, the data-driven model registry, the device hub and FakePrinter.
//!
//! Adding a model is one entry in `models/<vendor>.json`.

pub mod driver;
pub mod fake;
pub mod hub;
pub mod niimbot;
pub mod registry;
pub mod sim;

pub use driver::{Driver, Identity};
pub use hub::Hub;
pub use registry::{Model, Registry};

use std::sync::Arc;

/// Production services: the bundled-font renderer and the default device hub. Output does
/// not depend on the machine's fonts (CLI, MCP, tests).
pub fn default_services() -> labely_core::Services {
    labely_core::Services {
        renderer: Arc::new(labely_render::LabelRenderer::new()),
        devices: Arc::new(Hub::new()),
    }
}

/// Mobile services: bundled fonts and a hub whose `ble:` addresses go through the shell's
/// BLE stack (ADR-0025).
pub fn mobile_services(
    ble: Arc<dyn labely_transport::callback::NativeBleHost>,
) -> labely_core::Services {
    labely_core::Services {
        renderer: Arc::new(labely_render::LabelRenderer::new()),
        devices: Arc::new(Hub::new().with_native_ble(ble)),
    }
}

/// Desktop services: like [`default_services`], plus the fonts installed on the machine.
pub fn desktop_services() -> labely_core::Services {
    labely_core::Services {
        renderer: Arc::new(labely_render::LabelRenderer::with_system_fonts()),
        devices: Arc::new(Hub::new()),
    }
}
