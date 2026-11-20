//! Labely core: document model, command registry, session, undo/redo, batch.
//!
//! The command registry is the only way to change a document (ADR-0004): build a
//! [`Session`] with [`Services`] and call [`Session::execute`].

pub mod command;
pub mod commands;
pub mod datetime;
pub mod document;
pub mod envelope;
pub mod file;
pub mod jobs;
pub mod paper;
pub mod pdf;
pub mod policy;
pub mod raster;
pub mod registry;
pub mod schema;
pub mod services;
pub mod session;
pub mod steps;

pub use command::{Command, CommandError, CommandKind, CommandMeta, Ctx, ErrorCode};
pub use document::{Document, Element, ElementId, ElementKind, LabelSpec};
pub use envelope::Envelope;
pub use policy::Policy;
pub use raster::{Bitmap1, Gray4};
pub use registry::{CommandInfo, Registry};
pub use services::{DeviceHub, Renderer, Services};
pub use session::{JournalEntry, Session};

/// Command API version; bump when any command schema changes incompatibly.
pub const API_VERSION: u32 = 3;
