//! Borrowed raster views shared by all protocol encoders.

use thiserror::Error;

/// Errors constructing a [`RasterView`].
#[derive(Debug, Error, PartialEq, Eq)]
pub enum RasterError {
    #[error("raster stride {stride} too small for width {width}")]
    StrideTooSmall { width: u32, stride: usize },
    #[error("raster buffer has {have} bytes, needs {need}")]
    BufferTooSmall { have: usize, need: usize },
    #[error("grayscale raster width {0} is not even")]
    OddWidth(u32),
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

/// An owned 1-bit raster in the layout of [`RasterView`], with the tightest stride. The
/// decoders return it; rows are appended as they arrive.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Raster {
    width: u32,
    height: u32,
    data: Vec<u8>,
}

impl Raster {
    /// An empty raster `width` dots wide.
    pub fn new(width: u32) -> Self {
        Self {
            width,
            height: 0,
            data: Vec::new(),
        }
    }

    /// Width in dots.
    pub fn width(&self) -> u32 {
        self.width
    }

    /// Height in rows.
    pub fn height(&self) -> u32 {
        self.height
    }

    /// Bytes per row.
    pub fn row_bytes(&self) -> usize {
        (self.width as usize).div_ceil(8)
    }

    /// Packed rows, `row_bytes * height` bytes.
    pub fn data(&self) -> &[u8] {
        &self.data
    }

    /// Append whole rows. `rows` must hold a multiple of [`Self::row_bytes`] bytes; a
    /// trailing partial row is dropped.
    pub fn push_rows(&mut self, rows: &[u8]) {
        let rb = self.row_bytes();
        if rb == 0 {
            return;
        }
        let whole = rows.len() / rb;
        self.data.extend_from_slice(&rows[..whole * rb]);
        self.height += whole as u32;
    }

    /// Whether dot `(x, y)` is black.
    pub fn get(&self, x: u32, y: u32) -> bool {
        self.view().get(x, y)
    }

    /// Borrow as a view.
    pub fn view(&self) -> RasterView<'_> {
        RasterView {
            width: self.width,
            height: self.height,
            stride: self.row_bytes(),
            data: &self.data,
        }
    }
}

/// A 16-level gray raster: one byte per dot, 0 = white, 15 = black. Larger values count
/// as black.
///
/// Row `y` occupies `data[y * width .. (y + 1) * width]`. The width is even, because the
/// wire format packs two dots per byte.
#[derive(Debug, Clone, Copy)]
pub struct GrayView<'a> {
    width: u32,
    height: u32,
    data: &'a [u8],
}

impl<'a> GrayView<'a> {
    /// Create a view, validating the width and buffer length.
    pub fn new(width: u32, height: u32, data: &'a [u8]) -> Result<Self, RasterError> {
        if !width.is_multiple_of(2) {
            return Err(RasterError::OddWidth(width));
        }
        let need = width as usize * height as usize;
        if data.len() < need {
            return Err(RasterError::BufferTooSmall {
                have: data.len(),
                need,
            });
        }
        Ok(Self {
            width,
            height,
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

    /// The levels of row `y`.
    pub fn row(&self, y: u32) -> &'a [u8] {
        let w = self.width as usize;
        &self.data[y as usize * w..(y as usize + 1) * w]
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
    fn gray_view_checks_width_and_length() {
        assert_eq!(
            GrayView::new(3, 1, &[0; 3]).unwrap_err(),
            RasterError::OddWidth(3)
        );
        assert!(GrayView::new(4, 2, &[0; 7]).is_err());
        let data = [0, 1, 2, 3, 4, 5, 6, 7];
        assert_eq!(GrayView::new(4, 2, &data).unwrap().row(1), &[4, 5, 6, 7]);
    }

    #[test]
    fn rejects_short_buffer() {
        assert!(matches!(
            RasterView::new(16, 2, 2, &[0; 3]),
            Err(RasterError::BufferTooSmall { .. })
        ));
    }
}
