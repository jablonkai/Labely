//! Raster row encoding (docs/protocol.md §7). Single-colour jobs emit only 0x84 (blank
//! runs) and 0x85 (bitmap rows with repeat); never 0x83 (ADR-0010). Two-colour jobs wrap
//! their rows in 0x8A, and grayscale jobs send 0x8D rows (§7.5).

use serde::{Deserialize, Serialize};

use super::{Packet, PacketError, cmd, request};
use crate::{GrayView, RasterView};

/// How the three "count" bytes of a 0x85 row are filled (§7.3).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CountMode {
    /// Black pixels in dot ranges 0–191, 192–383, 384+ (vendor rule; equals the OSS split
    /// rule for 591-dot heads).
    #[default]
    Groups192,
    /// `[0][low][high]` of the total black-pixel count.
    Total,
    /// All zero (reported to work on some models).
    Zero,
}

fn counts(row: &[u8], mode: CountMode) -> [u8; 3] {
    let mut groups = [0u32; 3];
    for (i, byte) in row.iter().enumerate() {
        let g = ((i * 8) / 192).min(2);
        groups[g] += byte.count_ones();
    }
    match mode {
        CountMode::Groups192 => groups.map(|n| n.min(255) as u8),
        CountMode::Total => {
            let total = groups.iter().sum::<u32>().min(u32::from(u16::MAX)) as u16;
            let [hi, lo] = total.to_be_bytes();
            [0, lo, hi]
        }
        CountMode::Zero => [0; 3],
    }
}

/// Rows between two check-lines of the B21 tasks (§7).
pub const CHECK_LINE_INTERVAL: u32 = 200;

/// Encode all rows of `raster` as one-way packets in feed order.
///
/// Blank runs become 0x84 (≤255 rows each); identical consecutive non-blank rows are
/// merged with the 0x85 repeat byte (≤255).
pub fn encode_rows(raster: &RasterView<'_>, mode: CountMode) -> Result<Vec<Packet>, PacketError> {
    encode_rows_checked(raster, mode, None)
}

/// As [`encode_rows`]. With `check_every = Some(n)`, a check-line (0x86) follows every
/// `n`th row, and no run crosses one. The printer answers each check-line, so the caller
/// must wait for the reply.
pub fn encode_rows_checked(
    raster: &RasterView<'_>,
    mode: CountMode,
    check_every: Option<u32>,
) -> Result<Vec<Packet>, PacketError> {
    let mut out = Vec::new();
    let height = raster.height();
    let check_every = check_every.filter(|n| *n > 0);
    let mut y = 0u32;
    while y < height {
        let row = raster.row(y);
        let blank = row.iter().all(|b| *b == 0);
        // Rows left before the next check-line, this one included.
        let limit = check_every.map_or(255, |n| (n - y % n).min(255));
        let mut run = 1u32;
        while y + run < height && run < limit && raster.row(y + run) == row {
            run += 1;
        }
        let [hi, lo] = (y as u16).to_be_bytes();
        if blank {
            out.push(Packet::new(cmd::ROWS_EMPTY, vec![hi, lo, run as u8])?);
        } else {
            let c = counts(&row, mode);
            let mut payload = Vec::with_capacity(6 + row.len());
            payload.extend_from_slice(&[hi, lo, c[0], c[1], c[2], run as u8]);
            payload.extend_from_slice(&row);
            out.push(Packet::new(cmd::ROW_BITMAP, payload)?);
        }
        y += run;
        if check_every.is_some_and(|n| y.is_multiple_of(n)) {
            out.push(request::check_line((y - 1) as u16));
        }
    }
    Ok(out)
}

/// First byte of a two-colour row (0x8A).
mod plane {
    pub const EMPTY: u8 = 0;
    pub const RED: u8 = 1;
    pub const BLACK: u8 = 2;
    pub const MIXED: u8 = 3;
}

