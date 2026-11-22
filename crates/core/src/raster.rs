//! Rasters produced by `labely-render` and consumed by printer drivers.

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

    /// Mirror left to right.
    pub fn mirror_h(&self) -> Self {
        let mut out = Self::new(self.width, self.height);
        for y in 0..self.height {
            for x in 0..self.width {
                if self.get(x, y) {
                    out.set(self.width - 1 - x, y, true);
                }
            }
        }
        out
    }

    /// Rotate 180°.
    pub fn rotate_180(&self) -> Self {
        let mut out = Self::new(self.width, self.height);
        for y in 0..self.height {
            for x in 0..self.width {
                if self.get(x, y) {
                    out.set(self.width - 1 - x, self.height - 1 - y, true);
                }
            }
        }
        out
    }

    /// Move the content by `(dx, dy)` dots (positive is right and down) on a canvas of the
    /// same size; dots pushed off the edge are dropped.
    pub fn shifted(&self, dx: i64, dy: i64) -> Self {
        let mut out = Self::new(self.width, self.height);
        for y in 0..self.height {
            for x in 0..self.width {
                if self.get(x, y) {
                    let (nx, ny) = (i64::from(x) + dx, i64::from(y) + dy);
                    if let (Ok(nx), Ok(ny)) = (u32::try_from(nx), u32::try_from(ny)) {
                        out.set(nx, ny, true);
                    }
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

/// 16-level gray map for printers with grayscale rows: one byte per dot, 0 = white,
/// 15 = black. Same orientation as [`Bitmap1`].
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Gray4 {
    width: u32,
    height: u32,
    data: Vec<u8>,
}

impl std::fmt::Debug for Gray4 {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Gray4")
            .field("width", &self.width)
            .field("height", &self.height)
            .finish_non_exhaustive()
    }
}

impl Gray4 {
    /// Highest level (black).
    pub const BLACK: u8 = 15;

    /// All-white map.
    pub fn new(width: u32, height: u32) -> Self {
        Self {
            width,
            height,
            data: vec![0; width as usize * height as usize],
        }
    }

    pub fn width(&self) -> u32 {
        self.width
    }

    pub fn height(&self) -> u32 {
        self.height
    }

    /// The levels, row by row: `width * height` bytes.
    pub fn data(&self) -> &[u8] {
        &self.data
    }

    /// Level of dot `(x, y)`; white outside the map.
    pub fn get(&self, x: u32, y: u32) -> u8 {
        if x >= self.width || y >= self.height {
            return 0;
        }
        self.data[y as usize * self.width as usize + x as usize]
    }

    /// Set dot `(x, y)`; levels above 15 become 15, and out-of-range coordinates are
    /// ignored.
    pub fn set(&mut self, x: u32, y: u32, level: u8) {
        if x >= self.width || y >= self.height {
            return;
        }
        self.data[y as usize * self.width as usize + x as usize] = level.min(Self::BLACK);
    }

    /// Rotate 90° clockwise.
    pub fn rotate_cw(&self) -> Self {
        let mut out = Self::new(self.height, self.width);
        for y in 0..self.height {
            for x in 0..self.width {
                out.set(self.height - 1 - y, x, self.get(x, y));
            }
        }
        out
    }

    /// The 16 levels of an 8-bit gray image (row-major, 0 = black, 255 = white), each
    /// rounded to the nearest level. Missing pixels are white.
    pub fn from_gray8(width: u32, height: u32, gray: &[u8]) -> Self {
        let mut out = Self::new(width, height);
        for (dst, &g) in out.data.iter_mut().zip(gray) {
            *dst = ((u16::from(255 - g) * u16::from(Self::BLACK) + 127) / 255) as u8;
        }
        out
    }

    /// Mirror left to right.
    pub fn mirror_h(&self) -> Self {
        let mut out = Self::new(self.width, self.height);
        for y in 0..self.height {
            for x in 0..self.width {
                out.set(self.width - 1 - x, y, self.get(x, y));
            }
        }
        out
    }

    /// Rotate 180°.
    pub fn rotate_180(&self) -> Self {
        let mut out = Self::new(self.width, self.height);
        for y in 0..self.height {
            for x in 0..self.width {
                out.set(self.width - 1 - x, self.height - 1 - y, self.get(x, y));
            }
        }
        out
    }

    /// As [`Bitmap1::shifted`]: dots pushed off the edge are dropped.
    pub fn shifted(&self, dx: i64, dy: i64) -> Self {
        let mut out = Self::new(self.width, self.height);
        for y in 0..self.height {
            for x in 0..self.width {
                let (nx, ny) = (i64::from(x) + dx, i64::from(y) + dy);
                if let (Ok(nx), Ok(ny)) = (u32::try_from(nx), u32::try_from(ny)) {
                    out.set(nx, ny, self.get(x, y));
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
    fn gray_set_get_rotate() {
        let mut g = Gray4::new(3, 2);
        g.set(2, 0, 200);
        g.set(0, 1, 7);
        g.set(9, 9, 1);
        assert_eq!((g.get(2, 0), g.get(0, 1), g.get(9, 9)), (15, 7, 0));
        let r = g.rotate_cw();
        assert_eq!((r.width(), r.height()), (2, 3));
        assert_eq!((r.get(1, 2), r.get(0, 0)), (15, 7));
    }

    #[test]
    fn gray_from_gray8_and_transforms() {
        let g = Gray4::from_gray8(4, 2, &[255, 0, 128, 200, 9, 255]);
        assert_eq!(&g.data()[..6], &[0, 15, 7, 3, 14, 0]);
        assert_eq!(g.get(3, 1), 0, "missing pixels are white");
        assert_eq!(g.mirror_h().get(2, 0), 15);
        assert_eq!(g.rotate_180().get(2, 1), 15);
        let s = g.shifted(1, 1);
        assert_eq!((s.get(2, 1), s.get(0, 0)), (15, 0));
    }

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

    #[test]
    fn mirror_rotate_shift() {
        let mut b = Bitmap1::new(4, 3);
        b.set(0, 0, true);
        assert!(b.mirror_h().get(3, 0));
        assert!(b.rotate_180().get(3, 2));
        let s = b.shifted(2, 1);
        assert!(s.get(2, 1));
        assert_eq!(s.count_black(), 1);
        assert_eq!(b.shifted(-1, 0).count_black(), 0);
        assert_eq!(b.shifted(4, 0).count_black(), 0);
    }
}
