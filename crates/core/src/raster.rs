//! 1-bit raster produced by `labely-render` and consumed by printer drivers.

use serde::{Deserialize, Serialize};

/// 1-bit bitmap. Rows are packed MSB first; a set bit is black (burned).
///
/// Orientation is the label as designed (x to the right, y down); drivers rotate for the
/// model's print direction.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Bitmap1 {
    width: u32,
    height: u32,
    stride: usize,
    data: Vec<u8>,
}

impl std::fmt::Debug for Bitmap1 {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Bitmap1")
            .field("width", &self.width)
            .field("height", &self.height)
            .finish_non_exhaustive()
    }
}

impl Bitmap1 {
    /// All-white bitmap.
    pub fn new(width: u32, height: u32) -> Self {
        let stride = (width as usize).div_ceil(8);
        Self {
            width,
            height,
            stride,
            data: vec![0; stride * height as usize],
        }
    }

    pub fn width(&self) -> u32 {
        self.width
    }

    pub fn height(&self) -> u32 {
        self.height
    }

    /// Bytes per row.
    pub fn stride(&self) -> usize {
        self.stride
    }

    /// Packed row data, `stride * height` bytes.
    pub fn data(&self) -> &[u8] {
        &self.data
    }

    pub fn get(&self, x: u32, y: u32) -> bool {
        if x >= self.width || y >= self.height {
            return false;
        }
        self.data[y as usize * self.stride + (x / 8) as usize] & (0x80 >> (x % 8)) != 0
    }

    /// Set dot `(x, y)`; out-of-range coordinates are ignored.
    pub fn set(&mut self, x: u32, y: u32, black: bool) {
        if x >= self.width || y >= self.height {
            return;
        }
        let i = y as usize * self.stride + (x / 8) as usize;
        let m = 0x80 >> (x % 8);
        if black {
            self.data[i] |= m;
        } else {
            self.data[i] &= !m;
        }
    }

    /// Number of black dots.
    pub fn count_black(&self) -> u64 {
        self.data.iter().map(|b| u64::from(b.count_ones())).sum()
    }

    /// Rotate 90° clockwise.
    pub fn rotate_cw(&self) -> Self {
        let mut out = Self::new(self.height, self.width);
        for y in 0..self.height {
            for x in 0..self.width {
                if self.get(x, y) {
                    out.set(self.height - 1 - y, x, true);
                }
            }
        }
        out
    }

    /// Place this bitmap into a wider canvas of `width` dots, horizontally offset by `x0`.
    pub fn pad_to_width(&self, width: u32, x0: u32) -> Self {
        let mut out = Self::new(width, self.height);
        for y in 0..self.height {
            for x in 0..self.width {
                if self.get(x, y) {
                    out.set(x + x0, y, true);
                }
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn set_get_rotate() {
        let mut b = Bitmap1::new(3, 2);
        b.set(2, 0, true);
        assert!(b.get(2, 0));
        let r = b.rotate_cw();
        assert_eq!((r.width(), r.height()), (2, 3));
        assert!(r.get(1, 2));
        assert_eq!(r.count_black(), 1);
    }
}
