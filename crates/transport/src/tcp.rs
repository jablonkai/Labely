//! Raw TCP transport: the "port 9100" socket of network label printers
//! (docs/protocol-languages.md §7).

use std::io::{ErrorKind, Read, Write};
use std::net::{Ipv6Addr, Shutdown, TcpStream, ToSocketAddrs};
use std::time::Duration;

use crate::{Transport, TransportError, TransportInfo, TransportKind};

/// Port used when the address names none.
pub const DEFAULT_PORT: u16 = 9100;

/// Bytes per write. TCP does its own segmenting; this only bounds one `write` call.
const CHUNK: usize = 4096;

/// A connected TCP socket.
#[derive(Debug)]
pub struct TcpTransport {
    info: TransportInfo,
    stream: TcpStream,
    open: bool,
}

/// `host`, `host:port`, `[v6]` or `[v6]:port` with the default port filled in. A bare IPv6
/// address gets its brackets. A port that is not a number from 1 to 65535 is an error, and so
/// is a missing host.
fn with_port(address: &str) -> Result<String, TransportError> {
    let bad = |what: &str| TransportError::NotFound(format!("{address}: {what}"));
    let (host, port) = if let Some(rest) = address.strip_prefix('[') {
        let (v6, tail) = rest.split_once(']').ok_or_else(|| bad("missing ']'"))?;
        let port = match tail.strip_prefix(':') {
            Some(port) => Some(port),
            None if tail.is_empty() => None,
            None => return Err(bad("expected ':port' after ']'")),
        };
        (format!("[{v6}]"), port)
    } else if address.parse::<Ipv6Addr>().is_ok() {
        // A bare IPv6 address has colons but no port.
        (format!("[{address}]"), None)
    } else {
        match address.rsplit_once(':') {
            Some((host, port)) => (host.to_owned(), Some(port)),
            None => (address.to_owned(), None),
        }
    };
    if host.is_empty() || host == "[]" {
        return Err(bad("no host"));
    }
    let port = match port {
        Some(port) => port
            .parse::<u16>()
            .ok()
            .filter(|port| *port != 0)
            .ok_or_else(|| bad("the port must be a number from 1 to 65535"))?,
        None => DEFAULT_PORT,
    };
    Ok(format!("{host}:{port}"))
}

/// Connect to `address` (`host[:port]`, port 9100 by default), trying each resolved address
/// for up to `timeout`.
pub fn open(address: &str, timeout: Duration) -> Result<TcpTransport, TransportError> {
    let target = with_port(address)?;
    let addrs = target
        .to_socket_addrs()
        .map_err(|e| TransportError::NotFound(format!("{target}: {e}")))?;
    let mut last = None;
    for addr in addrs {
        match TcpStream::connect_timeout(&addr, timeout) {
            Ok(stream) => {
                // Small status queries should not wait for more data.
                let _ = stream.set_nodelay(true);
                stream.set_write_timeout(Some(Duration::from_secs(30)))?;
                return Ok(TcpTransport {
                    info: TransportInfo {
                        kind: TransportKind::Tcp,
                        address: target.clone(),
                        name: target,
                    },
                    stream,
                    open: true,
                });
            }
            Err(e) => last = Some(e),
        }
    }
    Err(match last {
        Some(e) => TransportError::NotFound(format!("{target}: {e}")),
        None => TransportError::NotFound(format!("{target}: no address")),
    })
}

impl Transport for TcpTransport {
    fn send(&mut self, bytes: &[u8]) -> Result<(), TransportError> {
        if !self.open {
            return Err(TransportError::Disconnected);
        }
        self.stream.write_all(bytes)?;
        self.stream.flush()?;
        Ok(())
    }

