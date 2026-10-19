//! Cora transport core: framing, session state machine and legacy framing.

pub mod frame;
pub mod legacy;
pub mod session;

pub use frame::{
    write_header, CoraFlags, CoraFrame, CoraParser, HidOp, CORA_HEADER, CORA_MAGIC,
    CORA_MAX_PAYLOAD,
};
pub use legacy::{LegacyFrame, LegacyParser};
pub use session::{CoraSession, FeatureStyle, PendingTx};

use crate::protocol::ProtocolError;

/// Errors produced by the Cora layer.
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CoraError {
    /// Receive or transmit buffer overflow.
    BufferFull,
    /// `hidOp` byte was not one of the three known values.
    UnknownHidOp,
    /// Frame was structurally invalid.
    Malformed,
    /// Underlying protocol error.
    Protocol(ProtocolError),
}

impl From<ProtocolError> for CoraError {
    fn from(value: ProtocolError) -> Self {
        CoraError::Protocol(value)
    }
}
