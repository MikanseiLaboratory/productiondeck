//! ProductionDeck — Stream Deck Studio (PID `0x00AA`), USB HID profile.
//!
//! The protocol surface is complete; the 16x2 matrix / encoder / NFC board
//! profile is still board-specific. This binary brings up USB HID and a display
//! drain sink so the official app can recognise and drive the device.

#![no_std]
#![no_main]

use defmt_rtt as _;
use embassy_executor::Spawner;
use panic_halt as _;
use productiondeck::device::Device;
use productiondeck::entry::run_single_core_quiet;

const DEVICE: Device = Device::Studio;

#[embassy_executor::main]
async fn main(spawner: Spawner) {
    run_single_core_quiet(spawner, DEVICE).await;
}
