//! Print-job programs (docs/protocol.md §5, §12). A job is a list of [`Step`]s that a
//! driver executes against a transport. Keeping it as data makes every variant testable
//! byte-for-byte without hardware.

use serde::{Deserialize, Serialize};
use thiserror::Error;

use super::request::{self, ColorMode, HeartbeatType, LabelType, PageSizeFormat, PrintStart};
use super::rows::{CountMode, encode_rows};
use super::{Packet, PacketError, cmd};
use crate::RasterView;

/// Print-task variant (docs/protocol.md §5.2). Selected per model in the registry.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PrintTask {
    /// 1-byte PrintStart, PrintClear, PageStart, 4-byte size, SetQuantity (B21S, D110, D11 v2).
    D110,
    /// 7-byte PrintStart, PageStart, 6-byte size (B1, D110_M, B21_C2B, N1).
    B1,
    /// 9-byte PrintStart, one-way status, 13-byte size, no PageStart (B21 Pro, B1 Pro, D11_H).
    D110mV4,
}

/// How many times to repeat a request whose boolean reply is `false`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Retry {
    Once,
    /// Re-send until the reply is true, at most `max` attempts, `delay_ms` apart.
    UntilTrue {
        max: u16,
        delay_ms: u32,
    },
}

/// One driver action.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Step {
    /// Send and wait for `expect`; a false boolean reply fails the job (or retries).
    Exchange {
        packet: Packet,
        expect: u8,
        retry: Retry,
    },
    /// Send without waiting for a reply.
    Send(Packet),
    /// Poll PrintStatus (0xA3) until `page >= total_pages`.
    WaitPages {
        total_pages: u16,
        interval_ms: u32,
        stall_timeout_ms: u32,
    },
}

impl Step {
    fn exchange(packet: Packet) -> Self {
        let expect = request::expected_response(packet.cmd, packet.payload.first().copied())
            .unwrap_or(cmd::NOT_SUPPORTED);
        Step::Exchange {
            packet,
            expect,
            retry: Retry::Once,
        }
    }

    fn exchange_until_true(packet: Packet) -> Self {
        match Self::exchange(packet) {
            Step::Exchange { packet, expect, .. } => Step::Exchange {
                packet,
                expect,
                retry: Retry::UntilTrue {
                    max: 50,
                    delay_ms: 100,
                },
            },
            other => other,
        }
    }

    /// Bytes this step puts on the wire (for fixtures and tests).
    pub fn wire_bytes(&self) -> Option<Vec<u8>> {
        match self {
            Step::Exchange { packet, .. } | Step::Send(packet) => Some(packet.encode_request()),
            Step::WaitPages { .. } => None,
        }
    }
}

/// Job settings shared by all pages.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct JobSettings {
    pub density: u8,
    pub label_type: LabelType,
    /// Copies of each page.
    pub copies: u16,
    pub count_mode: CountMode,
    /// PrintStart V9 byte 7 (speed/quality). UNCERTAIN on hardware; default 0.
    pub quality: u8,
}