    fn recv(&mut self, timeout: Duration) -> Result<Option<Vec<u8>>, TransportError> {
        if !self.open {
            return Err(TransportError::Disconnected);
        }
        // A zero read timeout means "block for ever" to the socket API.
        self.stream
            .set_read_timeout(Some(timeout.max(Duration::from_millis(1))))?;
        let mut buf = [0u8; CHUNK];
        match self.stream.read(&mut buf) {
            Ok(0) => {
                self.open = false;
                Err(TransportError::Disconnected)
            }
            Ok(n) => Ok(Some(buf[..n].to_vec())),
            Err(e) if matches!(e.kind(), ErrorKind::WouldBlock | ErrorKind::TimedOut) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    fn mtu(&self) -> usize {
        CHUNK
    }

    fn info(&self) -> &TransportInfo {
        &self.info
    }

    fn close(&mut self) {
        if self.open {
            let _ = self.stream.shutdown(Shutdown::Both);
            self.open = false;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::TcpListener;

    #[test]
    fn default_port() {
        let ok = |address: &str| with_port(address).unwrap();
        assert_eq!(ok("printer.local"), "printer.local:9100");
        assert_eq!(ok("10.0.0.5:6101"), "10.0.0.5:6101");
        assert_eq!(ok("[::1]:6101"), "[::1]:6101");
        assert_eq!(ok("[::1]"), "[::1]:9100");
        // A bare IPv6 address is bracketed, not given a ninth group.
        assert_eq!(ok("fe80::1"), "[fe80::1]:9100");
        assert_eq!(ok("::1"), "[::1]:9100");
    }

    #[test]
    fn bad_addresses_are_named() {
        for address in [
            "",
            ":9100",
            "[]:9100",
            "printer:port",
            "printer:99999",
            "printer:0",
            "printer:",
            "[::1",
            "[::1]9100",
        ] {
            let err = with_port(address).unwrap_err();
            assert!(
                matches!(&err, TransportError::NotFound(m) if m.starts_with(address)),
                "{address}: {err}"
            );
            assert!(matches!(
                open(address, Duration::from_millis(50)),
                Err(TransportError::NotFound(_))
            ));
        }
    }

    #[test]
    fn talks_over_ipv6_without_brackets_in_the_host() {
        let Ok(listener) = TcpListener::bind("[::1]:0") else {
            return; // no IPv6 loopback on this machine
        };
        let port = listener.local_addr().unwrap().port();
        let t = open(&format!("[::1]:{port}"), Duration::from_secs(5)).unwrap();
        assert_eq!(t.info().uri(), format!("tcp:[::1]:{port}"));
    }

    #[test]
    fn talks_to_a_local_socket() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let server = std::thread::spawn(move || {
            let (mut s, _) = listener.accept().unwrap();
            let mut buf = [0u8; 3];
            s.read_exact(&mut buf).unwrap();
            s.write_all(&[buf[2], buf[1], buf[0]]).unwrap();
        });
        let mut t = open(&format!("127.0.0.1:{port}"), Duration::from_secs(5)).unwrap();
        assert_eq!(t.info().uri(), format!("tcp:127.0.0.1:{port}"));
        assert_eq!(t.recv(Duration::from_millis(20)).unwrap(), None);
        t.send(&[1, 2, 3]).unwrap();
        assert_eq!(t.recv(Duration::from_secs(5)).unwrap(), Some(vec![3, 2, 1]));
        server.join().unwrap();
        // The peer closed the socket.
        assert!(matches!(
            t.recv(Duration::from_secs(5)),
            Err(TransportError::Disconnected)
        ));
        t.close();
        assert!(matches!(t.send(&[0]), Err(TransportError::Disconnected)));
    }

    #[test]
    fn refused_connection_is_not_found() {
        let port = {
            let l = TcpListener::bind("127.0.0.1:0").unwrap();
            l.local_addr().unwrap().port()
        };
        assert!(matches!(
            open(&format!("127.0.0.1:{port}"), Duration::from_secs(2)),
            Err(TransportError::NotFound(_))
        ));
    }
}
