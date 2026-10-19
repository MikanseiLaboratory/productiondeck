//! ProductionDeck — Stream Deck Neo (PID `0x009A`), Main Protocol.

#![no_std]
#![no_main]

use defmt_rtt as _;
use embassy_executor::Spawner;
use panic_halt as _;
use productiondeck::device::Device;
use productiondeck::entry::run_single_core_quiet;

const DEVICE: Device = Device::Neo;

#[embassy_executor::main]
async fn main(spawner: Spawner) {
    run_single_core_quiet(spawner, DEVICE).await;
}
