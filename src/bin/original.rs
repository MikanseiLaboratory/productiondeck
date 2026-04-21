//! ProductionDeck — Stream Deck first generation (PID `0x0060`).

#![no_std]
#![no_main]

use defmt_rtt as _;
use embassy_executor::Spawner;
use panic_halt as _;
use productiondeck::device::Device;
use productiondeck::entry::run_single_core;

const DEVICE: Device = Device::Original;

#[embassy_executor::main]
async fn main(spawner: Spawner) {
    run_single_core(spawner, DEVICE).await;
}