/// Encode a two-colour page as 0x8A packets in feed order (§7.5). `black` and `red` must
/// have the same size; a dot set in both planes prints red.
///
/// - blank rows: `[0][84][row u16][count]`
/// - rows of one colour: `[1 red / 2 black][85][row u16][repeat][row bytes]`
/// - rows of both: `[3][row u16][repeat][mask bytes][colour bytes]`. The mask has a bit for
///   every dot that prints. A colour byte follows for each non-zero mask byte, with a bit
///   set for every red dot.
///
/// Identical consecutive rows are merged (≤255). The rows carry no counts.
pub fn encode_rows_two_color(
    black: &RasterView<'_>,
    red: &RasterView<'_>,
) -> Result<Vec<Packet>, PacketError> {
    let mut out = Vec::new();
    let height = black.height().min(red.height());
    let mut y = 0u32;
    while y < height {
        let (b, r) = (black.row(y), red.row(y));
        let mut run = 1u32;
        while y + run < height && run < 255 && black.row(y + run) == b && red.row(y + run) == r {
            run += 1;
        }
        let [hi, lo] = (y as u16).to_be_bytes();
        let has_black = b.iter().any(|v| *v != 0);
        let has_red = r.iter().any(|v| *v != 0);
        let payload = match (has_black, has_red) {
            (false, false) => vec![plane::EMPTY, cmd::ROWS_EMPTY, hi, lo, run as u8],
            (true, true) => {
                let mut p = vec![plane::MIXED, hi, lo, run as u8];
                p.extend(b.iter().zip(&r).map(|(b, r)| b | r));
                p.extend(
                    b.iter()
                        .zip(&r)
                        .filter(|(b, r)| (**b | **r) != 0)
                        .map(|(_, r)| *r),
                );
                p
            }
            (only_black, _) => {
                let (mode, row) = if only_black {
                    (plane::BLACK, &b)
                } else {
                    (plane::RED, &r)
                };
                let mut p = vec![mode, cmd::ROW_BITMAP, hi, lo, run as u8];
                p.extend_from_slice(row);
                p
            }
        };
        out.push(Packet::new(cmd::ROW_COLOR, payload)?);
        y += run;
    }
    Ok(out)
}

/// One decoded 0x8A packet: the rows it covers and their two planes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ColorRow {
    pub row: u16,
    pub repeat: u8,
    /// Row bytes of the black plane; empty for a blank row.
    pub black: Vec<u8>,
    /// Row bytes of the red plane; empty for a blank row.
    pub red: Vec<u8>,
}

/// Decode the payload of a 0x8A packet for a page `row_bytes` wide (the simulator and the
/// tests use it). `None` for a payload that does not follow §7.5.
pub fn decode_color_row(payload: &[u8], row_bytes: usize) -> Option<ColorRow> {
    let blank = || vec![0u8; row_bytes];
    let fit = |data: &[u8]| {
        let mut v = data.to_vec();
        v.resize(row_bytes, 0);
        v
    };
    match payload {
        [plane::EMPTY, cmd::ROWS_EMPTY, hi, lo, count] => Some(ColorRow {
            row: u16::from_be_bytes([*hi, *lo]),
            repeat: *count,
            black: Vec::new(),
            red: Vec::new(),
        }),
        [
            mode @ (plane::RED | plane::BLACK),
            cmd::ROW_BITMAP,
            hi,
            lo,
            repeat,
            data @ ..,
        ] => {
            let (black, red) = if *mode == plane::BLACK {
                (fit(data), blank())
            } else {
                (blank(), fit(data))
            };
            Some(ColorRow {
                row: u16::from_be_bytes([*hi, *lo]),
                repeat: *repeat,
                black,
                red,
            })
        }
        [plane::MIXED, hi, lo, repeat, rest @ ..] => {
            let mask = rest.get(..row_bytes)?;
            let mut colors = rest[row_bytes..].iter();
            let (mut black, mut red) = (blank(), blank());
            for (i, m) in mask.iter().enumerate() {
                if *m != 0 {
                    let c = *colors.next()?;
                    red[i] = m & c;
                    black[i] = m & !c;
                }
            }
            Some(ColorRow {
                row: u16::from_be_bytes([*hi, *lo]),
                repeat: *repeat,
                black,
                red,
            })
        }
        _ => None,
    }
}

/// First byte of a grayscale row (0x8D).
mod gray {
    pub const VOID: u8 = 0x84;
    pub const DATA: u8 = 0x86;
    /// Longest run of dots after the two in the data byte.
    pub const MAX_RUN: usize = 252;
    /// Shortest such run worth a descriptor.
    pub const MIN_RUN: usize = 6;
    /// Most descriptor bytes: their count is one byte.
    pub const MAX_RUN_BYTES: usize = 252;
}