impl Default for JobSettings {
    fn default() -> Self {
        Self {
            density: 3,
            label_type: LabelType::Gap,
            copies: 1,
            count_mode: CountMode::default(),
            quality: 0,
        }
    }
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum JobError {
    #[error("no pages to print")]
    Empty,
    #[error("copies must be at least 1")]
    NoCopies,
    #[error("page {page} is {dim} = {value} dots, above the 65535 limit")]
    TooLarge {
        page: usize,
        dim: &'static str,
        value: u32,
    },
    #[error("total page count exceeds 65535")]
    TooManyPages,
    #[error(transparent)]
    Packet(#[from] PacketError),
}

/// Build the step program for printing `pages` with `task`.
pub fn build(
    task: PrintTask,
    settings: &JobSettings,
    pages: &[RasterView<'_>],
) -> Result<Vec<Step>, JobError> {
    if pages.is_empty() {
        return Err(JobError::Empty);
    }
    if settings.copies == 0 {
        return Err(JobError::NoCopies);
    }
    let total = (pages.len() as u32)
        .checked_mul(u32::from(settings.copies))
        .and_then(|t| u16::try_from(t).ok())
        .ok_or(JobError::TooManyPages)?;

    let mut steps = vec![
        Step::exchange(request::set_label_type(settings.label_type)),
        Step::exchange(request::set_density(settings.density)),
    ];
    steps.push(Step::exchange(request::print_start(match task {
        PrintTask::D110 => PrintStart::V1,
        PrintTask::B1 => PrintStart::V7 {
            total_pages: total,
            color: ColorMode::Single,
        },
        PrintTask::D110mV4 => PrintStart::V9 {
            total_pages: total,
            color: ColorMode::Single,
            quality: settings.quality,
            flag: 0,
        },
    })));

    for (i, page) in pages.iter().enumerate() {
        let rows = u16::try_from(page.height()).map_err(|_| JobError::TooLarge {
            page: i,
            dim: "height",
            value: page.height(),
        })?;
        let cols = u16::try_from(page.width()).map_err(|_| JobError::TooLarge {
            page: i,
            dim: "width",
            value: page.width(),
        })?;
        let copies = settings.copies;
        match task {
            PrintTask::D110 => {
                steps.push(Step::exchange(request::print_clear()));
                steps.push(Step::exchange(request::page_start()));
                steps.push(Step::exchange(request::set_page_size(
                    PageSizeFormat::Len4,
                    rows,
                    cols,
                    copies,
                )));
                steps.push(Step::exchange(request::set_quantity(copies)));
            }
            PrintTask::B1 => {
                steps.push(Step::exchange(request::page_start()));
                steps.push(Step::exchange(request::set_page_size(
                    PageSizeFormat::Len6,
                    rows,
                    cols,
                    copies,
                )));
            }
            PrintTask::D110mV4 => {
                // Firmware quirk: the first packet after PrintStart goes unanswered (§11).
                steps.push(Step::Send(request::print_status()));
                steps.push(Step::exchange(request::set_page_size(
                    PageSizeFormat::Len13,
                    rows,
                    cols,
                    copies,
                )));
            }
        }
        steps.extend(
            encode_rows(page, settings.count_mode)?
                .into_iter()
                .map(Step::Send),
        );
        steps.push(Step::exchange_until_true(request::page_end()));
    }

    steps.push(Step::WaitPages {
        total_pages: total,
        interval_ms: 300,
        stall_timeout_ms: 10_000,
    });
    steps.push(Step::exchange_until_true(request::print_end()));
    if task == PrintTask::D110mV4 {
        // Firmware quirk: the first packet after PrintEnd is dropped (§11).
        steps.push(Step::Send(request::heartbeat(HeartbeatType::Advanced1)));
    }
    Ok(steps)
}

/// Convenience: the request/expect sequence of a job as `(tx cmd, expected rx)` pairs.
pub fn command_trace(steps: &[Step]) -> Vec<(u8, Option<u8>)> {
    steps
        .iter()
        .filter_map(|s| match s {
            Step::Exchange { packet, expect, .. } => Some((packet.cmd, Some(*expect))),
            Step::Send(p) => Some((p.cmd, None)),
            Step::WaitPages { .. } => None,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn page(h: u32) -> Vec<u8> {
        let mut d = vec![0u8; 74 * h as usize];
        d[74 * 2] = 0xF0;
        d
    }

    #[test]
    fn b21_pro_sequence() {
        let data = page(4);
        let r = RasterView::new(584, 4, 74, &data).unwrap();
        let steps = build(
            PrintTask::D110mV4,
            &JobSettings {
                copies: 2,
                ..Default::default()
            },
            &[r],
        )
        .unwrap();
        let trace = command_trace(&steps);
        assert_eq!(
            trace,
            vec![
                (0x23, Some(0x33)),
                (0x21, Some(0x31)),
                (0x01, Some(0x02)),
                (0xA3, None),
                (0x13, Some(0x14)),
                (0x84, None),
                (0x85, None),
                (0x84, None),
                (0xE3, Some(0xE4)),
                (0xF3, Some(0xF4)),
                (0xDC, None),
            ]
        );
        assert!(matches!(
            steps[3 + 6],
            Step::WaitPages { total_pages: 2, .. }
        ));
        let Step::Exchange { packet, .. } = &steps[2] else {
            panic!()
        };
        assert_eq!(packet.payload, vec![0, 2, 0, 0, 0, 0, 0, 0, 0]);
    }

    #[test]
    fn b1_uses_page_start() {
        let data = page(3);
        let r = RasterView::new(584, 3, 74, &data).unwrap();
        let trace = command_trace(&build(PrintTask::B1, &JobSettings::default(), &[r]).unwrap());
        assert_eq!(trace[3], (0x03, Some(0x04)));
        assert_eq!(trace[4], (0x13, Some(0x14)));
    }

    #[test]
    fn rejects_empty_and_zero_copies() {
        assert_eq!(
            build(PrintTask::B1, &JobSettings::default(), &[]),
            Err(JobError::Empty)
        );
        let data = page(3);
        let r = RasterView::new(584, 3, 74, &data).unwrap();
        let s = JobSettings {
            copies: 0,
            ..Default::default()
        };
        assert_eq!(build(PrintTask::B1, &s, &[r]), Err(JobError::NoCopies));
    }
}
