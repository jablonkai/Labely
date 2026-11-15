//! Read-only NIIMBOT probe: connects, then dumps the raw reply of every PrinterInfo type and
//! the printer-info heartbeat. It never changes a setting or prints.
//! `cargo run -p labely-printers --features ble --example niimbot_probe -- <ble-address>`

use std::time::{Duration, Instant};

use labely_protocol::niimbot::request::{self, HeartbeatType, InfoType};
use labely_protocol::niimbot::{Decoder, Packet, cmd};
use labely_transport::Transport;

fn exchange(t: &mut dyn Transport, p: &Packet, expect: u8) -> Option<Packet> {
    t.send(&p.encode_request()).ok()?;
    let mut dec = Decoder::new();
    let deadline = Instant::now() + Duration::from_secs(2);
    while Instant::now() < deadline {
        if let Ok(Some(bytes)) = t.recv(Duration::from_millis(200)) {
            dec.push(&bytes);
            for pkt in dec.drain_packets() {
                if pkt.cmd == expect || pkt.cmd == 0x00 {
                    return Some(pkt);
                }
            }
        }
    }
    None
}

fn show(label: &str, reply: Option<Packet>) {
    match reply {
        Some(p) => println!("{label:<12} cmd={:02X} payload={:02X?}", p.cmd, p.payload),
        None => println!("{label:<12} (no reply)"),
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let address = std::env::args()
        .nth(1)
        .ok_or("usage: niimbot_probe <ble-address>")?;
    let address = address.strip_prefix("ble:").unwrap_or(&address).to_owned();
    let mut t = labely_transport::ble::open(&address, Duration::from_secs(15))?;
    show(
        "connect",
        exchange(&mut t, &request::connect(), cmd::CONNECT_RESP),
    );
    for kind in [
        InfoType::Density,
        InfoType::Speed,
        InfoType::LabelType,
        InfoType::Language,
        InfoType::AutoOff,
        InfoType::ModelId,
        InfoType::Firmware,
        InfoType::Battery,
        InfoType::Serial,
        InfoType::Hardware,
        InfoType::BtMac,
        InfoType::PrintMode,
        InfoType::Area,
    ] {
        let reply = exchange(&mut t, &request::printer_info(kind), kind.response_cmd());
        show(&format!("{kind:?}"), reply);
    }
    let hb = HeartbeatType::PrinterInfo;
    show(
        "hb-info",
        exchange(&mut t, &request::heartbeat(hb), hb.response_cmd()),
    );
    t.close();
    Ok(())
}
