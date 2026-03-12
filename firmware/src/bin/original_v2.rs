//! ProductionDeck - StreamDeck Original V2 Compatible Firmware
//!
//! This binary builds firmware specifically for StreamDeck Original V2 compatibility:
//! - 15 keys in 5x3 layout
//! - 72x72 pixel images per key
//! - USB VID:PID 0x0fd9:0x006d
//! - V2 JPEG protocol

#![no_std]
#![no_main]

use defmt::*;
use defmt_rtt as _;
use embassy_executor::Spawner;
use panic_halt as _;

// Set compile-time device selection
const DEVICE: productiondeck::device::Device = productiondeck::device::Device::OriginalV2;

// Import all modules from library
extern crate productiondeck;
use productiondeck::*;

// USB interrupt binding
// Use Irqs from the library to avoid duplicate definitions

/// Main application entry point for StreamDeck Original V2
#[embassy_executor::main]
async fn main(spawner: Spawner) {
    // Initialize hardware
    let p = embassy_rp::init(Default::default());

    // Create application supervisor for Original V2
    let mut supervisor = supervisor::AppSupervisor::new_for_device(DEVICE);

    // Print startup information
    supervisor.print_startup_banner();

    // Initialize and spawn all hardware tasks for Original V2
    match hardware::init_hardware_tasks_for_device(&spawner, p, DEVICE).await {
        Ok(()) => {
            info!("StreamDeck Original V2 firmware initialized successfully");
            supervisor.print_init_success();
        }
        Err(e) => {
            error!("Failed to spawn hardware tasks: {:?}", e);
            core::panic!("Hardware initialization failed");
        }
    }

    // Run the main supervisor loop
    supervisor.run().await;
}
