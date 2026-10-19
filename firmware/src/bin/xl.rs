//! ProductionDeck — Stream Deck XL (PID `0x006c`).

#![no_std]
#![no_main]

use defmt_rtt as _;
use embassy_executor::Spawner;
use panic_halt as _;
use productiondeck::device::Device;
use productiondeck::entry::run_single_core;

const DEVICE: Device = Device::Xl;

#[embassy_executor::main]
async fn main(spawner: Spawner) {
    run_single_core(spawner, DEVICE).await;
}
