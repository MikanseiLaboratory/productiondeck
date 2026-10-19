//! Cora framing.
//!
//! ```text
//! offset size field
//!      0    4 magic 43 93 8A 41
//!      4    2 flags (u16 LE)
//!      6    1 hidOp
//!      7    1 reserved
//!      8    4 messageId (u32 LE)
//!     12    4 payloadLen (u32 LE)
//!     16    N payload
//! ```

use crate::cora::CoraError;
use heapless::Vec;

/// Cora magic bytes.
pub const CORA_MAGIC: [u8; 4] = [0x43, 0x93, 0x8a, 0x41];
/// Cora header length.
pub const CORA_HEADER: usize = 16;
/// Maximum payload the core accepts (matches the 1024-byte HID output report).
pub const CORA_MAX_PAYLOAD: usize = 1024;

bitflags::bitflags! {
    /// Cora message flags. A bitfield, never an enum: combinations such as
    /// `REQ_ACK | VERBATIM` are common on image writes.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct CoraFlags: u16 {
        /// Payload is a verbatim HID report (child device / primary output).
        const VERBATIM = 0x8000;
        /// Host requests an ACK for this message.
        const REQ_ACK = 0x4000;
        /// Response to a `REQ_ACK` message.
        const ACK_NAK = 0x0200;
        /// Response to a `GET_REPORT` operation.
        const RESULT = 0x0100;
    }
}

/// Cora HID operation.
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum HidOp {
    /// `hid_write` (output report).
    Write = 0x00,
    /// `hid_send_feature_report`.
    SendReport = 0x01,
    /// `hid_get_feature_report`.
    GetReport = 0x02,
}

impl HidOp {
    pub const fn from_u8(value: u8) -> Option<Self> {
        match value {
            0x00 => Some(HidOp::Write),
            0x01 => Some(HidOp::SendReport),
            0x02 => Some(HidOp::GetReport),
            _ => None,
        }
    }
}

/// A borrowed Cora frame.
#[derive(Debug, Clone, Copy)]
pub struct CoraFrame<'a> {
    pub flags: CoraFlags,
    pub hid_op: HidOp,
    pub message_id: u32,
    pub payload: &'a [u8],
}

/// Write a Cora header for a payload of `payload_len` bytes.
///
/// Returns the header length (16) when `out` is large enough. The caller is
/// expected to write the payload at `out[16..16 + payload_len]`.
pub fn write_header(
    out: &mut [u8],
    flags: CoraFlags,
    hid_op: HidOp,
    message_id: u32,
    payload_len: usize,
) -> Option<usize> {
    if out.len() < CORA_HEADER {
        return None;
    }
    out[..4].copy_from_slice(&CORA_MAGIC);
    out[4..6].copy_from_slice(&flags.bits().to_le_bytes());
    out[6] = hid_op as u8;
    out[7] = 0;
    out[8..12].copy_from_slice(&message_id.to_le_bytes());
    out[12..16].copy_from_slice(&(payload_len as u32).to_le_bytes());
    Some(CORA_HEADER)
}

/// Streaming Cora frame reader.
#[derive(Debug)]
pub struct CoraParser<const RX: usize> {
    buf: Vec<u8, RX>,
}

impl<const RX: usize> Default for CoraParser<RX> {
    fn default() -> Self {
        Self::new()
    }
}

impl<const RX: usize> CoraParser<RX> {
    pub const fn new() -> Self {
        Self { buf: Vec::new() }
    }

    pub fn clear(&mut self) {
        self.buf.clear();
    }

    pub fn buffered(&self) -> usize {
        self.buf.len()
    }

    /// Append received bytes, resynchronising if the buffer overflows.
    pub fn push(&mut self, bytes: &[u8]) -> Result<(), CoraError> {
        if self.buf.extend_from_slice(bytes).is_ok() {
            return Ok(());
        }
        // Overflow: drop bytes before the first magic if possible, otherwise
        // drop the old half. Cora is resynchronisable by magic.
        self.compact();
        if self.buf.extend_from_slice(bytes).is_ok() {
            return Ok(());
        }
        let drop = self.buf.len() / 2;
        self.buf.drain(..drop);
        self.buf
            .extend_from_slice(bytes)
            .map_err(|_| CoraError::BufferFull)
    }

    fn compact(&mut self) {
        if let Some(idx) = find_magic(&self.buf) {
            if idx > 0 {
                self.buf.drain(..idx);
            }
        }
    }

