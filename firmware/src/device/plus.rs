//! StreamDeck Plus device configuration
//!
//! The StreamDeck Plus with 8 keys and additional controls (PID: 0x0080)

use super::{ButtonLayout, DeviceConfig, DisplayConfig, ImageFormat, ProtocolVersion, UsbConfig};

/// StreamDeck Plus configuration (PID: 0x0080)
pub struct PlusConfig;

impl DeviceConfig for PlusConfig {
    fn device_name(&self) -> &'static str {
        "StreamDeck Plus"
    }

    fn button_layout(&self) -> ButtonLayout {
        ButtonLayout::new(4, 2, true) // 4x2 layout, left-to-right
    }

    fn display_config(&self) -> DisplayConfig {
        DisplayConfig {
            image_width: 120,
            image_height: 120,
            format: ImageFormat::Jpeg,
            needs_rotation: false,
            flip_horizontal: false, // Plus needs no transformation
            flip_vertical: false,
        }
    }

    fn usb_config(&self) -> UsbConfig {
        UsbConfig {
            vid: 0x0fd9,
            pid: 0x0084,
            product_name: "Stream Deck Plus",
            manufacturer: "Elgato Systems",
            protocol: ProtocolVersion::V2,
        }
    }
}
