//! In-memory transport pair for tests and simulators.

use std::sync::mpsc::{Receiver, RecvTimeoutError, Sender, channel};
use std::time::Duration;

use crate::{Transport, TransportError, TransportInfo, TransportKind};

/// Host side of an in-memory link.
#[derive(Debug)]
pub struct MemoryTransport {
    info: TransportInfo,
    tx: Sender<Vec<u8>>,
    rx: Receiver<Vec<u8>>,
    mtu: usize,
    open: bool,
}

/// Device side of an in-memory link (what a simulator holds).
#[derive(Debug)]
pub struct MemoryPeer {
    pub tx: Sender<Vec<u8>>,
    pub rx: Receiver<Vec<u8>>,
}

impl MemoryTransport {
    /// Create a connected pair. Writes from the host arrive at the peer in chunks of at most `mtu`.
    pub fn pair(name: &str, mtu: usize) -> (Self, MemoryPeer) {
        let (host_tx, peer_rx) = channel();
        let (peer_tx, host_rx) = channel();
        let info = TransportInfo {
            kind: TransportKind::Memory,
            address: name.to_owned(),
            name: name.to_owned(),
        };
        (
            Self {
                info,
                tx: host_tx,
                rx: host_rx,
                mtu: mtu.max(1),
                open: true,
            },
            MemoryPeer {
                tx: peer_tx,
                rx: peer_rx,
            },
        )
    }
}

impl Transport for MemoryTransport {
    fn send(&mut self, bytes: &[u8]) -> Result<(), TransportError> {
        if !self.open {
            return Err(TransportError::Disconnected);
        }
        for chunk in bytes.chunks(self.mtu) {
            self.tx
                .send(chunk.to_vec())
                .map_err(|_| TransportError::Disconnected)?;
        }
        Ok(())
    }

    fn recv(&mut self, timeout: Duration) -> Result<Option<Vec<u8>>, TransportError> {
        if !self.open {
            return Err(TransportError::Disconnected);
        }
        match self.rx.recv_timeout(timeout) {
            Ok(b) => Ok(Some(b)),
            Err(RecvTimeoutError::Timeout) => Ok(None),
            Err(RecvTimeoutError::Disconnected) => Err(TransportError::Disconnected),
        }
    }

    fn mtu(&self) -> usize {
        self.mtu
    }

    fn info(&self) -> &TransportInfo {
        &self.info
    }

    fn close(&mut self) {
        self.open = false;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chunks_by_mtu() {
        let (mut host, peer) = MemoryTransport::pair("t", 4);
        host.send(&[1, 2, 3, 4, 5]).unwrap();
        assert_eq!(peer.rx.recv().unwrap(), vec![1, 2, 3, 4]);
        assert_eq!(peer.rx.recv().unwrap(), vec![5]);
        peer.tx.send(vec![9]).unwrap();
        assert_eq!(host.recv(Duration::from_millis(10)).unwrap(), Some(vec![9]));
        assert_eq!(host.recv(Duration::from_millis(1)).unwrap(), None);
    }
}