    /// Drain every complete frame, invoking `f` for each.
    ///
    /// Non-magic bytes (including the bare `00 00` greeting some devices send
    /// before the first frame) are skipped.
    pub fn drain<F>(&mut self, mut f: F) -> Result<(), CoraError>
    where
        F: FnMut(CoraFrame<'_>) -> Result<(), CoraError>,
    {
        loop {
            if self.buf.len() < CORA_MAGIC.len() {
                break;
            }
            match find_magic(&self.buf) {
                None => {
                    let keep = self.buf.len().min(3);
                    let drop = self.buf.len() - keep;
                    self.buf.drain(..drop);
                    break;
                }
                Some(0) => {}
                Some(idx) => {
                    self.buf.drain(..idx);
                    continue;
                }
            }
            if self.buf.len() < CORA_HEADER {
                break;
            }
            let payload_len =
                u32::from_le_bytes([self.buf[12], self.buf[13], self.buf[14], self.buf[15]])
                    as usize;
            if payload_len > CORA_MAX_PAYLOAD {
                // Bogus length: skip this magic and try to resync.
                self.buf.drain(..CORA_MAGIC.len());
                continue;
            }
            if self.buf.len() < CORA_HEADER + payload_len {
                break;
            }
            let flags =
                CoraFlags::from_bits_truncate(u16::from_le_bytes([self.buf[4], self.buf[5]]));
            let hid_op = HidOp::from_u8(self.buf[6]).ok_or(CoraError::UnknownHidOp)?;
            let message_id =
                u32::from_le_bytes([self.buf[8], self.buf[9], self.buf[10], self.buf[11]]);
            let result = {
                let payload = &self.buf[CORA_HEADER..CORA_HEADER + payload_len];
                let frame = CoraFrame {
                    flags,
                    hid_op,
                    message_id,
                    payload,
                };
                f(frame)
            };
            self.buf.drain(..CORA_HEADER + payload_len);
            result?;
        }
        Ok(())
    }
}

fn find_magic(buf: &[u8]) -> Option<usize> {
    buf.windows(CORA_MAGIC.len()).position(|w| w == CORA_MAGIC)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frame(flags: CoraFlags, hid_op: HidOp, id: u32, payload: &[u8]) -> Vec<u8, 128> {
        let mut buf = Vec::new();
        let mut header = [0u8; CORA_HEADER];
        write_header(&mut header, flags, hid_op, id, payload.len()).unwrap();
        buf.extend_from_slice(&header).unwrap();
        buf.extend_from_slice(payload).unwrap();
        buf
    }

    #[test]
    fn flags_are_a_bitfield() {
        let combined = CoraFlags::REQ_ACK | CoraFlags::VERBATIM;
        assert!(combined.contains(CoraFlags::REQ_ACK));
        assert!(combined.contains(CoraFlags::VERBATIM));
        assert_eq!(combined.bits(), 0xc000);
    }

    #[test]
    fn round_trip() {
        let bytes = frame(CoraFlags::VERBATIM, HidOp::Write, 0x1234, b"hello");
        let mut parser = CoraParser::<128>::new();
        parser.push(&bytes).unwrap();
        let mut seen = 0;
        parser
            .drain(|f| {
                assert_eq!(f.flags, CoraFlags::VERBATIM);
                assert_eq!(f.hid_op, HidOp::Write);
                assert_eq!(f.message_id, 0x1234);
                assert_eq!(f.payload, b"hello");
                seen += 1;
                Ok(())
            })
            .unwrap();
        assert_eq!(seen, 1);
        assert_eq!(parser.buffered(), 0);
    }

    #[test]
    fn skips_greeting_and_resyncs() {
        let mut bytes = Vec::<u8, 256>::new();
        bytes.extend_from_slice(&[0x00, 0x00, 0x00]).unwrap();
        let f = frame(CoraFlags::empty(), HidOp::SendReport, 1, b"x");
        bytes.extend_from_slice(&f).unwrap();
        let mut parser = CoraParser::<256>::new();
        parser.push(&bytes).unwrap();
        let mut count = 0;
        parser
            .drain(|f| {
                assert_eq!(f.payload, b"x");
                count += 1;
                Ok(())
            })
            .unwrap();
        assert_eq!(count, 1);
    }

    #[test]
    fn waits_for_partial_payload() {
        let f = frame(CoraFlags::empty(), HidOp::GetReport, 7, b"abcdef");
        let mut parser = CoraParser::<64>::new();
        parser.push(&f[..10]).unwrap();
        let mut count = 0;
        parser
            .drain(|_| {
                count += 1;
                Ok(())
            })
            .unwrap();
        assert_eq!(count, 0);
        parser.push(&f[10..]).unwrap();
        parser
            .drain(|_| {
                count += 1;
                Ok(())
            })
            .unwrap();
        assert_eq!(count, 1);
    }

    #[test]
    fn bogus_length_resyncs() {
        let mut bytes = Vec::<u8, 256>::new();
        let mut header = [0u8; CORA_HEADER];
        write_header(&mut header, CoraFlags::empty(), HidOp::Write, 0, 9999).unwrap();
        bytes.extend_from_slice(&header).unwrap();
        bytes.extend_from_slice(&[0, 0, 0, 0]).unwrap();
        let good = frame(CoraFlags::empty(), HidOp::Write, 1, b"ok");
        bytes.extend_from_slice(&good).unwrap();
        let mut parser = CoraParser::<256>::new();
        parser.push(&bytes).unwrap();
        let mut payloads = 0;
        parser
            .drain(|f| {
                assert_eq!(f.payload, b"ok");
                payloads += 1;
                Ok(())
            })
            .unwrap();
        assert_eq!(payloads, 1);
    }
}
