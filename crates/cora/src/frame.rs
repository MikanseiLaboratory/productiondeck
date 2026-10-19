//! Owned Cora frame used by host-side code.

use productiondeck_core::cora::{CoraFlags, CoraFrame, HidOp};

/// An owned Cora frame (payload copied out of the receive buffer).
#[derive(Debug, Clone)]
pub struct OwnedFrame {
    pub flags: CoraFlags,
    pub hid_op: HidOp,
    pub message_id: u32,
    pub payload: Vec<u8>,
}

impl OwnedFrame {
    pub fn is_keep_alive(&self) -> bool {
        self.payload.len() >= 2 && self.payload[0] == 0x01 && self.payload[1] == 0x0a
    }

    pub fn connection_no(&self) -> Option<u8> {
        if self.is_keep_alive() {
            self.payload.get(5).copied()
        } else {
            None
        }
    }

    /// Report id for a feature response, taking the VERBATIM prefix into account.
    pub fn feature_report_id(&self) -> Option<u8> {
        if self.payload.is_empty() {
            return None;
        }
        if self.flags.contains(CoraFlags::VERBATIM) || self.payload[0] != 0x03 {
            Some(self.payload[0])
        } else {
            self.payload.get(1).copied()
        }
    }
}

impl From<CoraFrame<'_>> for OwnedFrame {
    fn from(value: CoraFrame<'_>) -> Self {
        Self {
            flags: value.flags,
            hid_op: value.hid_op,
            message_id: value.message_id,
            payload: value.payload.to_vec(),
        }
    }
}
