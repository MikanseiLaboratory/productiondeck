//! ProductionDeck - StreamDeck Revised Mini Compatible Firmware
//!
//! This binary builds firmware specifically for StreamDeck Revised Mini compatibility:
//! - 6 keys in 3x2 layout
//! - 80x80 pixel images per key  
//! - USB VID:PID 0x0fd9:0x0080
//! - V1 BMP protocol

#![no_std]
#![no_main]

use defmt::*;
use defmt_rtt as _;
use embassy_executor::Spawner;
use panic_halt as _;

// Set compile-time device selection
const DEVICE: productiondeck::device::Device = productiondeck::device::Device::RevisedMini;

// Import all modules from library
extern crate productiondeck;
use productiondeck::*;

// Use Irqs from the library to avoid duplicate definitions

/// Main application entry point for StreamDeck Revised Mini
#[embassy_executor::main]
async fn main(spawner: Spawner) {
    // Initialize hardware
    let p = embassy_rp::init(Default::default());

    // Create application supervisor for Revised Mini
    let mut supervisor = supervisor::AppSupervisor::new_for_device(DEVICE);

    // Print startup information
    supervisor.print_startup_banner();

    // Initialize and spawn all hardware tasks for Revised Mini
    match hardware::init_hardware_tasks_for_device(&spawner, p, DEVICE).await {
        Ok(()) => {
            info!("StreamDeck Revised Mini firmware initialized successfully");
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
