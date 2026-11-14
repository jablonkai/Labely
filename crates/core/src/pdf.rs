//! Minimal PDF writer for `export.pdf`: one page per 1-bit raster, sized to the label in mm.
//!
//! The rasters are the exact print output, so the PDF shows what the printer would burn. Each
//! page holds one image XObject (`DeviceGray`, 1 bit, run-length encoded).

use std::fmt::Write as _;

use crate::raster::Bitmap1;

/// PDF points per mm.
const PT_PER_MM: f64 = 72.0 / 25.4;

/// PDF RunLengthDecode encoding of `data`.
fn run_length(data: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(data.len() / 4 + 8);
    let mut i = 0;
    while i < data.len() {
        // A run of 2+ equal bytes, up to 128.
        let b = data[i];
        let mut run = 1;
        while i + run < data.len() && data[i + run] == b && run < 128 {
            run += 1;
        }
        if run >= 2 {
            out.push((257 - run) as u8);
            out.push(b);
            i += run;
            continue;
        }
        // Literal bytes until the next run of 2 equal bytes, up to 128.
        let start = i;
        while i < data.len() && i - start < 128 {
            if i + 1 < data.len() && data[i] == data[i + 1] {
                break;
            }
            i += 1;
        }
        out.push((i - start - 1) as u8);
        out.extend_from_slice(&data[start..i]);
    }
    out.push(128);
    out
}

/// Text string as UTF-16BE hex with a byte-order mark.
fn text_string(s: &str) -> String {
    let mut out = String::from("<FEFF");
    for u in s.encode_utf16() {
        let _ = write!(out, "{u:04X}");
    }
    out.push('>');
    out
}

/// Write a PDF with one `width_mm` × `height_mm` page per bitmap.
pub fn write_pdf(pages: &[Bitmap1], width_mm: f64, height_mm: f64, title: &str) -> Vec<u8> {
    let (pw, ph) = (width_mm * PT_PER_MM, height_mm * PT_PER_MM);
    let mut out: Vec<u8> = b"%PDF-1.4\n%\xE2\xE3\xCF\xD3\n".to_vec();
    let mut offsets: Vec<usize> = Vec::new();
    // Objects: 1 catalog, 2 pages, 3 info, then (page, content, image) per bitmap.
    let obj = |out: &mut Vec<u8>, offsets: &mut Vec<usize>, body: &[u8]| {
        offsets.push(out.len());
        out.extend_from_slice(format!("{} 0 obj\n", offsets.len()).as_bytes());
        out.extend_from_slice(body);
        out.extend_from_slice(b"\nendobj\n");
    };
    let page_id = |i: usize| 4 + i * 3;
    obj(&mut out, &mut offsets, b"<< /Type /Catalog /Pages 2 0 R >>");
    let kids: Vec<String> = (0..pages.len())
        .map(|i| format!("{} 0 R", page_id(i)))
        .collect();
    obj(
        &mut out,
        &mut offsets,
        format!(
            "<< /Type /Pages /Kids [{}] /Count {} >>",
            kids.join(" "),
            pages.len()
        )
        .as_bytes(),
    );
    obj(
        &mut out,
        &mut offsets,
        format!("<< /Title {} /Producer (Labely) >>", text_string(title)).as_bytes(),
    );
    for (i, b) in pages.iter().enumerate() {
        let (content_id, image_id) = (page_id(i) + 1, page_id(i) + 2);
        obj(
            &mut out,
            &mut offsets,
            format!(
                "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 {pw:.3} {ph:.3}] \
                 /Resources << /XObject << /Im0 {image_id} 0 R >> >> /Contents {content_id} 0 R >>"
            )
            .as_bytes(),
        );
        let content = format!("q {pw:.3} 0 0 {ph:.3} 0 0 cm /Im0 Do Q");
        obj(
            &mut out,
            &mut offsets,
            format!(
                "<< /Length {} >>\nstream\n{content}\nendstream",
                content.len()
            )
            .as_bytes(),
        );
        let data = run_length(b.data());
        let mut body = format!(
            "<< /Type /XObject /Subtype /Image /Width {} /Height {} /ColorSpace /DeviceGray \
             /BitsPerComponent 1 /Decode [1 0] /Interpolate false /Filter /RunLengthDecode \
             /Length {} >>\nstream\n",
            b.width(),
            b.height(),
            data.len()
        )
        .into_bytes();
        body.extend_from_slice(&data);
        body.extend_from_slice(b"\nendstream");
        obj(&mut out, &mut offsets, &body);
    }
    let xref = out.len();
    let mut tail = format!("xref\n0 {}\n0000000000 65535 f \n", offsets.len() + 1);
    for o in &offsets {
        let _ = writeln!(tail, "{o:010} 00000 n ");
    }
    let _ = write!(
        tail,
        "trailer\n<< /Size {} /Root 1 0 R /Info 3 0 R >>\nstartxref\n{xref}\n%%EOF\n",
        offsets.len() + 1
    );
    out.extend_from_slice(tail.as_bytes());
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn decode(rle: &[u8]) -> Vec<u8> {
        let mut out = Vec::new();
        let mut i = 0;
        while i < rle.len() {
            let l = rle[i];
            i += 1;
            match l {
                128 => break,
                0..=127 => {
                    let n = usize::from(l) + 1;
                    out.extend_from_slice(&rle[i..i + n]);
                    i += n;
                }
                _ => {
                    out.extend(std::iter::repeat_n(rle[i], 257 - usize::from(l)));
                    i += 1;
                }
            }
        }
        out
    }

    #[test]
    fn run_length_roundtrip() {
        let mut data = vec![0u8; 300];
        data.extend([1, 2, 3, 3, 4]);
        data.extend((0..=255u8).cycle().take(400));
        data.push(9);
        for input in [&data[..], &[], &[7], &[7, 7]] {
            let enc = run_length(input);
            assert_eq!(enc.last(), Some(&128));
            assert_eq!(decode(&enc), input);
        }
        assert!(run_length(&[0; 1000]).len() < 20);
    }

    #[test]
    fn structure_and_xref() {
        let mut a = Bitmap1::new(16, 8);
        a.set(3, 3, true);
        let pdf = write_pdf(&[a.clone(), a], 50.0, 30.0, "Árvíztűrő (test)");
        let s = String::from_utf8_lossy(&pdf);
        assert!(s.starts_with("%PDF-1.4"));
        assert!(s.ends_with("%%EOF\n"));
        assert!(s.contains("/Count 2"));
        assert!(s.contains("/MediaBox [0 0 141.732 85.039]"));
        // Every xref offset points at its object header.
        let find = |needle: &[u8]| {
            pdf.windows(needle.len())
                .rposition(|w| w == needle)
                .unwrap()
        };
        let tail = std::str::from_utf8(&pdf[find(b"startxref\n")..]).unwrap();
        let start: usize = tail.lines().nth(1).unwrap().parse().unwrap();
        let xref = std::str::from_utf8(&pdf[start..]).unwrap();
        assert!(xref.starts_with("xref"));
        for (n, line) in xref.lines().skip(3).take(9).enumerate() {
            let off: usize = line[..10].parse().unwrap();
            assert!(
                pdf[off..].starts_with(format!("{} 0 obj", n + 1).as_bytes()),
                "object {}",
                n + 1
            );
        }
    }
}
