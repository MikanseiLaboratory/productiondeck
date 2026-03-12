//! Common types and data structures used across the ProductionDeck application
//!
//! This module contains shared types, enums, and structures that are used
//! by multiple modules in the application.

use crate::config::IMAGE_BUFFER_SIZE;
use heapless::Vec;

/// Button state structure for communicating button presses between tasks
#[derive(Clone, Copy, Debug, defmt::Format)]
pub struct ButtonState {
    /// Array of button states - true if pressed, false if released
    /// Using fixed size for compatibility across all devices
    pub buttons: [bool; 32], // Max buttons for any StreamDeck device (XL has 32)
    /// Whether any button state has changed since last report
    pub changed: bool,
    /// Number of active buttons for this device
    pub active_count: usize,
}

impl ButtonState {
    /// Create new button state with all buttons released
    pub fn new(active_count: usize) -> Self {
        Self {
            buttons: [false; 32],
            changed: false,
            active_count: active_count.min(32),
        }
    }

    /// Check if a specific button is pressed
    pub fn is_pressed(&self, button_index: usize) -> bool {
        if button_index < self.active_count {
            self.buttons[button_index]
        } else {
            false
        }
    }

    /// Set button state and mark as changed if different
    pub fn set_button(&mut self, button_index: usize, pressed: bool) {
        if button_index < self.active_count && self.buttons[button_index] != pressed {
            self.buttons[button_index] = pressed;
            self.changed = true;
        }
    }
}

/// USB commands that can be sent from the HID handler to other tasks
#[derive(Clone, Debug)]
#[allow(clippy::large_enum_variant)]
pub enum UsbCommand {
    /// Reset device to initial state
    Reset,
    /// Set display brightness (0-100%)
    SetBrightness(u8),
    /// Image data received for a specific key
    ImageData {
        key_id: u8,
        #[allow(clippy::large_enum_variant)]
        data: Vec<u8, IMAGE_BUFFER_SIZE>,
    },
}

/// Display commands for controlling the display subsystem
#[derive(Clone, Debug)]
#[allow(clippy::large_enum_variant)]
pub enum DisplayCommand {
    /// Clear a specific key display
    Clear(u8),
    /// Clear all key displays
    ClearAll,
    /// Set display brightness (0-100%)
    SetBrightness(u8),
    /// Display an image on a specific key
    DisplayImage {
        key_id: u8,
        #[allow(clippy::large_enum_variant)]
        data: Vec<u8, IMAGE_BUFFER_SIZE>,
    },
}

/// Application version information
pub struct AppVersion {
    pub major: u8,
    pub minor: u8,
    pub patch: u8,
}

impl AppVersion {
    pub const fn new(major: u8, minor: u8, patch: u8) -> Self {
        Self {
            major,
            minor,
            patch,
        }
    }

    pub fn as_string(&self) -> &'static str {
        // For embedded, use a simple compile-time string
        "0.1.0"
    }
}

/// Current application version
pub const APP_VERSION: AppVersion = AppVersion::new(0, 1, 0);
