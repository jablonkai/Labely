//! Labely studio: the egui desktop app. The lib target is driven by egui_kittest tests; the
//! bin target (`labely-studio`) opens a window.

pub mod app;
pub mod canvas;
pub mod dialogs;
pub mod jobs;
pub mod mcp_host;
pub mod menu;
pub mod panels;
pub mod templates;

pub use app::{LabelyApp, StudioOptions};
