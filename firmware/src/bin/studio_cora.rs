//! Placeholder Stream Deck Studio Cora firmware binary.
//!
//! The real implementation is tracked in the workspace plan; this file exists
//! so the workspace manifest resolves while the core/host layers are built.

#![no_std]
#![no_main]

use defmt_rtt as _;
use panic_halt as _;

#[embassy_executor::main]
async fn main(_spawner: embassy_executor::Spawner) {}
