//! Host-side Cora support.
//!
//! - [`client`]: TCP client / probe
//! - [`server`]: TCP server used by the emulator
//! - [`discovery`]: mDNS advertisement and browsing
//! - [`emulator`]: Studio / Network Dock emulator
//! - [`command`]: commands decoded from host output reports

#![forbid(unsafe_code)]

pub mod client;
pub mod command;
pub mod discovery;
pub mod emulator;
pub mod frame;
pub mod render;
pub mod server;

pub use productiondeck_core as core;

pub use core::cora::{
    CoraFlags, CoraFrame, CoraParser, FeatureStyle, HidOp, CORA_HEADER, CORA_MAGIC,
};
pub use core::feature::{ChildInfo, FirmwareVersions, Identity};
pub use core::input::InputEvent;
pub use core::model;
pub use core::protocol::OutputSink;
