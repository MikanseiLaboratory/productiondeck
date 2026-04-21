//! Device abstraction layer for StreamDeck compatible devices
//!
//! This module provides a unified interface for different StreamDeck models,
//! abstracting away device-specific configurations, protocols, and capabilities.
//!
//! Protocol families (Elgato HID API):
//! - **Legacy / Mini family**: Mini, Mini 2022, Mini Discord, 6-key Module — distinct report layout.
//! - **Main / Expanded family**: Classic, XL, Neo, Plus, Plus XL, 15/32-key Modules — see General Reference.

pub mod mini;
pub mod neo;
pub mod original;
pub mod original_v2;
pub mod plus;
pub mod plus_xl;
pub mod xl;

/// Image format supported by StreamDeck devices
#[derive(Debug, Clone, Copy, PartialEq, defmt::Format)]
pub enum ImageFormat {
    /// BMP format (used by V1 protocol devices)
    Bmp,
    /// JPEG format (used by V2 protocol devices)
    Jpeg,
}

/// Protocol version used by StreamDeck devices
#[derive(Debug, Clone, Copy, PartialEq, defmt::Format)]
pub enum ProtocolVersion {
    /// Legacy Mini-family protocol (BMP, distinct feature IDs)
    V1,
    /// Main / Expanded family protocol (JPEG chunks, feature report ID 0x03, …)
    V2,
    /// 6-key Module uses Mini-family legacy command set per Elgato Mini module page
    Module6Keys,
}

/// Button layout configuration
#[derive(Debug, Clone, Copy)]
pub struct ButtonLayout {
    /// Number of button columns
    pub cols: usize,
    /// Number of button rows
    pub rows: usize,
    /// Total number of physical keys (cols * rows)
    pub total_keys: usize,
    /// Button mapping order (true = left-to-right, false = right-to-left)
    pub left_to_right: bool,
}

impl ButtonLayout {
    pub const fn new(cols: usize, rows: usize, left_to_right: bool) -> Self {
        Self {
            cols,
            rows,
            total_keys: cols * rows,
            left_to_right,
        }
    }
}

/// Display configuration for StreamDeck devices
#[derive(Debug, Clone, Copy)]
pub struct DisplayConfig {
    /// Image width in pixels per key
    pub image_width: usize,
    /// Image height in pixels per key
    pub image_height: usize,
    /// Image format (BMP or JPEG)
    pub format: ImageFormat,
    /// Whether image needs rotation (Mini: host rotates 90° CW; device applies inverse)
    pub needs_rotation: bool,
    /// Whether image needs horizontal flip
    pub flip_horizontal: bool,
    /// Whether image needs vertical flip
    pub flip_vertical: bool,
}

/// USB configuration for StreamDeck devices
#[derive(Debug, Clone, Copy)]
pub struct UsbConfig {
    /// USB Vendor ID (always 0x0fd9 for Elgato)
    pub vid: u16,
    /// USB Product ID (device-specific)
    pub pid: u16,
    /// USB product name
    pub product_name: &'static str,
    /// USB manufacturer name
    pub manufacturer: &'static str,
    /// Protocol version
    pub protocol: ProtocolVersion,
}

/// Complete device configuration trait
pub trait DeviceConfig {
    fn device_name(&self) -> &'static str;
    fn button_layout(&self) -> ButtonLayout;
    fn display_config(&self) -> DisplayConfig;
    fn usb_config(&self) -> UsbConfig;

    /// Logical keys reported in Main Protocol input (includes Neo sensor slots)
    fn protocol_input_key_count(&self) -> usize {
        self.button_layout().total_keys
    }

    fn max_image_size(&self) -> usize {
        let display = self.display_config();
        match display.format {
            ImageFormat::Bmp => 54 + (display.image_width * display.image_height * 3),
            ImageFormat::Jpeg => display.image_width * display.image_height / 2,
        }
    }

    fn hid_descriptor_size(&self) -> usize {
        173
    }

    fn input_report_size(&self) -> usize {
        match self.usb_config().protocol {
            ProtocolVersion::V1 => self.button_layout().total_keys + 1,
            ProtocolVersion::V2 => self.protocol_input_key_count() + 4, // RID + cmd + len16 + payload
            ProtocolVersion::Module6Keys => 65,
        }
    }

    fn feature_report_size(&self) -> usize {
        32
    }

