//! Print-job programs (docs/protocol.md §5, §12). A job is a list of [`Step`]s that a
//! driver executes against a transport. Keeping it as data makes every variant testable
//! byte-for-byte without hardware.

use serde::{Deserialize, Serialize};
use thiserror::Error;

use super::request::{self, ColorMode, HeartbeatType, LabelType, PageSizeFormat, PrintStart};
use super::rows::{CHECK_LINE_INTERVAL, CountMode, encode_rows, encode_rows_checked};
use super::{Packet, PacketError, cmd};
use crate::RasterView;

/// Print-task variant (docs/protocol.md §5.2). Selected per model in the registry.
///
/// Only `D110mV4` has run on hardware (B21 Pro). The others follow the OSS sequences.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PrintTask {
    /// 1-byte PrintStart, PrintClear, PageStart, 2-byte size, SetQuantity; the printer
    /// reports each page with an unsolicited 0xE0 (old D11).
    D11V1,
    /// 1-byte PrintStart; each copy is sent as its own page with a 4-byte size and
    /// check-lines; PrintEnd is polled until the printer is done (B21).
    B21V1,
    /// As `B21V1`, but PageStart and PageEnd are retried and nothing is polled (B21-L2B).
    B21L2b,
    /// 1-byte PrintStart, PrintClear, PageStart, 4-byte size, SetQuantity (B21S, D110, D11 v2).
    D110,
    /// 7-byte PrintStart, PageStart, 6-byte size (B1, D110_M, B21_C2B, N1).
    B1,
    /// 2-byte PrintStart, PageStart, 9-byte size with the cut fields at zero (H1S).
    H1s,
    /// 9-byte PrintStart, one-way status, 13-byte size, no PageStart (B21 Pro, B1 Pro, D11_H).
    D110mV4,
}

impl PrintTask {
    /// The task for a registry `task` name; `None` for `auto` and unknown names.
    pub fn from_name(name: &str) -> Option<Self> {
        Some(match name {
            "d11_v1" => Self::D11V1,
            "b21_v1" => Self::B21V1,
            "b21_l2b" => Self::B21L2b,
            "d110" => Self::D110,
            "b1" => Self::B1,
            "h1s" => Self::H1s,
            "d110m_v4" => Self::D110mV4,
            _ => return None,
        })
    }

    /// The printer takes no copy count: every copy is sent as a page.
    fn per_copy(self) -> bool {
        matches!(self, Self::B21V1 | Self::B21L2b)
    }
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
    /// Send and wait for `expect`, whatever its payload says (row check-line).
    Sync { packet: Packet, expect: u8 },
    /// Send without waiting for a reply.
    Send(Packet),
    /// Poll PrintStatus (0xA3) until `page >= total_pages`.
    WaitPages {
        total_pages: u16,
        interval_ms: u32,
        stall_timeout_ms: u32,
    },
    /// Wait for unsolicited PageIndex (0xE0) packets until `page >= total_pages`.
    WaitPageIndex {
        total_pages: u16,
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
        Self::exchange_retry(packet, 50, 100)
    }

    fn exchange_retry(packet: Packet, max: u16, delay_ms: u32) -> Self {
        match Self::exchange(packet) {
            Step::Exchange { packet, expect, .. } => Step::Exchange {
                packet,
                expect,
                retry: Retry::UntilTrue { max, delay_ms },
            },
            other => other,
        }
    }

    /// A row packet: check-lines are answered, everything else is one-way.
    fn row(packet: Packet) -> Self {
        if packet.cmd == cmd::CHECK_LINE {
            Step::Sync {
                packet,
                expect: cmd::CHECK_LINE_RESP,
            }
        } else {
            Step::Send(packet)
        }
    }

