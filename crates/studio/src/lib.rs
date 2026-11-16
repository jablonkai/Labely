//! Labely studio: the egui desktop app. The lib target is driven by egui_kittest tests; the
//! bin target (`labely-studio`) opens a window.

pub mod app;
pub mod canvas;
pub mod data;
pub mod dialogs;
pub mod home;
mod icon_paths;
pub mod icons;
pub mod inspector;
pub mod jobs;
pub mod mcp_host;
pub mod menu;
pub mod panels;
pub mod paper;
pub mod prefs;
pub mod printers;
pub mod rich;
pub mod shell;
pub mod templates;
pub mod theme;

pub use app::{LabelyApp, StudioOptions};
