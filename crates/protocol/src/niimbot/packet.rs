//! NIIMBOT packet framing (docs/protocol.md §1).
//!
//! `55 55 | cmd | len | payload[len] | xor(cmd, len, payload) | AA AA`
//!
//! A grayscale row (0x8D) has a 2-byte big-endian length, and both bytes are in the checksum.

use thiserror::Error;

use super::cmd;

const HEAD: [u8; 2] = [0x55, 0x55];
const TAIL: [u8; 2] = [0xAA, 0xAA];
/// Prefix byte sent in front of a Connect (0xC1) request.
const CONNECT_PREFIX: u8 = 0x03;
/// Largest buffered receive data before the decoder discards stale bytes.
const MAX_BUFFER: usize = 64 * 1024;

/// Framing errors.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum PacketError {
    #[error("payload of {0} bytes is too long for one packet")]
    PayloadTooLong(usize),
    #[error(
        "checksum mismatch for command 0x{cmd:02X}: expected 0x{expected:02X}, got 0x{got:02X}"
    )]
    Checksum { cmd: u8, expected: u8, got: u8 },
    #[error("missing 0xAAAA tail for command 0x{0:02X}")]
    BadTail(u8),
}

/// One NIIMBOT packet (either direction).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Packet {
    pub cmd: u8,
    pub payload: Vec<u8>,
}

impl Packet {
    /// Whether `cmd` is framed with a 2-byte length.
    fn long(cmd: u8) -> bool {
        cmd == cmd::ROW_GRAY
    }

    /// Create a packet, rejecting payloads longer than 255 bytes (65535 for a grayscale row).
    pub fn new(cmd: u8, payload: Vec<u8>) -> Result<Self, PacketError> {
        let max = if Self::long(cmd) {
            usize::from(u16::MAX)
        } else {
            usize::from(u8::MAX)
        };
        if payload.len() > max {
            return Err(PacketError::PayloadTooLong(payload.len()));
        }
        Ok(Self { cmd, payload })
    }

    /// Internal constructor for builders whose payload length is statically small.
    pub(crate) fn small(cmd: u8, payload: &[u8]) -> Self {
        debug_assert!(payload.len() <= u8::MAX as usize);
        Self {
            cmd,
            payload: payload.to_vec(),
        }
    }

    /// XOR checksum over command, length and payload.
    pub fn checksum(&self) -> u8 {
        let len = self.payload.len();
        // The high length byte is zero in a short packet.
        self.payload
            .iter()
            .fold(self.cmd ^ (len >> 8) as u8 ^ len as u8, |acc, b| acc ^ b)
    }

    /// Encode as a frame (no Connect prefix). Used for fixtures and responses.
    pub fn encode(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(self.payload.len() + 8);
        out.extend_from_slice(&HEAD);
        out.push(self.cmd);
        if Self::long(self.cmd) {
            out.push((self.payload.len() >> 8) as u8);
        }
        out.push(self.payload.len() as u8);
        out.extend_from_slice(&self.payload);
        out.push(self.checksum());
        out.extend_from_slice(&TAIL);
        out
    }

    /// Encode as a request to the printer: identical to [`Self::encode`] except that a
    /// Connect request carries the `0x03` prefix byte.
    pub fn encode_request(&self) -> Vec<u8> {
        let frame = self.encode();
        if self.cmd == cmd::CONNECT {
            let mut out = Vec::with_capacity(frame.len() + 1);
            out.push(CONNECT_PREFIX);
            out.extend_from_slice(&frame);
            out
        } else {
            frame
        }
    }

    /// Interpret a one-byte status payload as a boolean (`01` = success).
    pub fn as_bool(&self) -> bool {
        self.payload.first().is_some_and(|b| *b != 0)
    }
}

/// Streaming decoder: feed transport reads, pull complete packets.
///
/// A read may contain several packets or only part of one. Garbage between frames is
/// skipped; a corrupt frame yields one error and decoding resynchronises after it.
#[derive(Debug, Default)]
pub struct Decoder {
    buf: Vec<u8>,
    /// Decode grayscale rows (0x8D) with their 2-byte length.
    requests: bool,
}

impl Decoder {
    pub fn new() -> Self {
        Self::default()
    }

    /// A decoder for the printer side, which also receives the long grayscale rows. A host
    /// never gets one, so [`Self::new`] keeps every frame short.
    pub fn for_requests() -> Self {
        Self {
            buf: Vec::new(),
            requests: true,
        }
    }

    /// Append received bytes.
    pub fn push(&mut self, bytes: &[u8]) {
        self.buf.extend_from_slice(bytes);
        if self.buf.len() > MAX_BUFFER {
            let excess = self.buf.len() - MAX_BUFFER;
            self.buf.drain(..excess);
        }
    }

    /// Number of buffered, not yet decoded bytes.
    pub fn buffered(&self) -> usize {
        self.buf.len()
    }

