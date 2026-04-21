//! StreamDeck Mini device configurations
//!
//! Supports the original Mini (PID 0x0063) and Mini 2022 (PID 0x0090).

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

/// Stream Deck Mini 2022 configuration (PID: 0x0090)
pub struct RevisedMiniConfig;

impl DeviceConfig for RevisedMiniConfig {
    fn device_name(&self) -> &'static str {
        "StreamDeck Mini 2022"
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