    fn output_report_size(&self) -> usize {
        1024
    }
}

/// Enum-based device configuration for no_std environment
///
/// Discriminants are stored by [`crate::config::init_runtime_device`]; do not reorder without
/// updating `from_runtime_tag`.
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Device {
    Mini = 0,
    /// Mini 2022 (Elgato PID 0x0090)
    RevisedMini = 1,
    /// Mini Discord (0x00B3)
    MiniDiscord = 2,
    /// First-gen Stream Deck (0x0060)
    Original = 3,
    /// Stream Deck 2019 (0x006D)
    OriginalV2 = 4,
    /// Stream Deck Mk.2 (0x0080)
    Mk2 = 5,
    /// Mk.2 scissor keys (0x00A5)
    Mk2ScissorKeys = 6,
    Xl = 7,
    /// Stream Deck XL 2022 (0x008F)
    Xl2022 = 8,
    /// Stream Deck + (0x0084)
    Plus = 9,
    /// Stream Deck + XL — same USB PID as + per Elgato HID summary table; layout differs (9×4).
    PlusXl = 10,
    Neo = 11,
    Module6Keys = 12,
    Module15Keys = 13,
    Module32Keys = 14,
}

/// Tag value before [`crate::config::init_runtime_device`] runs.
pub const RUNTIME_DEVICE_TAG_UNINIT: u8 = 0xFF;

impl Device {
    /// Decode a tag written by [`crate::config::init_runtime_device`].
    pub fn from_runtime_tag(tag: u8) -> Option<Self> {
        if tag == RUNTIME_DEVICE_TAG_UNINIT {
            return None;
        }
        match tag {
            0 => Some(Device::Mini),
            1 => Some(Device::RevisedMini),
            2 => Some(Device::MiniDiscord),
            3 => Some(Device::Original),
            4 => Some(Device::OriginalV2),
            5 => Some(Device::Mk2),
            6 => Some(Device::Mk2ScissorKeys),
            7 => Some(Device::Xl),
            8 => Some(Device::Xl2022),
            9 => Some(Device::Plus),
            10 => Some(Device::PlusXl),
            11 => Some(Device::Neo),
            12 => Some(Device::Module6Keys),
            13 => Some(Device::Module15Keys),
            14 => Some(Device::Module32Keys),
            _ => None,
        }
    }

    pub fn pid(&self) -> u16 {
        match self {
            Device::Mini => 0x0063,
            Device::RevisedMini => 0x0090,
            Device::MiniDiscord => 0x00B3,
            Device::Original => 0x0060,
            Device::OriginalV2 => 0x006d,
            Device::Mk2 => 0x0080,
            Device::Mk2ScissorKeys => 0x00A5,
            Device::Xl => 0x006c,
            Device::Xl2022 => 0x008F,
            Device::Plus | Device::PlusXl => 0x0084,
            Device::Neo => 0x009A,
            Device::Module6Keys => 0x00B8,
            Device::Module15Keys => 0x00B9,
            Device::Module32Keys => 0x00BA,
        }
    }

    /// Main Protocol GET Unit Information (feature report 0x08) payload bytes [1..=16] after report ID.
    pub fn unit_information_tail(&self) -> [u8; 16] {
        let (rows, cols, kw, kh, lcd_w, lcd_h) = match self {
            Device::OriginalV2 | Device::Mk2 | Device::Mk2ScissorKeys | Device::Module15Keys => {
                (3u8, 5u8, 72u16, 72u16, 480u16, 272u16)
            }
            Device::Xl | Device::Xl2022 | Device::Module32Keys => {
                (4u8, 8u8, 96u16, 96u16, 1024u16, 600u16)
            }
            Device::Plus => (2u8, 4u8, 120u16, 120u16, 800u16, 480u16),
            Device::PlusXl => (4u8, 9u8, 112u16, 112u16, 1280u16, 800u16),
            Device::Neo => (2u8, 4u8, 96u16, 96u16, 480u16, 320u16),
            Device::Mini | Device::RevisedMini | Device::MiniDiscord | Device::Module6Keys => {
                (2u8, 3u8, 80u16, 80u16, 320u16, 240u16)
            }
            Device::Original => (3u8, 5u8, 72u16, 72u16, 480u16, 272u16),
        };
        let mut b = [0u8; 16];
        b[0] = rows;
        b[1] = cols;
        let kw_e = kw.to_le_bytes();
        let kh_e = kh.to_le_bytes();
        let lw_e = lcd_w.to_le_bytes();
        let lh_e = lcd_h.to_le_bytes();
        b[2] = kw_e[0];
        b[3] = kw_e[1];
        b[4] = kh_e[0];
        b[5] = kh_e[1];
        b[6] = lw_e[0];
        b[7] = lw_e[1];
        b[8] = lh_e[0];
        b[9] = lh_e[1];
        b[10] = 24;
        b[11] = 0x00;
        b[12] = 0;
        b[13] = 0;
        b[14] = 0;
        b[15] = 0;
        b
    }

