//! ProductionDeck — Stream Deck Studio over Cora (W5500-EVB-Pico).
//!
//! Requires the `cora-w5500` feature:
//! `cargo build -p productiondeck-firmware --target thumbv6m-none-eabi --features cora-w5500 --bin studio-cora`

#![no_std]
#![no_main]

use defmt_rtt as _;
use embassy_executor::Spawner;
use panic_halt as _;
use productiondeck::device::Device;

const DEVICE: Device = Device::Studio;

#[embassy_executor::main]
async fn main(spawner: Spawner) {
    productiondeck::config::init_runtime_device(DEVICE);

    #[cfg(feature = "cora-w5500")]
    {
        let p = embassy_rp::init(Default::default());
        productiondeck::network::w5500::run::<4096, 16384>(spawner, p, DEVICE.core_model()).await;
    }

    #[cfg(not(feature = "cora-w5500"))]
    {
        let _ = spawner;
        defmt::error!("studio-cora requires the cora-w5500 feature");
    }
}
