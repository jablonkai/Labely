//! Driver trait and shared raster preparation.

use labely_core::Bitmap1;
use labely_core::document::MediaType;
use labely_core::services::{
    DeviceError, DeviceSettings, DeviceStatus, JobControl, PrintReport, PrintRequest,
};

use crate::registry::{Model, PrintDirection};

/// Identity learned at connect time.
#[derive(Debug, Clone)]
pub struct Identity {
    pub name: String,
    pub model: Option<Model>,
    /// Protocol generation (driver specific).
    pub protocol: Option<u8>,
}

/// A printer driver bound to one connection.
pub trait Driver: Send {
    fn connect(&mut self) -> Result<Identity, DeviceError>;
    fn status(&mut self) -> Result<DeviceStatus, DeviceError>;
    /// Print `job`, reporting progress to `control`. Once `control` is cancelled the driver
    /// stops at the next page boundary, leaves the printer idle and returns
    /// [`DeviceError::Cancelled`].
    fn print(
        &mut self,
        job: &PrintRequest,
        control: &JobControl,
    ) -> Result<PrintReport, DeviceError>;
    /// Calibrate the label sensor for `media`.
    fn calibrate(&mut self, media: MediaType) -> Result<(), DeviceError> {
        let _ = media;
        Err(DeviceError::Unavailable(
            "this printer does not support calibration".into(),
        ))
    }
    /// Read the settings the driver knows; unknown ones are `None`.
    fn settings(&mut self) -> Result<DeviceSettings, DeviceError> {
        Ok(DeviceSettings::default())
    }
    /// Apply the `Some` fields of `patch`.
    fn set_settings(&mut self, patch: &DeviceSettings) -> Result<(), DeviceError> {
        let _ = patch;
        Err(DeviceError::Unavailable(
            "this printer has no settings that Labely can change".into(),
        ))
    }
    fn close(&mut self);
}

/// Orient and fit a label bitmap to the model's head (docs/protocol.md §7.1):
/// rotate for `left` models, then make the width a multiple of 8 that fits the head,
/// cropping symmetrically if the label is wider than the head.
pub fn prepare_for_head(bitmap: &Bitmap1, model: &Model) -> Bitmap1 {
    let oriented = match model.print_direction {
        PrintDirection::Top => bitmap.clone(),
        PrintDirection::Left => bitmap.rotate_cw(),
    };
    let w = oriented.width();
    let head = model.head_dots;
    let padded = w.div_ceil(8) * 8;
    if padded <= head {
        return oriented.pad_to_width(padded, 0);
    }
    let target = (head / 8) * 8;
    let crop = w.saturating_sub(target);
    let left = crop / 2;
    let mut out = Bitmap1::new(target, oriented.height());
    for y in 0..oriented.height() {
        for x in 0..target {
            if oriented.get(x + left, y) {
                out.set(x, y, true);
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::registry::Registry;

    #[test]
    fn b21_pro_50mm_crops_to_584() {
        let m = Registry::builtin().by_id("niimbot.b21_pro").unwrap();
        let mut b = Bitmap1::new(591, 10);
        b.set(3, 0, true);
        b.set(4, 0, true);
        let p = prepare_for_head(&b, m);
        assert_eq!(p.width(), 584);
        assert!(p.get(0, 0), "dot 3 lands at 0 after cropping 3 on the left");
    }

    #[test]
    fn narrow_label_pads_to_multiple_of_8() {
        let m = Registry::builtin().by_id("niimbot.b21_pro").unwrap();
        let p = prepare_for_head(&Bitmap1::new(355, 5), m);
        assert_eq!(p.width(), 360);
    }
}
