//! StreamDeck Original V2 device configuration
//!
//! The second generation original StreamDeck with 15 keys and JPEG support (PID: 0x006d)

use super::{ButtonLayout, DeviceConfig, DisplayConfig, ImageFormat, ProtocolVersion, UsbConfig};

/// StreamDeck Original V2 configuration (PID: 0x006d)
pub struct OriginalV2Config;

impl DeviceConfig for OriginalV2Config {
    fn device_name(&self) -> &'static str {
        "StreamDeck Original V2"
    }

    fn button_layout(&self) -> ButtonLayout {
        ButtonLayout::new(5, 3, true) // 5x3 layout, left-to-right
    }

    fn display_config(&self) -> DisplayConfig {
        DisplayConfig {
            image_width: 72,
            image_height: 72,
            format: ImageFormat::Jpeg,
            needs_rotation: false,
            flip_horizontal: true, // V2 needs both horizontal and vertical flip
            flip_vertical: true,
        }
    }

    fn usb_config(&self) -> UsbConfig {
        UsbConfig {
            vid: 0x0fd9,
            pid: 0x006d,
            product_name: "Stream Deck",
            manufacturer: "Elgato Systems",
            protocol: ProtocolVersion::V2,
        }
    }
}
