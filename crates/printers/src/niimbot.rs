//! NIIMBOT driver: executes `labely_protocol::niimbot` programs over a [`Transport`].

use std::time::{Duration, Instant};

use labely_core::document::MediaType;
use labely_core::services::{
    DeviceError, DeviceSettings, DeviceStatus, JobControl, PaperInfo, PrintReport, PrintRequest,
};
use labely_protocol::RasterView;
use labely_protocol::niimbot::job::{self, JobSettings, PrintTask, Retry, Step};
use labely_protocol::niimbot::request::{self, HeartbeatType, InfoType, LabelType, SoundKind};
use labely_protocol::niimbot::response::{self, ConnectResult, ParseError, ProtocolVersion};
use labely_protocol::niimbot::{Decoder, Packet, cmd};
use labely_transport::{Transport, TransportError};
use tracing::{debug, trace};

use crate::driver::{Driver, Identity, prepare_for_head};
use crate::registry::{Model, Registry};

/// Reply timeout for one request.
const REPLY_TIMEOUT: Duration = Duration::from_millis(2000);
const CONNECT_ATTEMPTS: u32 = 5;
/// Calibration feeds labels until the sensor finds an edge; allow it time.
const CALIBRATE_TIMEOUT: Duration = Duration::from_secs(20);
/// Settings reads are optional; give up quickly on firmware that ignores them.
const SETTING_TIMEOUT: Duration = Duration::from_millis(800);
/// Model ids whose lid bit is inverted (docs/protocol.md §9).
const LID_INVERTED: &[u16] = &[
    512, 513, 514, 2304, 1792, 3584, 5120, 2560, 3840, 4352, 272, 273, 274,
];

/// NIIMBOT printer over any transport.
pub struct NiimbotDriver {
    transport: Box<dyn Transport>,
    decoder: Decoder,
    model: Option<Model>,
    model_code: Option<u16>,
    protocol: ProtocolVersion,
    /// Model to assume if the printer's model code is unknown.
    fallback_model: Option<Model>,
    /// Last page reported by an unsolicited PageIndex (old D11 task).
    page_index: u16,
}

impl std::fmt::Debug for NiimbotDriver {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("NiimbotDriver")
            .field("transport", self.transport.info())
            .field("model", &self.model.as_ref().map(|m| &m.id))
            .field("protocol", &self.protocol)
            .finish()
    }
}

fn io_err(e: TransportError) -> DeviceError {
    match e {
        TransportError::Disconnected => DeviceError::Unavailable("printer disconnected".into()),
        other => DeviceError::Other(other.to_string()),
    }
}

fn parse_err(e: ParseError) -> DeviceError {
    match e {
        ParseError::Printer(p) => DeviceError::Printer {
            message: format!("{p:?}"),
            hint: p.hint().to_owned(),
        },
        other => DeviceError::Other(other.to_string()),
    }
}

fn label_type(media: MediaType) -> LabelType {
    match media {
        MediaType::Gap => LabelType::Gap,
        MediaType::BlackMark => LabelType::BlackMark,
        MediaType::Continuous => LabelType::Continuous,
        MediaType::Transparent => LabelType::Transparent,
    }
}

fn media_from_code(code: u8) -> Option<MediaType> {
    match LabelType::from_code(code)? {
        LabelType::Gap | LabelType::BlackMarkGap => Some(MediaType::Gap),
        LabelType::BlackMark => Some(MediaType::BlackMark),
        LabelType::Continuous => Some(MediaType::Continuous),
        LabelType::Transparent => Some(MediaType::Transparent),
        _ => None,
    }
}

impl NiimbotDriver {
    pub fn new(transport: Box<dyn Transport>) -> Self {
        Self {
            transport,
            decoder: Decoder::new(),
            model: None,
            model_code: None,
            protocol: ProtocolVersion(0),
            fallback_model: None,
            page_index: 0,
        }
    }

    /// Model to use when the printer reports an unknown code (e.g. from the BLE name).
    pub fn with_fallback_model(mut self, model: Option<Model>) -> Self {
        self.fallback_model = model;
        self
    }

