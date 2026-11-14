//! Scan for BLE label printers for 5 s and print what was found. Never connects.
//! `cargo run -p labely-transport --features ble --example ble_scan [-- --all]`

use std::time::Duration;

fn main() -> Result<(), labely_transport::TransportError> {
    let all = std::env::args().any(|a| a == "--all");
    // First letters of known NIIMBOT model names (B1, B21, B3S, D11, D110, ...).
    let prefixes: &[&str] = if all {
        &[]
    } else {
        &["B", "D", "A", "H", "N", "P", "S", "T", "K", "M"]
    };
    let found = labely_transport::ble::scan(Duration::from_secs(5), prefixes)?;
    if found.is_empty() {
        println!("no devices found");
    }
    for d in found {
        let rssi = d.rssi.map_or_else(|| "?".to_owned(), |r| r.to_string());
        println!(
            "{:<40} {:>4} dBm  {:<24} {}",
            d.info.uri(),
            rssi,
            d.info.name,
            d.services.join(",")
        );
    }
    Ok(())
}