fn level(v: u8) -> u8 {
    v.min(15)
}

/// Pack a row of levels two per byte (high nibble first) and describe its runs (§7.5).
///
/// A run starts at a data byte whose two dots are equal and covers the equal dots after
/// it. Its descriptor is `(dots + 2) | bit 8 of the data position`, then the low byte of
/// the position; `dots` is even. The dots of a run are left out of the data. Runs of fewer
/// than 8 dots, and runs at a position above 511, are sent as plain data.
fn compress_gray_row(levels: &[u8]) -> (Vec<u8>, Vec<u8>) {
    let mut runs = Vec::new();
    let mut data = Vec::with_capacity(levels.len() / 2);
    let mut x = 0;
    while x < levels.len() {
        let a = level(levels[x]);
        let b = levels.get(x + 1).map_or(0, |v| level(*v));
        let at = data.len();
        data.push((a << 4) | b);
        x += 2;
        if a != b || runs.len() + 2 > gray::MAX_RUN_BYTES || at > 0x1FF {
            continue;
        }
        let same = levels[x.min(levels.len())..]
            .iter()
            .take(gray::MAX_RUN)
            .take_while(|v| level(**v) == b)
            .count()
            & !1;
        if same < gray::MIN_RUN {
            continue;
        }
        runs.push((same + 2) as u8 | ((at >> 8) & 1) as u8);
        runs.push(at as u8);
        x += same;
    }
    (runs, data)
}

/// Undo [`compress_gray_row`]: the levels of a row from its run descriptors and data.
pub fn expand_gray_row(runs: &[u8], data: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(data.len() * 2);
    let mut runs = runs.as_chunks::<2>().0.iter().peekable();
    for (i, byte) in data.iter().enumerate() {
        out.push(byte >> 4);
        out.push(byte & 0x0F);
        if let Some(run) = runs.peek()
            && ((usize::from(run[0] & 1) << 8) | usize::from(run[1])) == i
        {
            let extra = usize::from(run[0] & 0xFE).saturating_sub(2);
            out.extend(std::iter::repeat_n(byte & 0x0F, extra));
            runs.next();
        }
    }
    out
}

/// Encode a grayscale page as 0x8D packets in feed order (§7.5).
///
/// - blank rows: `[84][row u16][count][00]`
/// - other rows: `[86][row u16][repeat][run bytes n][n run bytes][data]`
///
/// Identical consecutive rows are merged (≤255).
pub fn encode_rows_gray(raster: &GrayView<'_>) -> Result<Vec<Packet>, PacketError> {
    let mut out = Vec::new();
    let height = raster.height();
    let same = |a: &[u8], b: &[u8]| a.iter().zip(b).all(|(a, b)| level(*a) == level(*b));
    let mut y = 0u32;
    while y < height {
        let row = raster.row(y);
        let mut run = 1u32;
        while y + run < height && run < 255 && same(raster.row(y + run), row) {
            run += 1;
        }
        let [hi, lo] = (y as u16).to_be_bytes();
        let payload = if row.iter().all(|v| *v == 0) {
            vec![gray::VOID, hi, lo, run as u8, 0]
        } else {
            let (runs, data) = compress_gray_row(row);
            let mut p = Vec::with_capacity(5 + runs.len() + data.len());
            p.extend_from_slice(&[gray::DATA, hi, lo, run as u8, runs.len() as u8]);
            p.extend_from_slice(&runs);
            p.extend_from_slice(&data);
            p
        };
        out.push(Packet::new(cmd::ROW_GRAY, payload)?);
        y += run;
    }
    Ok(out)
}

/// One decoded 0x8D packet: the rows it covers and their levels (empty for blank rows).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GrayRow {
    pub row: u16,
    pub repeat: u8,
    pub levels: Vec<u8>,
}

