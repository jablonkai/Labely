//! Driver for printers that take a one-way byte stream: ESC/POS, Phomemo, Marklife, TSPL
//! and ZPL (docs/protocol-languages.md).
//!
//! These printers do not say which model they are, so the model comes from the address or
//! the Bluetooth name. A job is sent page by page; the printer does not confirm a page, so
//! a page counts as printed once its bytes are sent.

use std::time::{Duration, Instant};

use labely_core::Bitmap1;
use labely_core::document::MediaType;
use labely_core::services::{
    DeviceError, DeviceStatus, JobControl, PagePlanes, PrintReport, PrintRequest,
};
use labely_protocol::{RasterView, escpos, marklife, phomemo, tspl, zpl};
use labely_transport::{Transport, TransportError};
use tracing::debug;

use crate::driver::{Driver, Identity, prepare_for_head, union};
use crate::registry::Model;

/// How long a printer gets to answer a status request. Many do not answer at all.
const STATUS_TIMEOUT: Duration = Duration::from_millis(500);
/// Gap and black-mark height assumed by TSPL jobs; the paper carries no such number.
const TSPL_GAP_MM: f64 = 2.0;
/// How long a Marklife printer gets to report a page done. A printer that stays silent has
/// usually printed all the same, so the job goes on.
const MARKLIFE_DONE_TIMEOUT: Duration = Duration::from_secs(5);

/// The printer language of a model, from its `driver` and `task` registry fields.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Language {
    /// `escpos`; task `raster`, `raster_cut` (partial cut) or `raster_full_cut`.
    EscPos { cut: Option<escpos::Cut> },
    /// `phomemo`; task `m02`, `m110` or `d30`.
    Phomemo(phomemo::Family),
    /// `marklife`; task `l11`.
    MarklifeL11,
    /// `tspl`; task `bitmap`.
    Tspl,
    /// `zpl`; task `gfa`.
    Zpl,
}

impl Language {
    /// The language of `model`, or `None` if another driver handles it.
    pub fn of(model: &Model) -> Option<Self> {
        match (model.driver.as_str(), model.task.as_str()) {
            ("escpos", "raster") => Some(Self::EscPos { cut: None }),
            ("escpos", "raster_cut") => Some(Self::EscPos {
                cut: Some(escpos::Cut::Partial),
            }),
            ("escpos", "raster_full_cut") => Some(Self::EscPos {
                cut: Some(escpos::Cut::Full),
            }),
            ("phomemo", "m02") => Some(Self::Phomemo(phomemo::Family::M02)),
            ("phomemo", "m110") => Some(Self::Phomemo(phomemo::Family::M110)),
            ("phomemo", "d30") => Some(Self::Phomemo(phomemo::Family::D30)),
            ("marklife", "l11") => Some(Self::MarklifeL11),
            ("tspl", "bitmap") => Some(Self::Tspl),
            ("zpl", "gfa") => Some(Self::Zpl),
            _ => None,
        }
    }
}

/// A stream printer over any transport.
pub struct StreamDriver {
    transport: Box<dyn Transport>,
    model: Model,
    language: Language,
}

impl std::fmt::Debug for StreamDriver {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("StreamDriver")
            .field("transport", self.transport.info())
            .field("model", &self.model.id)
            .field("language", &self.language)
            .finish()
    }
}

fn io_err(e: TransportError) -> DeviceError {
    match e {
        TransportError::Disconnected => DeviceError::Unavailable("printer disconnected".into()),
        other => DeviceError::Other(other.to_string()),
    }
}

fn view(b: &Bitmap1) -> Result<RasterView<'_>, DeviceError> {
    RasterView::new(b.width(), b.height(), b.stride(), b.data())
        .map_err(|e| DeviceError::Other(e.to_string()))
}

impl StreamDriver {
    pub fn new(transport: Box<dyn Transport>, model: Model, language: Language) -> Self {
        Self {
            transport,
            model,
            language,
        }
    }

