//! Pure, I/O-free encoders and decoders for label printer protocols.
//!
//! NIIMBOT is written from the clean-room specification in `docs/protocol.md`.
//! Everything else is written from `docs/protocol-languages.md`. The crate never touches a
//! transport: drivers in `labely-printers` execute the [`niimbot::job::Step`] programs
//! produced here, or send the byte streams of the other modules.

pub mod escpos;
pub mod marklife;
pub mod niimbot;
pub mod phomemo;
pub mod raster;
pub mod tspl;
pub mod zpl;

pub use raster::{GrayView, Raster, RasterError, RasterView};