    fn send(&mut self, packet: &Packet) -> Result<(), DeviceError> {
        let bytes = packet.encode_request();
        trace!(tx = ?bytes, "niimbot tx");
        self.transport.send(&bytes).map_err(io_err)
    }

    /// Wait for a reply with command `expect` (or a generic error / not-supported reply).
    fn wait_for(&mut self, expect: u8, timeout: Duration) -> Result<Packet, DeviceError> {
        let deadline = Instant::now() + timeout;
        loop {
            while let Some(res) = self.decoder.next_packet() {
                match res {
                    Ok(p)
                        if (p.cmd == expect && expect != cmd::PAGE_INDEX)
                            || p.cmd == cmd::ERROR
                            || p.cmd == cmd::NOT_SUPPORTED =>
                    {
                        trace!(rx = p.cmd, "niimbot rx");
                        return Ok(p);
                    }
                    // The old D11 task counts pages this way; a report can come at any time.
                    Ok(p) if p.cmd == cmd::PAGE_INDEX && p.payload.len() >= 2 => {
                        self.page_index = u16::from_be_bytes([p.payload[0], p.payload[1]]);
                        if expect == cmd::PAGE_INDEX {
                            return Ok(p);
                        }
                    }
                    Ok(p) => debug!(cmd = p.cmd, "ignoring unsolicited packet"),
                    Err(e) => debug!(%e, "dropping corrupt frame"),
                }
            }
            let now = Instant::now();
            if now >= deadline {
                return Err(DeviceError::Timeout(format!("no reply 0x{expect:02X}")));
            }
            if let Some(bytes) = self.transport.recv(deadline - now).map_err(io_err)? {
                self.decoder.push(&bytes);
            }
        }
    }

    fn exchange(&mut self, packet: &Packet, expect: u8) -> Result<Packet, DeviceError> {
        self.send(packet)?;
        self.wait_for(expect, REPLY_TIMEOUT)
    }

    fn drain_input(&mut self) {
        while let Ok(Some(bytes)) = self.transport.recv(Duration::from_millis(1)) {
            self.decoder.push(&bytes);
        }
        let _ = self.decoder.drain_packets();
    }

    /// Send `packet` and parse the reply with `parse`; `None` if the printer does not answer
    /// or does not support the request.
    fn try_read<T>(
        &mut self,
        packet: &Packet,
        expect: u8,
        parse: impl Fn(&Packet) -> Result<T, ParseError>,
    ) -> Result<Option<T>, DeviceError> {
        self.send(packet)?;
        match self.wait_for(expect, SETTING_TIMEOUT) {
            Ok(p) => match parse(&p) {
                Ok(v) => Ok(Some(v)),
                Err(e) => {
                    debug!(cmd = packet.cmd, %e, "setting not readable");
                    Ok(None)
                }
            },
            Err(DeviceError::Timeout(_)) => Ok(None),
            Err(e) => Err(e),
        }
    }

    /// Send a setter and require a positive reply.
    fn set(&mut self, packet: &Packet, expect: u8, what: &str) -> Result<(), DeviceError> {
        let reply = self.exchange(packet, expect)?;
        let accepted = match response::ok(&reply, expect) {
            Ok(v) => v,
            Err(ParseError::NotSupported) => {
                return Err(DeviceError::Unavailable(format!(
                    "this printer does not support changing {what}"
                )));
            }
            Err(e) => return Err(parse_err(e)),
        };
        if accepted {
            Ok(())
        } else {
            Err(DeviceError::Printer {
                message: format!("the printer rejected the {what} change"),
                hint: "check the value is supported by this model".into(),
            })
        }
    }

    fn resolve_task(&self, model: &Model) -> Result<PrintTask, DeviceError> {
        match model.task.as_str() {
            "auto" if self.protocol >= ProtocolVersion(4) => Ok(PrintTask::D110mV4),
            "auto" => Ok(PrintTask::B1),
            name => PrintTask::from_name(name).ok_or_else(|| {
                DeviceError::Unavailable(format!(
                    "print task '{name}' for {} is not known",
                    model.name
                ))
            }),
        }
    }

