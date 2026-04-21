//! ProductionDeck — Stream Deck Neo (PID 0x009A), Main Protocol.

#![no_std]
#![no_main]

use defmt_rtt as _;
use embassy_executor::Spawner;
use panic_halt as _;

const DEVICE: productiondeck::device::Device = productiondeck::device::Device::Neo;

extern crate productiondeck;
use productiondeck::*;

#[embassy_executor::main]
async fn main(spawner: Spawner) {
    config::init_runtime_device(DEVICE);
    let p = embassy_rp::init(Default::default());
    let mut supervisor = supervisor::AppSupervisor::new_for_device(DEVICE);
    supervisor.print_startup_banner();
    match hardware::init_hardware_tasks_for_device(&spawner, p, DEVICE).await {
        Ok(()) => supervisor.print_init_success(),
        Err(_) => core::panic!("Hardware init failed"),
    }
    supervisor.run().await;
}
