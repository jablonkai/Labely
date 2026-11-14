//! List USB serial ports. `cargo run -p labely-transport --features serial --example serial_list`

fn main() -> Result<(), labely_transport::TransportError> {
    let ports = labely_transport::serial::list()?;
    if ports.is_empty() {
        println!("no USB serial ports");
    }
    for p in ports {
        println!("{:<32} {}", p.info.uri(), p.info.name);
    }
    Ok(())
}
