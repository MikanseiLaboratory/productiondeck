//! # productiondeck-core
//!
//! Sans-I/O protocol core shared by the RP2040 firmware and the host-side
//! Cora client/server/emulator.
//!
//! The core never touches USB, sockets, clocks or the filesystem. Callers feed
//! it bytes and input events and pull encoded reports / Cora frames back out.
//!
//! Layout:
//! - [`model`]: device model definitions (including Stream Deck Studio)
//! - [`input`]: transport-independent input events
//! - [`protocol`]: HID report handling (`Gen2Handler`, ...)
//! - [`cora`]: Cora framing, session state machine and legacy framing
//! - [`feature`]: feature-report builders for USB / Cora primary / child styles
//! - [`image`]: chunked JPEG assembly
//! - [`hid_descriptor`]: per-family HID report descriptors

#![no_std]
#![forbid(unsafe_code)]

pub mod cora;
pub mod feature;
pub mod hid_descriptor;
pub mod image;
pub mod input;
pub mod model;
pub mod protocol;

pub use model::{ModelId, ModelInfo, ProtocolFamily, STUDIO};