/// Decode the payload of a 0x8D packet. `None` for a payload that does not follow §7.5.
pub fn decode_gray_row(payload: &[u8]) -> Option<GrayRow> {
    match payload {
        [gray::VOID, hi, lo, count, ..] => Some(GrayRow {
            row: u16::from_be_bytes([*hi, *lo]),
            repeat: *count,
            levels: Vec::new(),
        }),
        [gray::DATA, hi, lo, repeat, n, rest @ ..] => {
            let runs = rest.get(..usize::from(*n))?;
            Some(GrayRow {
                row: u16::from_be_bytes([*hi, *lo]),
                repeat: *repeat,
                levels: expand_gray_row(runs, &rest[usize::from(*n)..]),
            })
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    #[test]
    fn two_color_rows() {
        // 16 dots wide, 6 rows: blank, blank, black, black, red, both.
        let black = [0, 0, 0, 0, 0xF0, 0x01, 0xF0, 0x01, 0, 0, 0xC0, 0x00];
        let red = [0, 0, 0, 0, 0, 0, 0, 0, 0x0F, 0x00, 0x30, 0x00];
        let b = RasterView::new(16, 6, 2, &black).unwrap();
        let r = RasterView::new(16, 6, 2, &red).unwrap();
        let p = encode_rows_two_color(&b, &r).unwrap();
        assert!(p.iter().all(|p| p.cmd == 0x8A));
        let payloads: Vec<&[u8]> = p.iter().map(|p| p.payload.as_slice()).collect();
        assert_eq!(
            payloads,
            vec![
                &[0, 0x84, 0, 0, 2][..],
                &[2, 0x85, 0, 2, 2, 0xF0, 0x01],
                &[1, 0x85, 0, 4, 1, 0x0F, 0x00],
                // Mask F0 00, then one colour byte for the non-zero mask byte.
                &[3, 0, 5, 1, 0xF0, 0x00, 0x30],
            ]
        );
        let last = decode_color_row(payloads[3], 2).unwrap();
        assert_eq!((last.black, last.red), (vec![0xC0, 0], vec![0x30, 0]));
        assert_eq!(decode_color_row(&[9, 9], 2), None);
    }

    #[test]
    fn gray_rows() {
        // 8 dots wide: blank, blank, a ramp, a ramp.
        let mut data = vec![0u8; 16];
        data.extend_from_slice(&[0, 1, 2, 3, 4, 5, 6, 15]);
        data.extend_from_slice(&[0, 1, 2, 3, 4, 5, 6, 200]);
        let g = GrayView::new(8, 4, &data).unwrap();
        let p = encode_rows_gray(&g).unwrap();
        assert!(p.iter().all(|p| p.cmd == 0x8D));
        assert_eq!(p[0].payload, vec![0x84, 0, 0, 2, 0]);
        assert_eq!(p[1].payload, vec![0x86, 0, 2, 2, 0, 0x01, 0x23, 0x45, 0x6F]);
    }

    #[test]
    fn gray_runs() {
        // 2 dots of 3, then 20 dots of 9, then 2 dots of 0.
        let mut row = vec![3, 3];
        row.extend_from_slice(&[9; 20]);
        row.extend_from_slice(&[0, 0]);
        let (runs, data) = compress_gray_row(&row);
        // The first byte is not a run (only 0 equal dots follow). The second one covers
        // itself and 18 more dots.
        assert_eq!(data, vec![0x33, 0x99, 0x00]);
        assert_eq!(runs, vec![20, 1]);
        assert_eq!(expand_gray_row(&runs, &data), row);
        // 7 equal dots after the pair round down to 6: still a run.
        let (runs, _) = compress_gray_row(&[5; 9]);
        assert_eq!(runs, vec![8, 0]);
        // 4 equal dots after the pair are sent as data.
        assert_eq!(compress_gray_row(&[5; 6]).0, Vec::<u8>::new());
    }

    #[test]
    fn gray_run_position_uses_bit_8() {
        let mut row: Vec<u8> = (0..600).map(|i| (i % 2) as u8).collect();
        row.extend_from_slice(&[7; 40]);
        let (runs, data) = compress_gray_row(&row);
        // Data byte 300 = 0x12C: bit 8 goes into the descriptor, 0x2C follows.
        assert_eq!(runs, vec![40 | 1, 0x2C]);
        assert_eq!(expand_gray_row(&runs, &data), row);
    }

    proptest! {
        #[test]
        fn gray_row_roundtrips(
            parts in proptest::collection::vec((0u8..16, 1usize..40), 1..60),
        ) {
            let mut row: Vec<u8> = parts
                .iter()
                .flat_map(|(v, n)| std::iter::repeat_n(*v, *n))
                .collect();
            if row.len() % 2 == 1 {
                row.push(0);
            }
            let (runs, data) = compress_gray_row(&row);
            prop_assert!(runs.len() <= 252);
            prop_assert_eq!(expand_gray_row(&runs, &data), row);
        }

        #[test]
        fn two_color_roundtrips(
            rows in proptest::collection::vec((any::<[u8; 3]>(), any::<[u8; 3]>(), 0u8..3), 1..40),
        ) {
            // Kind 0 is a blank row, 1 a black row, 2 a row of both.
            let mut black = Vec::new();
            let mut red = Vec::new();
            for (b, r, kind) in &rows {
                let b = if *kind == 0 { [0; 3] } else { *b };
                let r = if *kind == 2 { *r } else { [0; 3] };
                // A dot of both planes prints red.
                black.extend(b.iter().zip(&r).map(|(b, r)| b & !r));
                red.extend_from_slice(&r);
            }
            let h = rows.len() as u32;
            let bv = RasterView::new(24, h, 3, &black).unwrap();
            let rv = RasterView::new(24, h, 3, &red).unwrap();
            let (mut got_b, mut got_r) = (vec![0u8; black.len()], vec![0u8; red.len()]);
            for p in encode_rows_two_color(&bv, &rv).unwrap() {
                let row = decode_color_row(&p.payload, 3).unwrap();
                if row.black.is_empty() {
                    continue;
                }
                for y in usize::from(row.row)..usize::from(row.row) + usize::from(row.repeat) {
                    got_b[y * 3..y * 3 + 3].copy_from_slice(&row.black);
                    got_r[y * 3..y * 3 + 3].copy_from_slice(&row.red);
                }
            }
            prop_assert_eq!(got_b, black);
            prop_assert_eq!(got_r, red);
        }
    }

    #[test]
    fn blank_and_repeat_runs() {
        // 16 dots wide, 5 rows: blank, blank, A, A, B
        let data = [0, 0, 0, 0, 0x80, 0x01, 0x80, 0x01, 0xFF, 0x00];
        let r = RasterView::new(16, 5, 2, &data).unwrap();
        let p = encode_rows(&r, CountMode::Groups192).unwrap();
        assert_eq!(p.len(), 3);
        assert_eq!(p[0], Packet::small(0x84, &[0, 0, 2]));
        assert_eq!(p[1], Packet::small(0x85, &[0, 2, 2, 0, 0, 2, 0x80, 0x01]));
        assert_eq!(p[2], Packet::small(0x85, &[0, 4, 8, 0, 0, 1, 0xFF, 0x00]));
    }

    #[test]
    fn groups_split_at_192() {
        let mut row = vec![0u8; 74];
        row[0] = 0xFF; // dots 0-7 -> group 0
        row[24] = 0x0F; // dots 192-199 -> group 1
        row[73] = 0x01; // dot 591 -> group 2
        assert_eq!(counts(&row, CountMode::Groups192), [8, 4, 1]);
        assert_eq!(counts(&row, CountMode::Total), [0, 13, 0]);
    }

    #[test]
    fn long_runs_split_at_255() {
        let data = vec![0u8; 300];
        let r = RasterView::new(8, 300, 1, &data).unwrap();
        let p = encode_rows(&r, CountMode::Zero).unwrap();
        assert_eq!(p.len(), 2);
        assert_eq!(p[1].payload, vec![0, 255, 45]);
    }

    #[test]
    fn check_lines_split_runs() {
        // 450 identical rows: runs of 200, 200 and 50, with a check-line after rows 199 and 399.
        let data = vec![0x80u8; 450];
        let r = RasterView::new(8, 450, 1, &data).unwrap();
        let p = encode_rows_checked(&r, CountMode::Total, Some(CHECK_LINE_INTERVAL)).unwrap();
        let cmds: Vec<u8> = p.iter().map(|p| p.cmd).collect();
        assert_eq!(cmds, vec![0x85, 0x86, 0x85, 0x86, 0x85]);
        assert_eq!(p[0].payload[..6], [0, 0, 0, 1, 0, 200]);
        assert_eq!(p[1].payload, vec![0, 199, 1]);
        assert_eq!(p[2].payload[..2], [0, 200]);
        assert_eq!(p[3].payload, vec![0x01, 0x8F, 1]);
        assert_eq!(p[4].payload[..6], [0x01, 0x90, 0, 1, 0, 50]);
    }
}
