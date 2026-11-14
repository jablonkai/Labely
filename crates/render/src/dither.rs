//! 8-bit gray → 1-bit conversion: threshold, Floyd–Steinberg, Atkinson and Bayer 8×8.
//!
//! All arithmetic is integer so results are identical on every platform.

use labely_core::document::Dither;

/// Classic 8×8 Bayer index matrix (values 0‥63).
const BAYER8: [[u8; 8]; 8] = [
    [0, 32, 8, 40, 2, 34, 10, 42],
    [48, 16, 56, 24, 50, 18, 58, 26],
    [12, 44, 4, 36, 14, 46, 6, 38],
    [60, 28, 52, 20, 62, 30, 54, 22],
    [3, 35, 11, 43, 1, 33, 9, 41],
    [51, 19, 59, 27, 49, 17, 57, 25],
    [15, 47, 7, 39, 13, 45, 5, 37],
    [63, 31, 55, 23, 61, 29, 53, 21],
];

/// Convert `gray` (row-major, 0 = black, 255 = white) to 1-bit; `true` means black.
///
/// `threshold` is the cut-off for [`Dither::Threshold`] and the gray point for the others
/// (128 = neutral). `gray.len()` must be `width * height`; extra/missing pixels are white.
pub fn dither(
    gray: &[u8],
    width: usize,
    height: usize,
    method: Dither,
    threshold: u8,
) -> Vec<bool> {
    dither_masked(gray, None, width, height, method, threshold)
}

/// Like [`dither`], but only pixels with `mask[i] == true` take part (others stay white and
/// receive no diffused error).
pub(crate) fn dither_masked(
    gray: &[u8],
    mask: Option<&[bool]>,
    width: usize,
    height: usize,
    method: Dither,
    threshold: u8,
) -> Vec<bool> {
    let n = width * height;
    let on = |i: usize| i < gray.len() && mask.is_none_or(|m| m.get(i).copied().unwrap_or(false));
    let mut out = vec![false; n];
    let t = i32::from(threshold);
    match method {
        Dither::Threshold => {
            for (i, o) in out.iter_mut().enumerate() {
                *o = on(i) && i32::from(gray[i]) < t;
            }
        }
        Dither::Bayer => {
            for y in 0..height {
                for x in 0..width {
                    let i = y * width + x;
                    if on(i) {
                        // Cell thresholds 2, 6, …, 254 shifted by the gray point.
                        let cell = i32::from(BAYER8[y % 8][x % 8]) * 4 + 2 + (t - 128);
                        out[i] = i32::from(gray[i]) < cell;
                    }
                }
            }
        }
        Dither::FloydSteinberg | Dither::Atkinson => {
            // Values in 1/16 gray units to keep fractions without floats.
            let mut v: Vec<i32> = (0..n)
                .map(|i| {
                    if i < gray.len() {
                        i32::from(gray[i]) * 16
                    } else {
                        255 * 16
                    }
                })
                .collect();
            let cut = t * 16;
            let fs: &[(isize, usize, i32)] = &[(1, 0, 7), (-1, 1, 3), (0, 1, 5), (1, 1, 1)];
            let atk: &[(isize, usize, i32)] = &[
                (1, 0, 1),
                (2, 0, 1),
                (-1, 1, 1),
                (0, 1, 1),
                (1, 1, 1),
                (0, 2, 1),
            ];
            let (taps, div) = if method == Dither::Atkinson {
                (atk, 8)
            } else {
                (fs, 16)
            };
            for y in 0..height {
                for x in 0..width {
                    let i = y * width + x;
                    if !on(i) {
                        continue;
                    }
                    let old = v[i];
                    let black = old < cut;
                    out[i] = black;
                    let err = old - if black { 0 } else { 255 * 16 };
                    for &(dx, dy, w) in taps {
                        let nx = x as isize + dx;
                        let ny = y + dy;
                        if nx < 0 || nx as usize >= width || ny >= height {
                            continue;
                        }
                        let j = ny * width + nx as usize;
                        if on(j) {
                            v[j] += err * w / div;
                        }
                    }
                }
            }
        }
    }
    out
}