    /// Send `bytes` in transport-sized pieces, pausing as the transport asks.
    fn send(&mut self, bytes: &[u8]) -> Result<(), DeviceError> {
        let gap = self.transport.packet_gap();
        for chunk in bytes.chunks(self.transport.mtu().max(1)) {
            self.transport.send(chunk).map_err(io_err)?;
            if !gap.is_zero() {
                std::thread::sleep(gap);
            }
        }
        Ok(())
    }

    fn drain_input(&mut self) {
        while let Ok(Some(_)) = self.transport.recv(Duration::ZERO) {}
    }

    /// Send `request` and collect the reply until `done` accepts it or the time is up.
    fn query<T>(
        &mut self,
        request: &[u8],
        done: impl Fn(&[u8]) -> Option<T>,
    ) -> Result<Option<T>, DeviceError> {
        self.drain_input();
        self.send(request)?;
        self.wait_for(STATUS_TIMEOUT, done)
    }

    /// Collect received bytes until `done` accepts them or `timeout` is over.
    fn wait_for<T>(
        &mut self,
        timeout: Duration,
        done: impl Fn(&[u8]) -> Option<T>,
    ) -> Result<Option<T>, DeviceError> {
        let deadline = Instant::now() + timeout;
        let mut reply = Vec::new();
        loop {
            let left = deadline.saturating_duration_since(Instant::now());
            if left.is_zero() {
                return Ok(None);
            }
            if let Some(bytes) = self.transport.recv(left).map_err(io_err)? {
                reply.extend_from_slice(&bytes);
                if let Some(value) = done(&reply) {
                    return Ok(Some(value));
                }
            }
        }
    }

    /// The bytes of each step of the job, with the pages a step completes.
    fn steps(&self, job: &PrintRequest) -> Result<Vec<(Vec<u8>, u32)>, DeviceError> {
        let copies = job.copies.max(1);
        let density = job
            .density
            .clamp(self.model.density.min, self.model.density.max);
        let mut steps = Vec::new();
        for (i, page) in job.pages.iter().enumerate() {
            // These printers have one colour: red dots print black.
            let merged = match &job.planes {
                PagePlanes::Red(red) => red.get(i).map(|r| union(page, r)),
                PagePlanes::None | PagePlanes::Gray(_) => None,
            };
            let bitmap = prepare_for_head(merged.as_ref().unwrap_or(page), &self.model);
            let raster = view(&bitmap)?;
            match self.language {
                Language::EscPos { cut } => {
                    let settings = escpos::JobSettings {
                        cut,
                        ..escpos::JobSettings::default()
                    };
                    let bytes = escpos::page(&raster, &settings);
                    steps.extend((0..copies).map(|_| (bytes.clone(), 1)));
                }
                Language::Phomemo(family) => {
                    let settings = phomemo::JobSettings {
                        density,
                        media: match job.media {
                            MediaType::Gap | MediaType::Transparent => phomemo::Media::Gap,
                            MediaType::BlackMark => phomemo::Media::BlackMark,
                            MediaType::Continuous => phomemo::Media::Continuous,
                        },
                    };
                    let bytes = phomemo::page(family, &raster, &settings);
                    steps.extend((0..copies).map(|_| (bytes.clone(), 1)));
                }
                Language::MarklifeL11 => {
                    let settings = marklife::JobSettings {
                        density,
                        gap: job.media != MediaType::Continuous,
                    };
                    let bytes = marklife::page(&raster, &settings);
                    steps.extend((0..copies).map(|_| (bytes.clone(), 1)));
                }
                Language::Tspl => {
                    let settings = tspl::JobSettings {
                        dpi: self.model.dpi,
                        density,
                        media: match job.media {
                            MediaType::Gap | MediaType::Transparent => tspl::Media::Gap {
                                gap_mm: TSPL_GAP_MM,
                            },
                            MediaType::BlackMark => tspl::Media::BlackMark {
                                mark_mm: TSPL_GAP_MM,
                            },
                            MediaType::Continuous => tspl::Media::Continuous,
                        },
                        copies,
                    };
                    steps.push((tspl::label(&raster, &settings), u32::from(copies)));
                }
                Language::Zpl => {
                    let settings = zpl::JobSettings {
                        darkness: density,
                        media: match job.media {
                            MediaType::Gap | MediaType::Transparent => zpl::Media::Gap,
                            MediaType::BlackMark => zpl::Media::BlackMark,
                            MediaType::Continuous => zpl::Media::Continuous,
                        },
                        copies,
                    };
                    steps.push((zpl::label(&raster, &settings), u32::from(copies)));
                }
            }
        }
        Ok(steps)
    }
}

