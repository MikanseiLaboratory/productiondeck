//! Hardware configuration for ProductionDeck
//! RP2040-based StreamDeck compatible device with multi-device support

use crate::device::{Device, DeviceConfig, RUNTIME_DEVICE_TAG_UNINIT};
use core::sync::atomic::{AtomicI32, AtomicU8, Ordering};

// ===================================================================
// Device Selection Configuration
// ===================================================================

static RUNTIME_DEVICE_TAG: AtomicU8 = AtomicU8::new(RUNTIME_DEVICE_TAG_UNINIT);

/// Store the firmware build device (call once from each binary `main` before other tasks).
pub fn init_runtime_device(device: Device) {
    RUNTIME_DEVICE_TAG.store(device as u8, Ordering::Relaxed);
}

/// Active device for [`streamdeck_keys`], USB strings, and display sizing.
pub fn get_current_device() -> Device {
    let tag = RUNTIME_DEVICE_TAG.load(Ordering::Relaxed);
    Device::from_runtime_tag(tag).expect("init_runtime_device must run before get_current_device")
}

// ===================================================================
// Button Input Mode Configuration
// ===================================================================

/// Button input mode selector
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ButtonInputMode {
    /// Traditional key matrix scanning (uses fewer GPIOs)
    Matrix = 0,
    /// Direct pin reading (one GPIO per key)
    Direct = 1,
}

/// Current button input mode (defaults to Matrix)
static BUTTON_INPUT_MODE: AtomicU8 = AtomicU8::new(ButtonInputMode::Matrix as u8);

/// Set the current button input mode
pub fn set_button_input_mode(mode: ButtonInputMode) {
    BUTTON_INPUT_MODE.store(mode as u8, Ordering::Relaxed);
}

/// Get the current button input mode
pub fn button_input_mode() -> ButtonInputMode {
    match BUTTON_INPUT_MODE.load(Ordering::Relaxed) {
        1 => ButtonInputMode::Direct,
        _ => ButtonInputMode::Matrix,
    }
}

// ===================================================================
// USB Configuration - Dynamic based on current device
// ===================================================================

/// Serial number (static for all devices)
pub const USB_SERIAL: &str = "PRODUCTIONDK"; // 12 chars

/// USB version settings
pub const USB_BCD_DEVICE: u16 = 0x0200; // Device version 2.0

// ===================================================================
// Device Specifications - Dynamic based on current device
// ===================================================================

pub fn streamdeck_keys() -> usize {
    get_current_device().button_layout().total_keys
}

pub fn streamdeck_cols() -> usize {
    get_current_device().button_layout().cols
}

pub fn streamdeck_rows() -> usize {
    get_current_device().button_layout().rows
}

pub fn key_image_size() -> usize {
    let display = get_current_device().display_config();
    display.image_width // Assume square images
}

// ===================================================================
// GPIO Pin Assignments - Raspberry Pi Pico
// ===================================================================
//
// Button matrix row/column BCM numbers live in [`crate::hardware::HardwareConfig::for_device`]
// together with [`crate::hardware::create_all_pins_for_device`] (single source of truth).

// SPI Display Interface
pub const SPI_MOSI_PIN: u8 = 19; // Data to display
pub const SPI_SCK_PIN: u8 = 18; // Clock to display
pub const SPI_BAUDRATE: u32 = 10_000_000; // 10MHz SPI clock

// Single Display Control Pins
pub const DISPLAY_CS_PIN: u8 = 8; // Chip select
pub const DISPLAY_DC_PIN: u8 = 14; // Data/Command select
pub const DISPLAY_RST_PIN: u8 = 15; // Reset
pub const DISPLAY_BL_PIN: u8 = 17; // Backlight control (PWM)

// Status LEDs
pub const LED_STATUS_PIN: u8 = 25; // Built-in LED on Pico
pub const LED_USB_PIN: u8 = 20; // USB status LED
pub const LED_ERROR_PIN: u8 = 21; // Error indication LED

// ===================================================================
// Hardware Configuration Options
// ===================================================================

pub const BUTTON_DEBOUNCE_MS: u64 = 20; // Button debounce time
pub const BUTTON_SCAN_RATE_HZ: u64 = 100; // Button scan frequency

// Display configuration - Dynamic
pub fn display_brightness() -> u8 {
    255 // Default brightness (0-255)
}