/// Lookup table applying brightness, contrast (both −1‥1) and invert to a gray value.
pub(crate) fn adjust_lut(brightness: f64, contrast: f64, invert: bool) -> [u8; 256] {
    let b = if brightness.is_finite() {
        brightness.clamp(-1.0, 1.0)
    } else {
        0.0
    };
    let c = if contrast.is_finite() {
        contrast.clamp(-1.0, 1.0)
    } else {
        0.0
    };
    let f = if c >= 0.0 {
        (1.0 / (1.0 - c).max(1.0 / 255.0)).min(255.0)
    } else {
        1.0 + c
    };
    let mut lut = [0u8; 256];
    for (g, slot) in lut.iter_mut().enumerate() {
        let mut v = (g as f64 - 128.0) * f + 128.0 + b * 255.0;
        v = v.round().clamp(0.0, 255.0);
        if invert {
            v = 255.0 - v;
        }
        *slot = v as u8;
    }
    lut
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ramp(w: usize, h: usize) -> Vec<u8> {
        (0..w * h)
            .map(|i| ((i % w) * 255 / (w - 1)) as u8)
            .collect()
    }

    fn black_ratio(v: &[bool]) -> f64 {
        v.iter().filter(|b| **b).count() as f64 / v.len() as f64
    }

    #[test]
    fn threshold_cuts_at_value() {
        let g = [0, 127, 128, 255];
        assert_eq!(
            dither(&g, 4, 1, Dither::Threshold, 128),
            [true, true, false, false]
        );
        assert_eq!(dither(&g, 4, 1, Dither::Threshold, 0), [false; 4]);
    }

    #[test]
    fn solid_extremes_stay_solid() {
        for m in [
            Dither::Threshold,
            Dither::FloydSteinberg,
            Dither::Atkinson,
            Dither::Bayer,
        ] {
            assert!(
                dither(&[0; 64], 8, 8, m, 128).iter().all(|b| *b),
                "{m:?} black"
            );
            assert!(
                dither(&[255; 64], 8, 8, m, 128).iter().all(|b| !*b),
                "{m:?} white"
            );
        }
    }

    #[test]
    fn mid_gray_is_about_half_black() {
        let g = vec![128u8; 64 * 64];
        for m in [Dither::FloydSteinberg, Dither::Bayer] {
            let r = black_ratio(&dither(&g, 64, 64, m, 128));
            assert!((r - 0.5).abs() < 0.02, "{m:?}: {r}");
        }
        // Atkinson diffuses only 6/8 of the error, so it is lighter in mid-tones.
        let r = black_ratio(&dither(&g, 64, 64, Dither::Atkinson, 128));
        assert!((0.3..0.6).contains(&r), "atkinson: {r}");
    }

    #[test]
    fn ramp_darkness_follows_input() {
        let (w, h) = (64, 32);
        let g = ramp(w, h);
        for m in [Dither::FloydSteinberg, Dither::Atkinson, Dither::Bayer] {
            let out = dither(&g, w, h, m, 128);
            let col = |x: usize| (0..h).filter(|y| out[y * w + x]).count();
            let left: usize = (0..8).map(col).sum();
            let right: usize = (w - 8..w).map(col).sum();
            assert!(left > right * 4, "{m:?}: left {left} right {right}");
        }
    }

    #[test]
    fn bayer_pattern_is_ordered() {
        let out = dither(&[128; 64], 8, 8, Dither::Bayer, 128);
        // Exactly the cells with index >= 32 are black.
        for y in 0..8 {
            for x in 0..8 {
                assert_eq!(out[y * 8 + x], BAYER8[y][x] >= 32);
            }
        }
    }

    #[test]
    fn mask_excludes_pixels() {
        let mask = [true, false, true, false];
        let out = dither_masked(&[0; 4], Some(&mask), 4, 1, Dither::FloydSteinberg, 128);
        assert_eq!(out, [true, false, true, false]);
    }

    #[test]
    fn adjust_lut_behaviour() {
        let id = adjust_lut(0.0, 0.0, false);
        assert!(id.iter().enumerate().all(|(i, v)| usize::from(*v) == i));
        let inv = adjust_lut(0.0, 0.0, true);
        assert_eq!((inv[0], inv[255]), (255, 0));
        assert_eq!(adjust_lut(1.0, 0.0, false)[0], 255);
        let flat = adjust_lut(0.0, -1.0, false);
        assert_eq!((flat[0], flat[255]), (128, 128));
        let hard = adjust_lut(0.0, 1.0, false);
        assert_eq!((hard[127], hard[129]), (0, 255));
    }
}