    /// Bytes this step puts on the wire (for fixtures and tests).
    pub fn wire_bytes(&self) -> Option<Vec<u8>> {
        match self {
            Step::Exchange { packet, .. } | Step::Sync { packet, .. } | Step::Send(packet) => {
                Some(packet.encode_request())
            }
            Step::WaitPages { .. } | Step::WaitPageIndex { .. } => None,
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
        PrintTask::D11V1 | PrintTask::B21V1 | PrintTask::B21L2b | PrintTask::D110 => PrintStart::V1,
        PrintTask::H1s => PrintStart::V2 { total_pages: total },
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
        // The B21 tasks use the total count and check-lines (OSS).
        let row_packets = if task.per_copy() {
            encode_rows_checked(page, CountMode::Total, Some(CHECK_LINE_INTERVAL))?
        } else {
            encode_rows(page, settings.count_mode)?
        };
        let sends = if task.per_copy() { copies } else { 1 };
        for _ in 0..sends {
            let size = |format| request::set_page_size(format, rows, cols, copies);
            match task {
                PrintTask::D11V1 | PrintTask::D110 => {
                    let format = if task == PrintTask::D11V1 {
                        PageSizeFormat::Len2
                    } else {
                        PageSizeFormat::Len4
                    };
                    steps.push(Step::exchange(request::print_clear()));
                    steps.push(Step::exchange(request::page_start()));
                    steps.push(Step::exchange(size(format)));
                    steps.push(Step::exchange(request::set_quantity(copies)));
                }
                PrintTask::B21V1 => {
                    steps.push(Step::exchange(request::page_start()));
                    steps.push(Step::exchange(size(PageSizeFormat::Len4)));
                }
                PrintTask::B21L2b => {
                    steps.push(Step::exchange_retry(request::page_start(), 5, 500));
                    steps.push(Step::exchange(size(PageSizeFormat::Len4)));
                }
                PrintTask::B1 => {
                    steps.push(Step::exchange(request::page_start()));
                    steps.push(Step::exchange(size(PageSizeFormat::Len6)));
                }
                PrintTask::H1s => {
                    steps.push(Step::exchange(request::page_start()));
                    steps.push(Step::exchange(size(PageSizeFormat::Len9)));
                }
                PrintTask::D110mV4 => {
                    // Firmware quirk: the first packet after PrintStart goes unanswered (§11).
                    steps.push(Step::Send(request::print_status()));
                    steps.push(Step::exchange(size(PageSizeFormat::Len13)));
                }
            }
            steps.extend(row_packets.iter().cloned().map(Step::row));
            steps.push(if task == PrintTask::B21L2b {
                Step::exchange_retry(request::page_end(), 20, 500)
            } else {
                Step::exchange_until_true(request::page_end())
            });
        }
    }

    match task {
        PrintTask::D11V1 => steps.push(Step::WaitPageIndex {
            total_pages: total,
            stall_timeout_ms: 10_000,
        }),
        // B21: PrintEnd answers false until the last page is out. B21-L2B: PageEnd did.
        PrintTask::B21V1 | PrintTask::B21L2b => {}
        PrintTask::D110 | PrintTask::B1 | PrintTask::H1s | PrintTask::D110mV4 => {
            steps.push(Step::WaitPages {
                total_pages: total,
                interval_ms: 300,
                stall_timeout_ms: 10_000,
            });
        }
    }
    steps.push(if task == PrintTask::B21V1 {
        Step::exchange_retry(request::print_end(), 600, 500)
    } else {
        Step::exchange_until_true(request::print_end())
    });
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
            Step::Exchange { packet, expect, .. } | Step::Sync { packet, expect } => {
                Some((packet.cmd, Some(*expect)))
            }
            Step::Send(p) => Some((p.cmd, None)),
            Step::WaitPages { .. } | Step::WaitPageIndex { .. } => None,
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

    fn trace(task: PrintTask, copies: u16, rows: u32) -> Vec<(u8, Option<u8>)> {
        let data = page(rows);
        let r = RasterView::new(584, rows, 74, &data).unwrap();
        let settings = JobSettings {
            copies,
            ..JobSettings::default()
        };
        command_trace(&build(task, &settings, &[r]).unwrap())
    }

    fn payload_of(steps: &[Step], cmd: u8) -> Vec<u8> {
        steps
            .iter()
            .find_map(|s| match s {
                Step::Exchange { packet, .. } if packet.cmd == cmd => Some(packet.payload.clone()),
                _ => None,
            })
            .unwrap()
    }

    #[test]
    fn d11_v1_sequence() {
        let data = page(3);
        let r = RasterView::new(96, 3, 74, &data).unwrap();
        let settings = JobSettings {
            copies: 2,
            ..JobSettings::default()
        };
        let steps = build(PrintTask::D11V1, &settings, &[r]).unwrap();
        let cmds: Vec<u8> = command_trace(&steps).iter().map(|c| c.0).collect();
        assert_eq!(
            cmds,
            vec![
                0x23, 0x21, 0x01, 0x20, 0x03, 0x13, 0x15, 0x84, 0x85, 0xE3, 0xF3
            ]
        );
        assert_eq!(payload_of(&steps, 0x01), vec![1]);
        assert_eq!(payload_of(&steps, 0x13), vec![0, 3]);
        assert_eq!(payload_of(&steps, 0x15), vec![0, 2]);
        assert!(steps.contains(&Step::WaitPageIndex {
            total_pages: 2,
            stall_timeout_ms: 10_000
        }));
        assert!(!steps.iter().any(|s| matches!(s, Step::WaitPages { .. })));
    }

    #[test]
    fn b21_v1_sends_each_copy_with_check_lines() {
        let t = trace(PrintTask::B21V1, 2, 201);
        let cmds: Vec<u8> = t.iter().map(|c| c.0).collect();
        let copy = [0x03, 0x13, 0x84, 0x85, 0x84, 0x86, 0x84, 0xE3];
        let mut expected = vec![0x23, 0x21, 0x01];
        expected.extend(copy);
        expected.extend(copy);
        expected.push(0xF3);
        assert_eq!(cmds, expected);
        assert!(t.contains(&(0x86, Some(0xD3))));

        let data = page(3);
        let r = RasterView::new(584, 3, 74, &data).unwrap();
        let steps = build(PrintTask::B21V1, &JobSettings::default(), &[r]).unwrap();
        assert_eq!(payload_of(&steps, 0x13), vec![0, 3, 0x02, 0x48]);
        // Total count mode: [0][low][high].
        let row = steps
            .iter()
            .find_map(|s| match s {
                Step::Send(p) if p.cmd == 0x85 => Some(p.payload.clone()),
                _ => None,
            })
            .unwrap();
        assert_eq!(row[2..5], [0, 4, 0]);
        assert!(!steps.iter().any(|s| matches!(s, Step::WaitPages { .. })));
    }

    #[test]
    fn b21_l2b_retries_page_start_and_end() {
        let data = page(3);
        let r = RasterView::new(584, 3, 74, &data).unwrap();
        let steps = build(PrintTask::B21L2b, &JobSettings::default(), &[r]).unwrap();
        let retry_of = |cmd: u8| {
            steps.iter().find_map(|s| match s {
                Step::Exchange { packet, retry, .. } if packet.cmd == cmd => Some(*retry),
                _ => None,
            })
        };
        assert_eq!(
            retry_of(0x03),
            Some(Retry::UntilTrue {
                max: 5,
                delay_ms: 500
            })
        );
        assert_eq!(
            retry_of(0xE3),
            Some(Retry::UntilTrue {
                max: 20,
                delay_ms: 500
            })
        );
        assert!(
            !steps
                .iter()
                .any(|s| matches!(s, Step::WaitPages { .. } | Step::WaitPageIndex { .. }))
        );
    }

    #[test]
    fn h1s_sequence() {
        let data = page(3);
        let r = RasterView::new(96, 3, 74, &data).unwrap();
        let settings = JobSettings {
            copies: 3,
            ..JobSettings::default()
        };
        let steps = build(PrintTask::H1s, &settings, &[r]).unwrap();
        let cmds: Vec<u8> = command_trace(&steps).iter().map(|c| c.0).collect();
        assert_eq!(
            cmds,
            vec![0x23, 0x21, 0x01, 0x03, 0x13, 0x84, 0x85, 0xE3, 0xF3]
        );
        assert_eq!(payload_of(&steps, 0x01), vec![0, 3]);
        assert_eq!(payload_of(&steps, 0x13), vec![0, 3, 0, 96, 0, 3, 0, 0, 0]);
        assert!(steps.iter().any(|s| matches!(s, Step::WaitPages { .. })));
    }

    #[test]
    fn task_names() {
        assert_eq!(PrintTask::from_name("d11_v1"), Some(PrintTask::D11V1));
        assert_eq!(PrintTask::from_name("b21_v1"), Some(PrintTask::B21V1));
        assert_eq!(PrintTask::from_name("b21_l2b"), Some(PrintTask::B21L2b));
        assert_eq!(PrintTask::from_name("h1s"), Some(PrintTask::H1s));
        assert_eq!(PrintTask::from_name("auto"), None);
    }
}
