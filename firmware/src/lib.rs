//! ProductionDeck - Open Source StreamDeck Alternative for RP2040
//!
//! This library provides multi-device StreamDeck compatible firmware
//! using Embassy async framework on RP2040.
//!
//! ## Supported Devices
//! - StreamDeck Mini (6 keys, 80x80px)
//! - StreamDeck Original (15 keys, 72x72px)
//! - StreamDeck Original V2 (15 keys, 72x72px, JPEG)
//! - StreamDeck XL (32 keys, 96x96px, JPEG)
//! - StreamDeck Plus (8 keys, 120x120px, JPEG)
//!
//! ## Architecture
//! - **Multi-core**: USB/Protocol on Core 0, Display/Buttons on Core 1
//! - **Async**: Embassy framework with async/await
//! - **Channels**: Lock-free inter-task communication
//! - **Device Abstraction**: Compile-time device selection and configuration

#![no_std]

use embassy_rp::usb::InterruptHandler;
use embassy_rp::{bind_interrupts, peripherals};

// Export all modules for use by device-specific binaries
pub mod buttons;
pub mod channels;
pub mod config;
pub mod device;
pub mod display;
pub mod hardware;
pub mod protocol;
pub mod supervisor;
pub mod types;
pub mod usb;

// USB interrupt binding - shared by all binaries
bind_interrupts!(pub struct Irqs {
    USBCTRL_IRQ => InterruptHandler<peripherals::USB>;
});
