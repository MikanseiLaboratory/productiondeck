//! StreamDeck Mini device configurations
//!
//! Supports both the original Mini (PID 0x0063) and Revised Mini (PID 0x0090)

use super::{ButtonLayout, DeviceConfig, DisplayConfig, ImageFormat, ProtocolVersion, UsbConfig};

/// StreamDeck Mini configuration (PID: 0x0063)
pub struct MiniConfig;

impl DeviceConfig for MiniConfig {
    fn device_name(&self) -> &'static str {
        "StreamDeck Mini"
    }

    fn button_layout(&self) -> ButtonLayout {
        ButtonLayout::new(3, 2, true) // 3x2 layout, left-to-right
    }

    fn display_config(&self) -> DisplayConfig {
        DisplayConfig {
            image_width: 80,
            image_height: 80,
            format: ImageFormat::Bmp,
            needs_rotation: true, // Mini needs 270° rotation
            flip_horizontal: false,
            flip_vertical: false,
        }
    }

    fn usb_config(&self) -> UsbConfig {
        UsbConfig {
            vid: 0x0fd9,
            pid: 0x0063,
            product_name: "Stream Deck Mini",
            manufacturer: "Elgato Systems",
            protocol: ProtocolVersion::V1,
        }
    }
}

/// StreamDeck Revised Mini configuration (PID: 0x0090)
pub struct RevisedMiniConfig;

impl DeviceConfig for RevisedMiniConfig {
    fn device_name(&self) -> &'static str {
        "StreamDeck Revised Mini"
    }

    fn button_layout(&self) -> ButtonLayout {
        ButtonLayout::new(3, 2, true) // 3x2 layout, left-to-right
    }

    fn display_config(&self) -> DisplayConfig {
        DisplayConfig {
            image_width: 80,
            image_height: 80,
            format: ImageFormat::Bmp,
            needs_rotation: true, // Mini needs 270° rotation
            flip_horizontal: false,
            flip_vertical: false,
        }
    }

    fn usb_config(&self) -> UsbConfig {
        UsbConfig {
            vid: 0x0fd9,
            pid: 0x0090,
            product_name: "Stream Deck Mini",
            manufacturer: "Elgato Systems",
            protocol: ProtocolVersion::V1,
        }
    }
}
