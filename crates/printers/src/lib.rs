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

/// Production services: the bundled-font renderer and the default device hub.
pub fn default_services() -> labely_core::Services {
    labely_core::Services {
        renderer: Arc::new(labely_render::LabelRenderer::new()),
        devices: Arc::new(Hub::new()),
    }
}
