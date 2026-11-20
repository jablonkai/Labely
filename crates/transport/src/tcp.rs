//! Raw TCP transport: the "port 9100" socket of network label printers
//! (docs/protocol-languages.md §7).

use std::io::{ErrorKind, Read, Write};
use std::net::{Shutdown, TcpStream, ToSocketAddrs};
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

/// `host` or `host:port` (also `[v6]:port`) with the default port filled in.
fn with_port(address: &str) -> String {
    let has_port = match address.rsplit_once(':') {
        // A bare IPv6 address has colons but no port.
        Some((host, port)) => {
            port.parse::<u16>().is_ok() && (!host.contains(':') || host.ends_with(']'))
        }
        None => false,
    };
    if has_port {
        address.to_owned()
    } else {
        format!("{address}:{DEFAULT_PORT}")
    }
}

/// Connect to `address` (`host[:port]`, port 9100 by default), trying each resolved address
/// for up to `timeout`.
pub fn open(address: &str, timeout: Duration) -> Result<TcpTransport, TransportError> {
    let target = with_port(address);
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
        assert_eq!(with_port("printer.local"), "printer.local:9100");
        assert_eq!(with_port("10.0.0.5:6101"), "10.0.0.5:6101");
        assert_eq!(with_port("[::1]:6101"), "[::1]:6101");
        assert_eq!(with_port("fe80::1"), "fe80::1:9100");
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
