/// Cora protocol implementation.
///
/// Frame layout (from socketWrapper.ts):
/// offset  size   field
///      0     4   magic       [0x43, 0x93, 0x8a, 0x41]
///      4     2   flags       CoraMessageFlags (u16 LE)
///      6     1   hid_op      CoraHidOp (u8)
///      7     1   (reserved)
///      8     4   message_id  u32 LE  (aka STAN)
///     12     4   payload_len u32 LE
///     16     N   payload
use bytes::{Buf, BufMut, Bytes, BytesMut};
use tokio_util::codec::{Decoder, Encoder};

pub const CORA_MAGIC: [u8; 4] = [0x43, 0x93, 0x8a, 0x41];
pub const HEADER_LEN: usize = 16;

// ---------------------------------------------------------------------------
// Flags (u16, bitfield)
// ---------------------------------------------------------------------------
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CoraFlags(pub u16);

impl CoraFlags {
    /// Payload is for child HID device (verbatim pass-through)
    pub const VERBATIM: CoraFlags = CoraFlags(0x8000);
    /// Out: host requests an ACK
    #[allow(dead_code)]
    pub const REQ_ACK: CoraFlags = CoraFlags(0x4000);
    /// In: unit ACK/NAK response to REQ_ACK
    pub const ACK_NAK: CoraFlags = CoraFlags(0x0200);
    /// In: unit response to GET_REPORT op
    pub const RESULT: CoraFlags = CoraFlags(0x0100);
    pub const NONE: CoraFlags = CoraFlags(0x0000);

    pub fn contains(self, other: CoraFlags) -> bool {
        (self.0 & other.0) != 0
    }
}

// ---------------------------------------------------------------------------
// HID operation
// ---------------------------------------------------------------------------
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum HidOp {
    Write = 0x00,       // hid_write
    SendReport = 0x01,  // hid_send_feature_report
    GetReport = 0x02,   // hid_get_feature_report
}

impl HidOp {
    pub fn from_u8(v: u8) -> Option<Self> {
        match v {
            0x00 => Some(HidOp::Write),
            0x01 => Some(HidOp::SendReport),
            0x02 => Some(HidOp::GetReport),
            _ => None,
        }
    }
}

// ---------------------------------------------------------------------------
// Message
// ---------------------------------------------------------------------------
#[derive(Debug, Clone)]
pub struct CoraMessage {
    pub flags: CoraFlags,
    pub hid_op: HidOp,
    pub message_id: u32,
    pub payload: Bytes,
}

impl CoraMessage {
    pub fn new(flags: CoraFlags, hid_op: HidOp, message_id: u32, payload: impl Into<Bytes>) -> Self {
        Self {
            flags,
            hid_op,
            message_id,
            payload: payload.into(),
        }
    }

    /// Encode this message into a BytesMut (header + payload).
    pub fn encode_into(&self, dst: &mut BytesMut) {
        dst.put_slice(&CORA_MAGIC);
        dst.put_u16_le(self.flags.0);
        dst.put_u8(self.hid_op as u8);
        dst.put_u8(0); // reserved
        dst.put_u32_le(self.message_id);
        dst.put_u32_le(self.payload.len() as u32);
        dst.put_slice(&self.payload);
    }
}

// ---------------------------------------------------------------------------
// tokio_util Codec
// ---------------------------------------------------------------------------
/// Framing codec for Cora messages over a TCP stream.
/// Handles buffered, incomplete, and mis-aligned data (re-syncs on magic).
#[derive(Default)]
pub struct CoraCodec;

impl Decoder for CoraCodec {
    type Item = CoraMessage;
    type Error = std::io::Error;

    fn decode(&mut self, src: &mut BytesMut) -> Result<Option<Self::Item>, Self::Error> {
        loop {
            if src.len() < HEADER_LEN {
                return Ok(None);
            }

            // Find magic bytes
            let magic_pos = src
                .windows(CORA_MAGIC.len())
                .position(|w| w == CORA_MAGIC);

            match magic_pos {
                None => {
                    // Keep last 3 bytes in case the magic starts here
                    let keep = src.len().min(CORA_MAGIC.len() - 1);
                    let discard = src.len() - keep;
                    src.advance(discard);
                    return Ok(None);
                }
                Some(0) => {
                    // Magic is at the start – proceed to parse
                }
                Some(pos) => {
                    // Discard bytes before magic
                    src.advance(pos);
                    continue;
                }
            }

            // We have magic at position 0; check we have a full header
            if src.len() < HEADER_LEN {
                return Ok(None);
            }

            let payload_len = u32::from_le_bytes([src[12], src[13], src[14], src[15]]) as usize;

            if src.len() < HEADER_LEN + payload_len {
                return Ok(None);
            }

            // Consume header
            src.advance(4); // magic
            let flags = CoraFlags(u16::from_le_bytes([src[0], src[1]]));
            src.advance(2);
            let hid_op_byte = src[0];
            src.advance(1);
            src.advance(1); // reserved
            let message_id = u32::from_le_bytes([src[0], src[1], src[2], src[3]]);
            src.advance(4);
            src.advance(4); // payload_len already read above

            let hid_op = HidOp::from_u8(hid_op_byte).unwrap_or(HidOp::Write);
            let payload = src.split_to(payload_len).freeze();

            return Ok(Some(CoraMessage {
                flags,
                hid_op,
                message_id,
                payload,
            }));
        }
    }
}

impl Encoder<CoraMessage> for CoraCodec {
    type Error = std::io::Error;

    fn encode(&mut self, item: CoraMessage, dst: &mut BytesMut) -> Result<(), Self::Error> {
        item.encode_into(dst);
        Ok(())
    }
}
