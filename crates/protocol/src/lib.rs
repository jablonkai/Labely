//! Pure, I/O-free encoders and decoders for label printer protocols.
//!
//! Everything here is written from the clean-room specification in `docs/protocol.md`.
//! The crate never touches a transport: drivers in `labely-printers` execute the
//! [`niimbot::job::Step`] programs produced here.

pub mod niimbot;
pub mod raster;

pub use raster::{RasterError, RasterView};
