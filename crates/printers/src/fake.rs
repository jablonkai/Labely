//! FakePrinter: "prints" each page to a PNG plus a `job.json` describing the job.

use std::path::{Path, PathBuf};
use std::time::Instant;

use labely_core::Bitmap1;
use labely_core::services::{DeviceError, DeviceStatus, PrintReport, PrintRequest};
use serde::Serialize;

use crate::driver::{Driver, Identity, Progress, prepare_for_head};
use crate::registry::Model;

/// Writes `<dir>/job-NNNN/page-NN.png` (head orientation, as a real printer would receive
/// it) and `job.json`.
#[derive(Debug)]
pub struct FakePrinter {
    dir: PathBuf,
    model: Model,
    jobs: u32,
}

#[derive(Serialize)]
struct JobRecord<'a> {
    model: &'a str,
    copies: u16,
    density: u8,
    media: labely_core::document::MediaType,
    pages: Vec<PageRecord>,
}

#[derive(Serialize)]
struct PageRecord {
    file: String,
    width_dots: u32,
    height_dots: u32,
    black_dots: u64,
}

impl FakePrinter {
    pub fn new(dir: impl Into<PathBuf>, model: Model) -> Self {
        Self {
            dir: dir.into(),
            model,
            jobs: 0,
        }
    }

    pub fn model(&self) -> &Model {
        &self.model
    }
}

fn io(e: impl std::fmt::Display) -> DeviceError {
    DeviceError::Other(format!("fake printer: {e}"))
}

/// Encode a 1-bit bitmap as a grayscale-1 PNG (black = 0).
pub fn write_png(path: &Path, bitmap: &Bitmap1) -> Result<(), DeviceError> {
    let file = std::fs::File::create(path).map_err(io)?;
    let mut enc = png::Encoder::new(
        std::io::BufWriter::new(file),
        bitmap.width(),
        bitmap.height(),
    );
    enc.set_color(png::ColorType::Grayscale);
    enc.set_depth(png::BitDepth::One);
    let mut w = enc.write_header().map_err(io)?;
    // PNG 1-bit: 1 = white. Bitmap1: 1 = black. Invert.
    let inverted: Vec<u8> = bitmap.data().iter().map(|b| !b).collect();
    w.write_image_data(&inverted).map_err(io)?;
    w.finish().map_err(io)
}

impl Driver for FakePrinter {
    fn connect(&mut self) -> Result<Identity, DeviceError> {
        std::fs::create_dir_all(&self.dir).map_err(io)?;
        Ok(Identity {
            name: format!("FakePrinter ({})", self.model.name),
            model: Some(self.model.clone()),
            protocol: None,
        })
    }

    fn status(&mut self) -> Result<DeviceStatus, DeviceError> {
        Ok(DeviceStatus {
            battery_percent: Some(100),
            lid_closed: Some(true),
            paper_present: Some(true),
            paper: None,
        })
    }

    fn print(
        &mut self,
        job: &PrintRequest,
        progress: &mut dyn FnMut(Progress),
    ) -> Result<PrintReport, DeviceError> {
        let started = Instant::now();
        self.jobs += 1;
        let job_dir = self.dir.join(format!("job-{:04}", self.jobs));
        std::fs::create_dir_all(&job_dir).map_err(io)?;
        let mut pages = Vec::new();
        for (i, page) in job.pages.iter().enumerate() {
            let head = prepare_for_head(page, &self.model);
            let file = format!("page-{:02}.png", i + 1);
            write_png(&job_dir.join(&file), &head)?;
            pages.push(PageRecord {
                file,
                width_dots: head.width(),
                height_dots: head.height(),
                black_dots: head.count_black(),
            });
        }
        let total = job.pages.len() as u32 * u32::from(job.copies.max(1));
        let record = JobRecord {
            model: &self.model.id,
            copies: job.copies,
            density: job.density,
            media: job.media,
            pages,
        };
        let json = serde_json::to_vec_pretty(&record).map_err(io)?;
        std::fs::write(job_dir.join("job.json"), json).map_err(io)?;
        progress(Progress {
            pages_done: total,
            pages_total: total,
        });
        Ok(PrintReport {
            pages_printed: total,
            duration_ms: started.elapsed().as_millis() as u64,
        })
    }

    fn close(&mut self) {}
}