    fn run_steps(
        &mut self,
        steps: &[Step],
        control: &JobControl,
        total: u32,
    ) -> Result<u32, DeviceError> {
        let gap = self.transport.packet_gap();
        let mut printed = 0u32;
        // Tasks without a wait step send every copy as a page: count the answered PageEnds.
        let counted = steps
            .iter()
            .any(|s| matches!(s, Step::WaitPages { .. } | Step::WaitPageIndex { .. }));
        self.page_index = 0;
        // Stop between pages, never inside one: before the job starts, and after
        // PrintStart or a PageEnd has been answered.
        let mut at_boundary = true;
        for step in steps {
            if at_boundary && control.is_cancelled() {
                return Err(DeviceError::Cancelled);
            }
            at_boundary = matches!(
                step,
                Step::Exchange { packet, .. }
                    if packet.cmd == cmd::PRINT_START || packet.cmd == cmd::PAGE_END
            );
            match step {
                Step::Send(p) => {
                    self.send(p)?;
                    if !gap.is_zero() {
                        std::thread::sleep(gap);
                    }
                }
                Step::Exchange {
                    packet,
                    expect,
                    retry,
                } => {
                    let (max, delay) = match retry {
                        Retry::Once => (1, 0),
                        Retry::UntilTrue { max, delay_ms } => (u32::from(*max), *delay_ms),
                    };
                    let mut ok = false;
                    for _ in 0..max.max(1) {
                        let reply = self.exchange(packet, *expect)?;
                        if response::ok(&reply, *expect).map_err(parse_err)? {
                            ok = true;
                            break;
                        }
                        std::thread::sleep(Duration::from_millis(u64::from(delay)));
                    }
                    if !ok {
                        return Err(DeviceError::Printer {
                            message: format!("command 0x{:02X} rejected", packet.cmd),
                            hint: "check labels and lid, then retry".into(),
                        });
                    }
                    if !counted && packet.cmd == cmd::PAGE_END {
                        printed += 1;
                        control.set_progress(printed, total);
                    }
                }
                Step::Sync { packet, expect } => {
                    let reply = self.exchange(packet, *expect)?;
                    response::check(&reply, *expect).map_err(parse_err)?;
                }
                Step::WaitPageIndex {
                    total_pages,
                    stall_timeout_ms,
                } => {
                    let stall = Duration::from_millis(u64::from(*stall_timeout_ms));
                    let mut page = self.page_index;
                    while page < *total_pages {
                        if control.is_cancelled() {
                            return Err(DeviceError::Cancelled);
                        }
                        let reply = self.wait_for(cmd::PAGE_INDEX, stall).map_err(|e| match e {
                            DeviceError::Timeout(_) => DeviceError::Timeout(format!(
                                "printing stalled at page {page} of {total_pages}"
                            )),
                            other => other,
                        })?;
                        response::check(&reply, cmd::PAGE_INDEX).map_err(parse_err)?;
                        page = self.page_index;
                        control.set_progress(u32::from(page), total);
                    }
                    printed = u32::from(page);
                }
                Step::WaitPages {
                    total_pages,
                    interval_ms,
                    stall_timeout_ms,
                } => {
                    let mut last_change = Instant::now();
                    let mut last_page = 0u16;
                    loop {
                        let reply =
                            self.exchange(&request::print_status(), cmd::PRINT_STATUS_RESP)?;
                        let st = response::print_status(&reply).map_err(parse_err)?;
                        if let Some(e) = st.error {
                            return Err(DeviceError::Printer {
                                message: format!("{e:?}"),
                                hint: e.hint().into(),
                            });
                        }
                        if st.page != last_page {
                            last_page = st.page;
                            last_change = Instant::now();
                            control.set_progress(u32::from(st.page), total);
                        }
                        if control.is_cancelled() && st.page < *total_pages {
                            return Err(DeviceError::Cancelled);
                        }
                        if st.page >= *total_pages {
                            printed = u32::from(st.page);
                            break;
                        }
                        if last_change.elapsed()
                            > Duration::from_millis(u64::from(*stall_timeout_ms))
                        {
                            return Err(DeviceError::Timeout(format!(
                                "printing stalled at page {} of {total_pages}",
                                st.page
                            )));
                        }
                        std::thread::sleep(Duration::from_millis(u64::from(*interval_ms)));
                    }
                }
            }
        }
        Ok(printed)
    }
}

