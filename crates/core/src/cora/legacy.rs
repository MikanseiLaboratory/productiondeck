//! Legacy (pre-Cora) framing.
//!
//! Older Studio firmware (for example 2.01.000) speaks a headerless fixed-size
//! packet protocol on the same TCP port. Detection is by the keep-alive prefix
//! `01 0A` rather than the Cora magic.
//!
//! The exact packet sizes must be confirmed by capture (U9 / C6); the parser is
//! therefore parameterised over the packet size and defaults to the values used
//! by `node-elgato-stream-deck` (512-byte reads, 1024-byte writes).

use crate::cora::CoraError;
use heapless::Vec;

/// A borrowed legacy packet.
#[derive(Debug, Clone, Copy)]
pub struct LegacyFrame<'a> {
    pub payload: &'a [u8],
}

impl LegacyFrame<'_> {
    /// True when this packet is a keep-alive (`01 0A ...`).
    pub fn is_keep_alive(&self) -> bool {
        self.payload.len() >= 2 && self.payload[0] == 0x01 && self.payload[1] == 0x0a
    }

    /// Connection number carried at payload byte 5 of a keep-alive.
    pub fn connection_no(&self) -> Option<u8> {
        if self.is_keep_alive() && self.payload.len() > 5 {
            Some(self.payload[5])
        } else {
            None
        }
    }
}

/// Streaming legacy packet reader.
#[derive(Debug)]
pub struct LegacyParser<const N: usize> {
    buf: Vec<u8, N>,
    packet_size: usize,
}

impl<const N: usize> LegacyParser<N> {
    /// Create a parser that emits `packet_size`-byte packets.
    pub const fn new(packet_size: usize) -> Self {
        Self {
            buf: Vec::new(),
            packet_size,
        }
    }

    pub fn clear(&mut self) {
        self.buf.clear();
    }

    pub fn buffered(&self) -> usize {
        self.buf.len()
    }

    pub fn push(&mut self, bytes: &[u8]) -> Result<(), CoraError> {
        self.buf
            .extend_from_slice(bytes)
            .map_err(|_| CoraError::BufferFull)
    }

    /// Drain every complete fixed-size packet.
    pub fn drain<F>(&mut self, mut f: F) -> Result<(), CoraError>
    where
        F: FnMut(LegacyFrame<'_>) -> Result<(), CoraError>,
    {
        while self.buf.len() >= self.packet_size {
            {
                let payload = &self.buf[..self.packet_size];
                f(LegacyFrame { payload })?;
            }
            self.buf.drain(..self.packet_size);
        }
        Ok(())
    }
}

/// Build a legacy keep-alive ACK into `out`, returning its length.
pub fn build_keep_alive_ack(
    out: &mut [u8],
    connection_no: u8,
    packet_size: usize,
) -> Option<usize> {
    if out.len() < packet_size {
        return None;
    }
    out[..packet_size].fill(0);
    out[0] = 0x03;
    out[1] = 0x1a;
    out[2] = connection_no;
    Some(packet_size)
}

/// Build a legacy keep-alive into `out`, returning its length.
pub fn build_keep_alive(out: &mut [u8], connection_no: u8, packet_size: usize) -> Option<usize> {
    if out.len() < packet_size {
        return None;
    }
    out[..packet_size].fill(0);
    out[0] = 0x01;
    out[1] = 0x0a;
    out[2] = 0x02;
    out[3] = 0x00;
    out[4] = 0x01;
    out[5] = connection_no;
    Some(packet_size)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn drains_fixed_packets() {
        let mut bytes = [0u8; 32];
        for i in (0..bytes.len()).step_by(16) {
            let packet = &mut bytes[i..i + 16];
            packet[0] = 0x01;
            packet[1] = 0x0a;
            packet[5] = 7;
        }
        let mut parser = LegacyParser::<64>::new(16);
        parser.push(&bytes).unwrap();
        let mut count = 0;
        parser
            .drain(|f| {
                assert!(f.is_keep_alive());
                assert_eq!(f.connection_no(), Some(7));
                count += 1;
                Ok(())
            })
            .unwrap();
        assert_eq!(count, 2);
    }

    #[test]
    fn builds_ack() {
        let mut out = [0u8; 32];
        let n = build_keep_alive_ack(&mut out, 9, 32).unwrap();
        assert_eq!(n, 32);
        assert_eq!(&out[..3], &[0x03, 0x1a, 9]);
    }
}
