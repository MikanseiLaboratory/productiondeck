//! StreamDeck XL device configuration
//!
//! The large StreamDeck with 32 keys (PID: 0x006c)

use super::{ButtonLayout, DeviceConfig, DisplayConfig, ImageFormat, ProtocolVersion, UsbConfig};

/// StreamDeck XL configuration (PID: 0x006c)
pub struct XlConfig;

impl DeviceConfig for XlConfig {
    fn device_name(&self) -> &'static str {
        "StreamDeck XL"
    }

    fn button_layout(&self) -> ButtonLayout {
        ButtonLayout::new(8, 4, true) // 8x4 layout, left-to-right
    }

    fn display_config(&self) -> DisplayConfig {
        DisplayConfig {
            image_width: 96,
            image_height: 96,
            format: ImageFormat::Jpeg,
            needs_rotation: false,
            flip_horizontal: true, // XL needs both horizontal and vertical flip
            flip_vertical: true,
        }
    }

    fn usb_config(&self) -> UsbConfig {
        UsbConfig {
            vid: 0x0fd9,
            pid: 0x006c,
            product_name: "Stream Deck XL",
            manufacturer: "Elgato Systems",
            protocol: ProtocolVersion::V2,
        }
    }
}