    pub fn supports_background_feature(&self) -> bool {
        matches!(
            self,
            Device::OriginalV2
                | Device::Mk2
                | Device::Mk2ScissorKeys
                | Device::Xl
                | Device::Xl2022
                | Device::Module15Keys
                | Device::Module32Keys
        )
    }

    pub fn supports_window_image_commands(&self) -> bool {
        matches!(self, Device::Neo | Device::Plus | Device::PlusXl)
    }
}

impl DeviceConfig for Device {
    fn device_name(&self) -> &'static str {
        match self {
            Device::Mini => "StreamDeck Mini",
            Device::RevisedMini => "StreamDeck Mini 2022",
            Device::MiniDiscord => "StreamDeck Mini Discord",
            Device::Original => "StreamDeck Original",
            Device::OriginalV2 => "StreamDeck Classic (2019)",
            Device::Mk2 => "StreamDeck Mk.2",
            Device::Mk2ScissorKeys => "StreamDeck Mk.2 (Scissor)",
            Device::Xl => "StreamDeck XL",
            Device::Xl2022 => "StreamDeck XL 2022",
            Device::Plus => "StreamDeck +",
            Device::PlusXl => "StreamDeck + XL",
            Device::Neo => "StreamDeck Neo",
            Device::Module6Keys => "StreamDeck Module 6 Keys",
            Device::Module15Keys => "StreamDeck Module 15 Keys",
            Device::Module32Keys => "StreamDeck Module 32 Keys",
        }
    }

    fn protocol_input_key_count(&self) -> usize {
        match self {
            Device::Neo => 10,
            _ => self.button_layout().total_keys,
        }
    }

    fn button_layout(&self) -> ButtonLayout {
        match self {
            Device::Mini | Device::RevisedMini | Device::MiniDiscord | Device::Module6Keys => {
                ButtonLayout::new(3, 2, true)
            }
            Device::Module15Keys | Device::OriginalV2 | Device::Mk2 | Device::Mk2ScissorKeys => {
                ButtonLayout::new(5, 3, true)
            }
            Device::Original => ButtonLayout::new(5, 3, false),
            Device::Xl | Device::Xl2022 | Device::Module32Keys => ButtonLayout::new(8, 4, true),
            Device::Plus | Device::Neo => ButtonLayout::new(4, 2, true),
            Device::PlusXl => ButtonLayout::new(9, 4, true),
        }
    }

    fn display_config(&self) -> DisplayConfig {
        match self {
            Device::Mini | Device::RevisedMini | Device::MiniDiscord | Device::Module6Keys => {
                DisplayConfig {
                    image_width: 80,
                    image_height: 80,
                    format: ImageFormat::Bmp,
                    needs_rotation: true,
                    flip_horizontal: false,
                    flip_vertical: false,
                }
            }
            Device::Module15Keys | Device::OriginalV2 | Device::Mk2 | Device::Mk2ScissorKeys => {
                DisplayConfig {
                    image_width: 72,
                    image_height: 72,
                    format: ImageFormat::Jpeg,
                    needs_rotation: false,
                    flip_horizontal: true,
                    flip_vertical: true,
                }
            }
            Device::Original => DisplayConfig {
                image_width: 72,
                image_height: 72,
                format: ImageFormat::Bmp,
                needs_rotation: false,
                flip_horizontal: true,
                flip_vertical: false,
            },
            Device::Xl | Device::Xl2022 | Device::Module32Keys => DisplayConfig {
                image_width: 96,
                image_height: 96,
                format: ImageFormat::Jpeg,
                needs_rotation: false,
                flip_horizontal: true,
                flip_vertical: true,
            },
            Device::Neo => DisplayConfig {
                image_width: 96,
                image_height: 96,
                format: ImageFormat::Jpeg,
                needs_rotation: false,
                flip_horizontal: true,
                flip_vertical: true,
            },
            Device::Plus => DisplayConfig {
                image_width: 120,
                image_height: 120,
                format: ImageFormat::Jpeg,
                needs_rotation: false,
                flip_horizontal: false,
                flip_vertical: false,
            },
            Device::PlusXl => DisplayConfig {
                image_width: 112,
                image_height: 112,
                format: ImageFormat::Jpeg,
                needs_rotation: false,
                flip_horizontal: true,
                flip_vertical: true,
            },
        }
    }

    fn usb_config(&self) -> UsbConfig {
        match self {
            Device::Mini => UsbConfig {
                vid: 0x0fd9,
                pid: 0x0063,
                product_name: "Stream Deck Mini",
                manufacturer: "Elgato Systems",
                protocol: ProtocolVersion::V1,
            },
            Device::RevisedMini => UsbConfig {
                vid: 0x0fd9,
                pid: 0x0090,
                product_name: "Stream Deck Mini",
                manufacturer: "Elgato Systems",
                protocol: ProtocolVersion::V1,
            },
            Device::MiniDiscord => UsbConfig {
                vid: 0x0fd9,
                pid: 0x00B3,
                product_name: "Stream Deck Mini",
                manufacturer: "Elgato Systems",
                protocol: ProtocolVersion::V1,
            },
            Device::Original => UsbConfig {
                vid: 0x0fd9,
                pid: 0x0060,
                product_name: "Stream Deck",
                manufacturer: "Elgato Systems",
                protocol: ProtocolVersion::V1,
            },
            Device::OriginalV2 => UsbConfig {
                vid: 0x0fd9,
                pid: 0x006d,
                product_name: "Stream Deck",
                manufacturer: "Elgato Systems",
                protocol: ProtocolVersion::V2,
            },
            Device::Mk2 => UsbConfig {
                vid: 0x0fd9,
                pid: 0x0080,
                product_name: "Stream Deck",
                manufacturer: "Elgato Systems",
                protocol: ProtocolVersion::V2,
            },
            Device::Mk2ScissorKeys => UsbConfig {
                vid: 0x0fd9,
                pid: 0x00A5,
                product_name: "Stream Deck",
                manufacturer: "Elgato Systems",
                protocol: ProtocolVersion::V2,
            },
            Device::Xl => UsbConfig {
                vid: 0x0fd9,
                pid: 0x006c,
                product_name: "Stream Deck XL",
                manufacturer: "Elgato Systems",
                protocol: ProtocolVersion::V2,
            },
            Device::Xl2022 => UsbConfig {
                vid: 0x0fd9,
                pid: 0x008F,
                product_name: "Stream Deck XL",
                manufacturer: "Elgato Systems",
                protocol: ProtocolVersion::V2,
            },
            Device::Plus => UsbConfig {
                vid: 0x0fd9,
                pid: 0x0084,
                product_name: "Stream Deck +",
                manufacturer: "Elgato Systems",
                protocol: ProtocolVersion::V2,
            },
            Device::PlusXl => UsbConfig {
                vid: 0x0fd9,
                pid: 0x0084,
                product_name: "Stream Deck + XL",
                manufacturer: "Elgato Systems",
                protocol: ProtocolVersion::V2,
            },
            Device::Neo => UsbConfig {
                vid: 0x0fd9,
                pid: 0x009A,
                product_name: "Stream Deck Neo",
                manufacturer: "Elgato Systems",
                protocol: ProtocolVersion::V2,
            },
            Device::Module6Keys => UsbConfig {
                vid: 0x0fd9,
                pid: 0x00B8,
                product_name: "Stream Deck Module 6 Keys",
                manufacturer: "Elgato Systems",
                protocol: ProtocolVersion::Module6Keys,
            },
            Device::Module15Keys => UsbConfig {
                vid: 0x0fd9,
                pid: 0x00B9,
                product_name: "Stream Deck Module 15 Keys",
                manufacturer: "Elgato Systems",
                protocol: ProtocolVersion::V2,
            },
            Device::Module32Keys => UsbConfig {
                vid: 0x0fd9,
                pid: 0x00BA,
                product_name: "Stream Deck Module 32 Keys",
                manufacturer: "Elgato Systems",
                protocol: ProtocolVersion::V2,
            },
        }
    }
}
