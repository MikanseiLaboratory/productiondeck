//! Application supervisor and monitoring
//!
//! This module provides application-level supervision, monitoring,
//! and lifecycle management functionality.

use crate::config;
use crate::device::{Device, DeviceConfig};
use crate::types::APP_VERSION;
use defmt::*;
use embassy_time::{Duration, Timer};

/// Application supervisor responsible for monitoring and lifecycle management
pub struct AppSupervisor {
    device: Device,
    uptime_seconds: u32,
    last_heartbeat: u32,
}

impl AppSupervisor {
    /// Create a new application supervisor
    pub fn new() -> Self {
        Self::new_for_device(config::get_current_device())
    }

    /// Create a new application supervisor for a specific device
    pub fn new_for_device(device: Device) -> Self {
        Self {
            device,
            uptime_seconds: 0,
            last_heartbeat: 0,
        }
    }

    /// Print application startup banner with device information
    pub fn print_startup_banner(&self) {
        let device = self.device;
        let usb_config = device.usb_config();
        let layout = device.button_layout();
        let display = device.display_config();

        info!("========================================");
        info!("ProductionDeck v{}", APP_VERSION.as_string());
        info!("Open Source StreamDeck Alternative");
        info!("========================================");
        info!("Hardware: RP2040 (Raspberry Pi Pico)");
        info!("Target: {} Compatible", device.device_name());
        info!(
            "USB: VID=0x{:04X} PID=0x{:04X}",
            usb_config.vid, usb_config.pid
        );
        info!("Protocol: {:?}", usb_config.protocol);
        info!(
            "Keys: {} ({}x{} layout)",
            layout.total_keys, layout.cols, layout.rows
        );
        info!(
            "Display: {}x{} per key",
            display.image_width, display.image_height
        );
        info!("========================================");
    }

    /// Print successful initialization message
    pub fn print_init_success(&self) {
        let usb_config = self.device.usb_config();
        info!("ProductionDeck initialized successfully");
        info!(
            "USB VID:PID = {:04X}:{:04X}",
            usb_config.vid, usb_config.pid
        );
        info!("Waiting for USB connection...");
    }

    /// Run the main supervisor loop
    pub async fn run(&mut self) {
        info!("Application supervisor started");

        loop {
            // Wait for 10 seconds
            Timer::after(Duration::from_secs(10)).await;
            self.uptime_seconds += 10;

            // Print status every 60 seconds (6 iterations)
            if self.uptime_seconds - self.last_heartbeat >= 60 {
                self.print_status();
                self.last_heartbeat = self.uptime_seconds;
            }
        }
    }

    /// Print current application status
    fn print_status(&self) {
        let minutes = self.uptime_seconds / 60;
        let hours = minutes / 60;
        let remaining_minutes = minutes % 60;

        if hours > 0 {
            info!("Status: Uptime {}h{}m", hours, remaining_minutes);
        } else {
            info!("Status: Uptime {}m", minutes);
        }

        // TODO: Add memory usage, task health, etc.
    }

    /// Get current uptime in seconds
    pub fn uptime(&self) -> u32 {
        self.uptime_seconds
    }
}

impl Default for AppSupervisor {
    fn default() -> Self {
        Self::new()
    }
}