impl Driver for NiimbotDriver {
    fn connect(&mut self) -> Result<Identity, DeviceError> {
        self.drain_input();
        let mut result = None;
        for attempt in 0..CONNECT_ATTEMPTS {
            match self.exchange(&request::connect(), cmd::CONNECT_RESP) {
                Ok(p) => {
                    result = Some(response::connect_result(&p).map_err(parse_err)?);
                    break;
                }
                Err(DeviceError::Timeout(_)) => debug!(attempt, "connect timeout, retrying"),
                Err(e) => return Err(e),
            }
        }
        let result =
            result.ok_or_else(|| DeviceError::Timeout("printer did not answer Connect".into()))?;
        self.protocol = match result {
            ConnectResult::ConnectedQueryVersion => {
                let p = self.exchange(&request::status_data(), cmd::STATUS_DATA_RESP)?;
                response::status_data(&p).map_err(parse_err)?.0
            }
            ConnectResult::ConnectedV1 => ProtocolVersion(1),
            ConnectResult::ConnectedLegacy => ProtocolVersion(0),
            ConnectResult::Disconnected => {
                return Err(DeviceError::Unavailable(
                    "printer refused the connection".into(),
                ));
            }
            ConnectResult::FirmwareError => {
                return Err(DeviceError::Printer {
                    message: "firmware error".into(),
                    hint: "power-cycle the printer".into(),
                });
            }
            ConnectResult::Other(n) => ProtocolVersion(n),
        };
        let info = request::printer_info(InfoType::ModelId);
        let reply = self.exchange(&info, InfoType::ModelId.response_cmd())?;
        let code = response::model_id(&reply).map_err(parse_err)?;
        self.model_code = Some(code);
        // Some codes are shared (512: D11 and Hi-NB-D11): the name decides between them.
        self.model = self
            .fallback_model
            .clone()
            .filter(|m| m.model_codes.contains(&code))
            .or_else(|| Registry::builtin().by_code("niimbot", code).cloned())
            .or_else(|| self.fallback_model.clone());
        debug!(code, protocol = self.protocol.0, model = ?self.model.as_ref().map(|m| &m.id), "connected");
        // Best effort: older firmware may not answer these.
        let firmware = self.try_read(
            &request::printer_info(InfoType::Firmware),
            InfoType::Firmware.response_cmd(),
            |p| response::version(p, InfoType::Firmware),
        )?;
        let hardware = self.try_read(
            &request::printer_info(InfoType::Hardware),
            InfoType::Hardware.response_cmd(),
            |p| response::version(p, InfoType::Hardware),
        )?;
        let serial = self.try_read(
            &request::printer_info(InfoType::Serial),
            InfoType::Serial.response_cmd(),
            response::serial,
        )?;
        Ok(Identity {
            name: self.transport.info().name.clone(),
            model: self.model.clone(),
            protocol: Some(self.protocol.0),
            firmware,
            hardware,
            serial: serial.filter(|s| !s.is_empty()),
        })
    }

    fn status(&mut self) -> Result<DeviceStatus, DeviceError> {
        let kind = if self.protocol >= ProtocolVersion(3) {
            HeartbeatType::Advanced2
        } else {
            HeartbeatType::Advanced1
        };
        let inverted = self.model_code.is_some_and(|c| LID_INVERTED.contains(&c));
        let reply = self.exchange(&request::heartbeat(kind), kind.response_cmd())?;
        // Some firmware answers type 4 with the type 1 reply; accept either.
        let hb = response::heartbeat(&reply, inverted).map_err(parse_err)?;
        let paper = if self.model.as_ref().is_none_or(|m| m.rfid) {
            match self.exchange(&request::rfid_info(), cmd::RFID_INFO_RESP) {
                Ok(p) => response::rfid_info(&p).ok().flatten().map(|r| PaperInfo {
                    barcode: r.barcode,
                    serial: r.serial,
                    total: u32::from(r.total),
                    used: u32::from(r.used),
                    media: media_from_code(r.label_type),
                }),
                Err(_) => None,
            }
        } else {
            None
        };
        Ok(DeviceStatus {
            battery_percent: hb.battery_percent,
            lid_closed: hb.lid_closed,
            paper_present: hb.paper_present,
            paper,
        })
    }