    /// Next complete packet, an error for a corrupt frame, or `None` if more bytes are needed.
    pub fn next_packet(&mut self) -> Option<Result<Packet, PacketError>> {
        let start = self.buf.windows(2).position(|w| w == HEAD);
        let Some(start) = start else {
            // Keep a trailing 0x55 that may begin the next header.
            let keep = usize::from(self.buf.last() == Some(&HEAD[0]));
            let drop = self.buf.len() - keep;
            self.buf.drain(..drop);
            return None;
        };
        self.buf.drain(..start);
        if self.buf.len() < 4 {
            return None;
        }
        let cmd = self.buf[2];
        let (len, head) = if self.requests && Packet::long(cmd) {
            if self.buf.len() < 5 {
                return None;
            }
            (
                usize::from(u16::from_be_bytes([self.buf[3], self.buf[4]])),
                5,
            )
        } else {
            (usize::from(self.buf[3]), 4)
        };
        let total = head + len + 3;
        if self.buf.len() < total {
            return None;
        }
        let frame: Vec<u8> = self.buf.drain(..total).collect();
        if frame[total - 2..] != TAIL {
            // Not a real frame boundary: resync one byte after this header.
            let mut rest = frame[2..].to_vec();
            rest.extend_from_slice(&self.buf);
            self.buf = rest;
            return Some(Err(PacketError::BadTail(cmd)));
        }
        let packet = Packet {
            cmd,
            payload: frame[head..head + len].to_vec(),
        };
        let got = frame[head + len];
        let expected = packet.checksum();
        if got != expected {
            return Some(Err(PacketError::Checksum { cmd, expected, got }));
        }
        Some(Ok(packet))
    }

    /// Drain all complete packets currently buffered, discarding corrupt frames.
    pub fn drain_packets(&mut self) -> Vec<Packet> {
        let mut out = Vec::new();
        while let Some(res) = self.next_packet() {
            if let Ok(p) = res {
                out.push(p);
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    #[test]
    fn encodes_connect_with_prefix() {
        let p = Packet::small(cmd::CONNECT, &[0x01]);
        assert_eq!(
            p.encode_request(),
            vec![0x03, 0x55, 0x55, 0xC1, 0x01, 0x01, 0xC1, 0xAA, 0xAA]
        );
        assert_eq!(
            p.encode(),
            vec![0x55, 0x55, 0xC1, 0x01, 0x01, 0xC1, 0xAA, 0xAA]
        );
    }

    #[test]
    fn checksum_is_xor() {
        let p = Packet::small(0x21, &[0x03]);
        assert_eq!(p.checksum(), 0x21 ^ 0x01 ^ 0x03);
    }

    #[test]
    fn rejects_long_payload() {
        assert_eq!(
            Packet::new(0x85, vec![0; 256]),
            Err(PacketError::PayloadTooLong(256))
        );
    }

    #[test]
    fn gray_row_has_a_16_bit_length() {
        let p = Packet::new(cmd::ROW_GRAY, vec![0x11; 300]).unwrap();
        let frame = p.encode();
        assert_eq!(frame[..5], [0x55, 0x55, 0x8D, 0x01, 0x2C]);
        assert_eq!(frame.len(), 300 + 8);
        // 300 equal bytes cancel out; the checksum is cmd ^ len_hi ^ len_lo.
        assert_eq!(frame[305], 0x8D ^ 0x01 ^ 0x2C);
        let mut d = Decoder::for_requests();
        d.push(&frame[..200]);
        assert_eq!(d.next_packet(), None);
        d.push(&frame[200..]);
        assert_eq!(d.next_packet(), Some(Ok(p)));
        assert!(Packet::new(cmd::ROW_GRAY, vec![0; 65536]).is_err());
    }

    #[test]
    fn decodes_split_and_joined_reads() {
        let a = Packet::small(0x31, &[0x01]).encode();
        let b = Packet::small(0xB3, &[0x00, 0x01, 0x64, 0x64]).encode();
        let mut stream = vec![0x00, 0x13];
        stream.extend_from_slice(&a);
        stream.extend_from_slice(&b);
        let mut d = Decoder::new();
        let mut got = Vec::new();
        for chunk in stream.chunks(3) {
            d.push(chunk);
            got.extend(d.drain_packets());
        }
        assert_eq!(got.len(), 2);
        assert_eq!(got[0].cmd, 0x31);
        assert_eq!(got[1].payload, vec![0x00, 0x01, 0x64, 0x64]);
        assert_eq!(d.buffered(), 0);
    }

    #[test]
    fn reports_bad_checksum_then_recovers() {
        let mut bad = Packet::small(0x31, &[0x01]).encode();
        bad[5] ^= 0xFF;
        let good = Packet::small(0x33, &[0x01]).encode();
        let mut d = Decoder::new();
        d.push(&bad);
        d.push(&good);
        assert!(matches!(
            d.next_packet(),
            Some(Err(PacketError::Checksum { .. }))
        ));
        assert_eq!(d.next_packet().unwrap().unwrap().cmd, 0x33);
    }

    proptest! {
        #[test]
        fn roundtrip(cmd in any::<u8>(), payload in proptest::collection::vec(any::<u8>(), 0..=255)) {
            let p = Packet::new(cmd, payload).unwrap();
            let mut d = Decoder::for_requests();
            d.push(&p.encode());
            prop_assert_eq!(d.next_packet(), Some(Ok(p)));
        }

        #[test]
        fn decoder_never_panics(bytes in proptest::collection::vec(any::<u8>(), 0..600)) {
            let mut d = Decoder::new();
            d.push(&bytes);
            let _ = d.drain_packets();
        }
    }
}
