//! Raster row encoding (docs/protocol.md §7). Emits only 0x84 (blank runs) and 0x85
//! (bitmap rows with repeat); never 0x83 (ADR-0010).

use serde::{Deserialize, Serialize};

use super::{Packet, PacketError, cmd};
use crate::RasterView;

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

/// Encode all rows of `raster` as one-way packets in feed order.
///
/// Blank runs become 0x84 (≤255 rows each); identical consecutive non-blank rows are
/// merged with the 0x85 repeat byte (≤255).
pub fn encode_rows(raster: &RasterView<'_>, mode: CountMode) -> Result<Vec<Packet>, PacketError> {
    let mut out = Vec::new();
    let height = raster.height();
    let mut y = 0u32;
    while y < height {
        let row = raster.row(y);
        let blank = row.iter().all(|b| *b == 0);
        let mut run = 1u32;
        while y + run < height && run < 255 && raster.row(y + run) == row {
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
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
