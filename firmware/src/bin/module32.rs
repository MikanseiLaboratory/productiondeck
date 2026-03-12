#![allow(unreachable_code)]
//! ProductionDeck - Stream Deck Module 32 Compatible Firmware
//!
//! This binary builds firmware specifically for Stream Deck Module 32 compatibility:
//! - 32 keys in 8x4 layout
//! - 96x96 pixel images per key (rotate 180° per spec)
//! - USB VID:PID 0x0FD9:0x00BA
//! - Module HID protocol (Input 512B, Output 1024B, Feature 32B)

#![no_std]
#![no_main]

use defmt::*;
use defmt_rtt as _;
use embassy_executor::Executor;
use embassy_rp::multicore::{spawn_core1, Stack};
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::channel::Channel;
use panic_halt as _;
use static_cell::StaticCell;

// Set compile-time device selection
const DEVICE: productiondeck::device::Device = productiondeck::device::Device::Module32Keys;

// Import all modules from library
extern crate productiondeck;
use productiondeck::*;

// Multicore setup
static mut CORE1_STACK: Stack<4096> = Stack::new();
static EXECUTOR0: StaticCell<Executor> = StaticCell::new();
static EXECUTOR1: StaticCell<Executor> = StaticCell::new();

// Inter-core communication channel for image processing
static IMAGE_CHANNEL: Channel<CriticalSectionRawMutex, productiondeck::types::DisplayCommand, 8> =
    Channel::new();

/// Main application entry point for Stream Deck Module 32 with multicore support
#[cortex_m_rt::entry]
fn main() -> ! {
    // Initialize hardware
    let p = embassy_rp::init(Default::default());

    // Create application supervisor for Module 32
    let supervisor = supervisor::AppSupervisor::new_for_device(DEVICE);

    // Print startup information
    supervisor.print_startup_banner();

    // Spawn core 1 for image processing and display tasks
    spawn_core1(
        p.CORE1,
        unsafe { &mut *core::ptr::addr_of_mut!(CORE1_STACK) },
        move || {
            let executor1 = EXECUTOR1.init(Executor::new());
            executor1.run(|spawner| {
                unwrap!(spawner.spawn(core1_image_processing_task()));
            });
        },
    );

    // Run core 0 for USB, buttons, and supervision
    let executor0 = EXECUTOR0.init(Executor::new());
    executor0.run(|spawner| {
        unwrap!(spawner.spawn(core0_main_task(supervisor)));
        // Also spawn the USB task directly
        unwrap!(spawner.spawn(usb::usb_task_for_device(
            embassy_rp::usb::Driver::new(p.USB, crate::Irqs),
            embassy_rp::gpio::Output::new(p.PIN_25, embassy_rp::gpio::Level::Low),
            DEVICE
        )));
        // Spawn button task for Module 32 (matrix 8x4 = 32 buttons)
        unwrap!(spawner.spawn(buttons::button_task_matrix_8x4(
            // rows: 4 outputs
            embassy_rp::gpio::Output::new(p.PIN_2, embassy_rp::gpio::Level::High),
            embassy_rp::gpio::Output::new(p.PIN_3, embassy_rp::gpio::Level::High),
            embassy_rp::gpio::Output::new(p.PIN_7, embassy_rp::gpio::Level::High),
            embassy_rp::gpio::Output::new(p.PIN_9, embassy_rp::gpio::Level::High),
            // cols: 8 inputs with pull-ups
            embassy_rp::gpio::Input::new(p.PIN_4, embassy_rp::gpio::Pull::Up),
            embassy_rp::gpio::Input::new(p.PIN_5, embassy_rp::gpio::Pull::Up),
            embassy_rp::gpio::Input::new(p.PIN_6, embassy_rp::gpio::Pull::Up),
            embassy_rp::gpio::Input::new(p.PIN_10, embassy_rp::gpio::Pull::Up),
            embassy_rp::gpio::Input::new(p.PIN_11, embassy_rp::gpio::Pull::Up),
            embassy_rp::gpio::Input::new(p.PIN_12, embassy_rp::gpio::Pull::Up),
            embassy_rp::gpio::Input::new(p.PIN_13, embassy_rp::gpio::Pull::Up),
            embassy_rp::gpio::Input::new(p.PIN_16, embassy_rp::gpio::Pull::Up),
        )));
        // Spawn status LED task (using different pins to avoid conflicts)
        unwrap!(spawner.spawn(hardware::status_task(
            embassy_rp::gpio::Output::new(p.PIN_20, embassy_rp::gpio::Level::Low),
            embassy_rp::gpio::Output::new(p.PIN_21, embassy_rp::gpio::Level::Low)
        )));
    });

    // This should never be reached
    loop {
        cortex_m::asm::wfe();
    }
}

/// Core 0 main task: USB, buttons, and supervision
#[embassy_executor::task]
async fn core0_main_task(mut supervisor: supervisor::AppSupervisor) {
    info!("Core 0: Starting USB and button tasks");

    // Initialize and spawn core 0 tasks (USB, buttons)
    // Note: spawner is not available in this context, we'll use the existing channel system
    info!("Core 0: Stream Deck Module 32 firmware initialized successfully");
    supervisor.print_init_success();

    // Run the main supervisor loop
    supervisor.run().await;
}

/// Core 1 task: Image processing and display
#[embassy_executor::task]
async fn core1_image_processing_task() {
    info!("Core 1: Starting image processing and display tasks");

    // Initialize and spawn core 1 tasks (display, image processing)
    match hardware::init_hardware_tasks_core1(DEVICE).await {
        Ok(()) => {
            info!("Core 1: Image processing tasks initialized successfully");
        }
        Err(e) => {
            error!("Core 1: Failed to spawn image processing tasks: {:?}", e);
            core::panic!("Image processing initialization failed");
        }
    }

    // Optimized image processing buffer for Module 32 (96x96 JPEG)
    let mut image_processing_buffer = [0u8; 16384]; // 16KB buffer for larger images

    // Process display commands from core 0
    let receiver = IMAGE_CHANNEL.receiver();
    loop {
        match receiver.receive().await {
            productiondeck::types::DisplayCommand::DisplayImage { key_id, data } => {
                info!(
                    "Core 1: Processing image for key {} ({} bytes)",
                    key_id,
                    data.len()
                );

                // Optimized image processing with larger buffer
                if data.len() <= image_processing_buffer.len() {
                    // Copy data to processing buffer for faster access
                    let copy_len = data.len().min(image_processing_buffer.len());
                    image_processing_buffer[..copy_len].copy_from_slice(&data[..copy_len]);

                    // TODO: Implement actual image processing and display for Module 32
                    // Process image from buffer for better performance
                    // Note: Module 32 uses 96x96 JPEG images that need 180° rotation
                } else {
                    warn!(
                        "Core 1: Image too large for buffer ({} > {} bytes)",
                        data.len(),
                        image_processing_buffer.len()
                    );
                }
            }
            productiondeck::types::DisplayCommand::SetBrightness(brightness) => {
                info!("Core 1: Setting brightness to {}%", brightness);
                // TODO: Implement brightness control
            }
            productiondeck::types::DisplayCommand::ClearAll => {
                info!("Core 1: Clearing all displays");
                // TODO: Implement display clear
            }
            productiondeck::types::DisplayCommand::Clear(key_id) => {
                info!("Core 1: Clearing display for key {}", key_id);
                // TODO: Implement single key clear
            }
        }
    }
}
