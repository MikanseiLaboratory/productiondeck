//! StreamDeck Original device configuration
//!
//! The original StreamDeck with 15 keys (PID: 0x0060)

use super::{ButtonLayout, DeviceConfig, DisplayConfig, ImageFormat, ProtocolVersion, UsbConfig};

/// StreamDeck Original configuration (PID: 0x0060)
pub struct OriginalConfig;

impl DeviceConfig for OriginalConfig {
    fn device_name(&self) -> &'static str {
        "StreamDeck Original"
    }

    fn button_layout(&self) -> ButtonLayout {
        ButtonLayout::new(5, 3, false) // 5x3 layout, right-to-left mapping
    }

    fn display_config(&self) -> DisplayConfig {
        DisplayConfig {
            image_width: 72,
            image_height: 72,
            format: ImageFormat::Bmp,
            needs_rotation: false,
            flip_horizontal: true, // Original needs horizontal flip
            flip_vertical: false,
        }
    }

    fn usb_config(&self) -> UsbConfig {
        UsbConfig {
            vid: 0x0fd9,
            pid: 0x0060,
            product_name: "Stream Deck",
            manufacturer: "Elgato Systems",
            protocol: ProtocolVersion::V1,
        }
    }
}
