//! Common types and data structures used across the ProductionDeck application
//!
//! This module contains shared types, enums, and structures that are used
//! by multiple modules in the application.

use crate::config::IMAGE_BUFFER_SIZE;
use heapless::Vec;

/// Maximum logical keys / protocol slots (Stream Deck + XL has 36 keys)
pub const MAX_BUTTON_SLOTS: usize = 48;

/// Button state structure for communicating button presses between tasks
#[derive(Clone, Copy, Debug, defmt::Format)]
pub struct ButtonState {
    /// Array of button states - true if pressed, false if released
    /// Using fixed size for compatibility across all devices
    pub buttons: [bool; MAX_BUTTON_SLOTS],
    /// Whether any button state has changed since last report
    pub changed: bool,
    /// Number of active buttons for this device
    pub active_count: usize,
}

impl ButtonState {
    /// Create new button state with all buttons released
    pub fn new(active_count: usize) -> Self {
        Self {
            buttons: [false; MAX_BUTTON_SLOTS],
            changed: false,
            active_count: active_count.min(MAX_BUTTON_SLOTS),
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

/// Touch activity on Stream Deck + / + XL window (host protocol; reserved for hardware)
#[derive(Clone, Copy, Debug, defmt::Format)]
pub enum TouchActivityKind {
    Tap,
    Press,
    Flick,
}

#[derive(Clone, Copy, Debug, defmt::Format)]
pub struct TouchActivity {
    pub kind: TouchActivityKind,
    pub x: u16,
    pub y: u16,
    pub x2: u16,
    pub y2: u16,
}

/// Encoder activity (Stream Deck + / + XL; reserved for hardware)
#[derive(Clone, Copy, Debug, defmt::Format)]
pub enum EncoderActivityKind {
    Button,
    Rotate,
}

#[derive(Clone, Copy, Debug, defmt::Format)]
pub struct EncoderActivity {
    pub kind: EncoderActivityKind,
    pub states: [i8; 8],
    pub count: usize,
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
    /// Full-screen JPEG assembled (reserved for display pipeline)
    FullScreenImage {
        #[allow(clippy::large_enum_variant)]
        data: Vec<u8, IMAGE_BUFFER_SIZE>,
    },
    /// Window strip JPEG (Neo / + / + XL)
    WindowImage {
        #[allow(clippy::large_enum_variant)]
        data: Vec<u8, IMAGE_BUFFER_SIZE>,
    },
    /// Partial window update (reserved)
    PartialWindowImage {
        x: u16,
        y: u16,
        width: u16,
        height: u16,
        #[allow(clippy::large_enum_variant)]
        data: Vec<u8, IMAGE_BUFFER_SIZE>,
    },
    /// Background slot image (Classic / XL)
    BackgroundImage {
        index: u8,
        #[allow(clippy::large_enum_variant)]
        data: Vec<u8, IMAGE_BUFFER_SIZE>,
    },
    /// Fill entire LCD with RGB (feature report)
    FillLcdColor {
        r: u8,
        g: u8,
        b: u8,
    },
    /// Fill one key with RGB (feature report)
    FillKeyColor {
        key_index: u8,
        r: u8,
        g: u8,
        b: u8,
    },
    /// Show stored background by index (XL family feature 0x03 / 0x13)
    ShowBackgroundByIndex {
        index: u8,
    },
    /// Touch / encoder events for future wiring (no-op in USB task until implemented)
    TouchActivity(TouchActivity),
    EncoderActivity(EncoderActivity),
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
    /// Full LCD image (JPEG assembled)
    DisplayFullScreen {
        #[allow(clippy::large_enum_variant)]
        data: Vec<u8, IMAGE_BUFFER_SIZE>,
    },
    /// Window / info strip image
    DisplayWindow {
        #[allow(clippy::large_enum_variant)]
        data: Vec<u8, IMAGE_BUFFER_SIZE>,
    },
    FillLcd {
        r: u8,
        g: u8,
        b: u8,
    },
    FillKey {
        key_index: u8,
        r: u8,
        g: u8,
        b: u8,
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