pub fn display_total_width() -> usize {
    streamdeck_cols() * key_image_size()
}

pub fn display_total_height() -> usize {
    streamdeck_rows() * key_image_size()
}

// USB Configuration
pub const USB_POLL_RATE_MS: u64 = 1; // 1ms USB polling (1000Hz)
pub const IMAGE_BUFFER_SIZE: usize = 1024; // 1KB buffer size

// Image processing optimization
pub const IMAGE_PROCESSING_BUFFER_SIZE: usize = 8192; // 8KB for image processing
pub const DISPLAY_BUFFER_SIZE: usize = 2048; // 2KB for display operations
pub const MULTICORE_CHANNEL_SIZE: usize = 8; // Increased channel size for better throughput

// ===================================================================
// Power Management: Idle Time (Sleep Mode)
// ===================================================================

/// Idle time before entering Sleep Mode, in seconds. 0 disables sleep.
static IDLE_TIME_SECONDS: AtomicI32 = AtomicI32::new(0);

/// Set idle time before entering Sleep Mode (seconds). Use 0 to disable sleep.
pub fn set_idle_time_seconds(seconds: i32) {
    IDLE_TIME_SECONDS.store(seconds, Ordering::Relaxed);
}

/// Get idle time before entering Sleep Mode (seconds). 0 means disabled.
pub fn get_idle_time_seconds() -> i32 {
    IDLE_TIME_SECONDS.load(Ordering::Relaxed)
}

// ===================================================================
// USB HID Report IDs and Commands
// ===================================================================

// Report types
pub const OUTPUT_REPORT_IMAGE: u8 = 0x02;
pub const IMAGE_COMMAND_V2: u8 = 0x07;

// Feature report IDs and commands
pub const FEATURE_REPORT_VERSION_V1: u8 = 0x04;
pub const FEATURE_REPORT_VERSION_V2: u8 = 0x05;
pub const FEATURE_REPORT_SERIAL_NUMBER: u8 = 0x03;
pub const FEATURE_REPORT_FIRMWARE_INFO: u8 = 0xA1;
pub const FEATURE_REPORT_RESET_V1: u8 = 0x0B;
pub const FEATURE_REPORT_BRIGHTNESS_V1: u8 = 0x05;
pub const FEATURE_REPORT_V2_COMMANDS: u8 = 0x03; // V2 command container

// V2 sub-commands (used with FEATURE_REPORT_V2_COMMANDS)
pub const V2_COMMAND_RESET: u8 = 0x02;
pub const V2_COMMAND_BRIGHTNESS: u8 = 0x08;

// Idle time feature report constants
pub const FEATURE_REPORT_IDLE_TIME: u8 = 0x0B;
pub const IDLE_TIME_COMMAND: u8 = 0xA2;
pub const FEATURE_REPORT_GET_IDLE_TIME: u8 = 0xA3;

// StreamDeck protocol magic bytes
pub const STREAMDECK_MAGIC_1: u8 = 0x55;
pub const STREAMDECK_MAGIC_2: u8 = 0xAA;
pub const STREAMDECK_MAGIC_3: u8 = 0xD1;
pub const STREAMDECK_RESET_MAGIC: u8 = 0x63;
pub const STREAMDECK_BRIGHTNESS_RESET_MAGIC: u8 = 0x3E;

// ===================================================================
// ST7735 Display Commands
// ===================================================================

pub const ST7735_SWRESET: u8 = 0x01; // Software reset
pub const ST7735_SLPOUT: u8 = 0x11; // Sleep out
pub const ST7735_COLMOD: u8 = 0x3A; // Color mode
pub const ST7735_CASET: u8 = 0x2A; // Column address set
pub const ST7735_RASET: u8 = 0x2B; // Row address set
pub const ST7735_INVOFF: u8 = 0x20; // Display inversion off
pub const ST7735_NORON: u8 = 0x13; // Normal display mode
pub const ST7735_DISPON: u8 = 0x29; // Display on
pub const ST7735_RAMWR: u8 = 0x2C; // Memory write

// ST7735 Color format constants
pub const ST7735_COLOR_MODE_16BIT: u8 = 0x05; // RGB565 format

// RGB565 conversion masks
pub const RGB565_RED_MASK: u16 = 0xF8;
pub const RGB565_GREEN_MASK: u16 = 0xFC;
pub const RGB565_BLUE_SHIFT: u8 = 3;
