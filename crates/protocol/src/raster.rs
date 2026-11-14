//! Borrowed 1-bit raster view shared by all protocol encoders.

use thiserror::Error;

/// Errors constructing a [`RasterView`].
#[derive(Debug, Error, PartialEq, Eq)]
pub enum RasterError {
    #[error("raster stride {stride} too small for width {width}")]
    StrideTooSmall { width: u32, stride: usize },
    #[error("raster buffer has {have} bytes, needs {need}")]
    BufferTooSmall { have: usize, need: usize },
}

/// A 1-bit raster: rows packed MSB first, bit set = black (burn).
///
/// Row `y` occupies `data[y * stride .. y * stride + stride]`. Rows run across the
/// printhead; bit 7 of byte 0 is the leftmost dot.
#[derive(Debug, Clone, Copy)]
pub struct RasterView<'a> {
    width: u32,
    height: u32,
    stride: usize,
    data: &'a [u8],
}

impl<'a> RasterView<'a> {
    /// Create a view, validating stride and buffer length.
    pub fn new(
        width: u32,
        height: u32,
        stride: usize,
        data: &'a [u8],
    ) -> Result<Self, RasterError> {
        let min_stride = (width as usize).div_ceil(8);
        if stride < min_stride {
            return Err(RasterError::StrideTooSmall { width, stride });
        }
        let need = stride * height as usize;
        if data.len() < need {
            return Err(RasterError::BufferTooSmall {
                have: data.len(),
                need,
            });
        }
        Ok(Self {
            width,
            height,
            stride,
            data,
        })
    }

    /// Width in dots.
    pub fn width(&self) -> u32 {
        self.width
    }

    /// Height in rows.
    pub fn height(&self) -> u32 {
        self.height
    }

    /// Bytes needed per row on the wire (width rounded up to a multiple of 8).
    pub fn row_bytes(&self) -> usize {
        (self.width as usize).div_ceil(8)
    }

    /// Row `y` trimmed to [`Self::row_bytes`], with padding bits beyond `width` cleared.
    pub fn row(&self, y: u32) -> Vec<u8> {
        let start = y as usize * self.stride;
        let mut row = self.data[start..start + self.row_bytes()].to_vec();
        let rem = self.width % 8;
        if rem != 0
            && let Some(last) = row.last_mut()
        {
            *last &= 0xFFu8 << (8 - rem);
        }
        row
    }

    /// Whether dot `(x, y)` is black.
    pub fn get(&self, x: u32, y: u32) -> bool {
        if x >= self.width || y >= self.height {
            return false;
        }
        let byte = self.data[y as usize * self.stride + (x / 8) as usize];
        byte & (0x80 >> (x % 8)) != 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn row_masks_padding() {
        let data = [0xFF, 0xFF];
        let v = RasterView::new(12, 1, 2, &data).unwrap();
        assert_eq!(v.row(0), vec![0xFF, 0xF0]);
        assert!(v.get(11, 0));
        assert!(!v.get(12, 0));
    }

    #[test]
    fn rejects_short_buffer() {
        assert!(matches!(
            RasterView::new(16, 2, 2, &[0; 3]),
            Err(RasterError::BufferTooSmall { .. })
        ));
    }
}