    fn print(
        &mut self,
        job: &PrintRequest,
        control: &JobControl,
    ) -> Result<PrintReport, DeviceError> {
        let started = Instant::now();
        let model = self.model.clone().ok_or_else(|| {
            DeviceError::Unavailable("unknown printer model; connect first".into())
        })?;
        let task = self.resolve_task(&model)?;
        let density = job.density.clamp(model.density.min, model.density.max);
        let pages: Vec<_> = job
            .pages
            .iter()
            .map(|b| prepare_for_head(b, &model))
            .collect();
        let views = pages
            .iter()
            .map(|b| RasterView::new(b.width(), b.height(), b.stride(), b.data()))
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| DeviceError::Other(e.to_string()))?;
        let settings = JobSettings {
            density,
            label_type: label_type(job.media),
            copies: job.copies.max(1),
            ..JobSettings::default()
        };
        let steps =
            job::build(task, &settings, &views).map_err(|e| DeviceError::Other(e.to_string()))?;
        self.drain_input();
        let total = job.pages.len() as u32 * u32::from(settings.copies);
        control.set_progress(0, total);
        let printed = self.run_steps(&steps, control, total);
        if printed.is_err() {
            // Best effort: leave the printer idle.
            let _ = self.exchange(&request::print_end(), cmd::PRINT_END_RESP);
        }
        Ok(PrintReport {
            pages_printed: printed?,
            duration_ms: started.elapsed().as_millis() as u64,
        })
    }

    fn calibrate(&mut self, media: MediaType) -> Result<(), DeviceError> {
        self.drain_input();
        self.send(&request::calibrate(label_type(media)))?;
        let reply = self.wait_for(cmd::CALIBRATE_RESP, CALIBRATE_TIMEOUT)?;
        match response::check(&reply, cmd::CALIBRATE_RESP) {
            Ok(p) => {
                debug!(payload = ?p.payload, "calibration done");
                Ok(())
            }
            Err(ParseError::NotSupported) => Err(DeviceError::Unavailable(
                "this printer does not support calibration".into(),
            )),
            Err(e) => Err(parse_err(e)),
        }
    }

    fn settings(&mut self) -> Result<DeviceSettings, DeviceError> {
        self.drain_input();
        let auto_off_level = self.try_read(
            &request::printer_info(InfoType::AutoOff),
            InfoType::AutoOff.response_cmd(),
            response::auto_off,
        )?;
        let sound_power = self.try_read(
            &request::sound_get(SoundKind::Power),
            cmd::SOUND_RESP,
            response::sound,
        )?;
        let sound_bluetooth = self.try_read(
            &request::sound_get(SoundKind::Bluetooth),
            cmd::SOUND_RESP,
            response::sound,
        )?;
        Ok(DeviceSettings {
            auto_off_level: auto_off_level.filter(|l| request::AUTO_OFF_LEVELS.contains(l)),
            sound_power,
            sound_bluetooth,
        })
    }

    fn set_settings(&mut self, patch: &DeviceSettings) -> Result<(), DeviceError> {
        self.drain_input();
        if let Some(level) = patch.auto_off_level {
            if !request::AUTO_OFF_LEVELS.contains(&level) {
                return Err(DeviceError::Other(format!(
                    "auto-off level {level} is not 1–4"
                )));
            }
            self.set(
                &request::set_auto_off(level),
                cmd::SET_AUTO_OFF_RESP,
                "the auto-off time",
            )?;
        }
        for (kind, value) in [
            (SoundKind::Power, patch.sound_power),
            (SoundKind::Bluetooth, patch.sound_bluetooth),
        ] {
            if let Some(on) = value {
                let reply = self.exchange(&request::sound_set(kind, on), cmd::SOUND_RESP)?;
                match response::check(&reply, cmd::SOUND_RESP) {
                    Ok(_) => {}
                    Err(ParseError::NotSupported) => {
                        return Err(DeviceError::Unavailable(
                            "this printer does not support sound settings".into(),
                        ));
                    }
                    Err(e) => return Err(parse_err(e)),
                }
            }
        }
        Ok(())
    }

    fn close(&mut self) {
        self.transport.close();
    }
}