impl Driver for StreamDriver {
    fn connect(&mut self) -> Result<Identity, DeviceError> {
        self.drain_input();
        let name = self.transport.info().name.clone();
        debug!(model = %self.model.id, language = ?self.language, "connected");
        Ok(Identity {
            name: if name.is_empty() {
                self.model.name.clone()
            } else {
                name
            },
            model: Some(self.model.clone()),
            protocol: None,
            firmware: None,
            hardware: None,
            serial: None,
        })
    }

    /// The lid and paper flags, or all `None` from a printer that does not answer.
    fn status(&mut self) -> Result<DeviceStatus, DeviceError> {
        let (lid_closed, paper_present) = match self.language {
            Language::EscPos { .. } => {
                let last = |reply: &[u8]| reply.last().copied();
                let paper = self
                    .query(&escpos::status_request(escpos::STATUS_PAPER), last)?
                    .and_then(escpos::paper_present);
                let lid = match paper {
                    // A printer that ignored the first request will ignore the second.
                    None => None,
                    Some(_) => self
                        .query(&escpos::status_request(escpos::STATUS_OFFLINE), last)?
                        .and_then(escpos::cover_closed),
                };
                (lid, paper)
            }
            Language::Phomemo(_) => {
                let paper = self.query(&phomemo::PAPER_REQUEST, phomemo::paper_present)?;
                let lid = match paper {
                    None => None,
                    Some(_) => self.query(&phomemo::COVER_REQUEST, phomemo::cover_closed)?,
                };
                (lid, paper)
            }
            // No status request of this dialect is confirmed (§4).
            Language::MarklifeL11 => (None, None),
            Language::Tspl => {
                let status = self
                    .query(&tspl::STATUS_REQUEST, |r| r.last().copied())?
                    .map(tspl::Status::from_byte);
                (
                    status.map(|s| !s.head_open),
                    status.map(|s| !s.out_of_paper),
                )
            }
            Language::Zpl => {
                let status = self.query(zpl::STATUS_REQUEST, zpl::parse_status)?;
                (status.map(|s| !s.head_up), status.map(|s| !s.paper_out))
            }
        };
        Ok(DeviceStatus {
            battery_percent: None,
            lid_closed,
            paper_present,
            paper: None,
        })
    }

    fn print(
        &mut self,
        job: &PrintRequest,
        control: &JobControl,
    ) -> Result<PrintReport, DeviceError> {
        let started = Instant::now();
        let steps = self.steps(job)?;
        let total: u32 = steps.iter().map(|(_, pages)| pages).sum();
        control.set_progress(0, total);
        // The job itself gets no answer, so ask first whether the printer can print.
        let status = self.status()?;
        if status.lid_closed == Some(false) {
            return Err(DeviceError::Printer {
                message: "the cover is open".into(),
                hint: "close the cover and print again".into(),
            });
        }
        if status.paper_present == Some(false) {
            return Err(DeviceError::Printer {
                message: "out of paper".into(),
                hint: "load paper and print again".into(),
            });
        }
        let mut printed = 0;
        for (bytes, pages) in &steps {
            if control.is_cancelled() {
                return Err(DeviceError::Cancelled);
            }
            self.send(bytes)?;
            if self.language == Language::MarklifeL11 {
                let done = self.wait_for(MARKLIFE_DONE_TIMEOUT, |r| {
                    marklife::is_done(r).then_some(())
                })?;
                if done.is_none() {
                    debug!("no end-of-job byte from the printer");
                }
            }
            printed += pages;
            control.set_progress(printed, total);
        }
        Ok(PrintReport {
            pages_printed: printed,
            duration_ms: started.elapsed().as_millis() as u64,
        })
    }

    fn close(&mut self) {
        self.transport.close();
    }
}
