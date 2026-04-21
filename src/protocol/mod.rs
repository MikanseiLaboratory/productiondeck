//! StreamDeck protocol abstraction layer
//!
//! Handles different protocol versions (V1 and V2) with unified interface

pub mod module;
pub mod module_6;
pub mod v1;
pub mod v2;

use crate::config::IMAGE_BUFFER_SIZE;
use crate::device::{Device, DeviceConfig, ProtocolVersion};
use crate::protocol::module::ModuleSetCommand;
use heapless::Vec;

/// Parsed outcome of an Output Report (host -> device)
#[derive(Debug)]
#[allow(clippy::large_enum_variant)]
pub enum OutputReportResult {
    /// Update Key Image (cmd 0x07)
    KeyImageComplete {
        key_id: u8,
        image: Vec<u8, IMAGE_BUFFER_SIZE>,
    },
    /// Full LCD JPEG transfer complete (cmd 0x08)
    FullScreenImageComplete {
        image: Vec<u8, IMAGE_BUFFER_SIZE>,
    },
    /// Window strip JPEG complete (cmd 0x0B)
    WindowImageComplete {
        image: Vec<u8, IMAGE_BUFFER_SIZE>,
    },
    /// Partial window JPEG complete (cmd 0x0C)
    PartialWindowImageComplete {
        x: u16,
        y: u16,
        width: u16,
        height: u16,
        image: Vec<u8, IMAGE_BUFFER_SIZE>,
    },
    /// Background slot JPEG complete (cmd 0x0D)
    BackgroundImageComplete {
        index: u8,
        image: Vec<u8, IMAGE_BUFFER_SIZE>,
    },
    /// Legacy / in-progress chunk (not assembled)
    FullScreenImageChunk,
    BootLogoImageChunk,
    Unhandled,
}

/// Protocol-specific image processing result
#[derive(Debug)]
#[allow(clippy::large_enum_variant)]
pub enum ImageProcessResult {
    /// Image processing complete, ready to display
    Complete {
        key_id: u8,
        image: Vec<u8, IMAGE_BUFFER_SIZE>,
    },
    /// More packets needed to complete image
    Incomplete,
    /// Error processing image
    Error(&'static str),
}

/// Button mapping result for different devices
#[derive(Debug)]
pub struct ButtonMapping {
    pub mapped_buttons: [bool; crate::types::MAX_BUTTON_SLOTS],
    pub active_count: usize,
}

/// Map physical scan order (row-major, `cols` × `rows`) to protocol key order.
/// `left_to_right` matches [`crate::device::ButtonLayout::left_to_right`].
pub fn map_buttons_grid(
    physical_buttons: &[bool],
    cols: usize,
    rows: usize,
    left_to_right: bool,
) -> ButtonMapping {
    let mut mapped_buttons = [false; crate::types::MAX_BUTTON_SLOTS];
    let total_keys = cols
        .saturating_mul(rows)
        .min(crate::types::MAX_BUTTON_SLOTS);

    for (physical_idx, &pressed) in physical_buttons.iter().take(total_keys).enumerate() {
        let mapped_idx = if left_to_right {
            physical_idx
        } else {
            let row = physical_idx / cols;
            let col = physical_idx % cols;
            let reversed_col = cols.saturating_sub(1).saturating_sub(col);
            row * cols + reversed_col
        };

        if mapped_idx < crate::types::MAX_BUTTON_SLOTS {
            mapped_buttons[mapped_idx] = pressed;
        }
    }

    ButtonMapping {
        mapped_buttons,
        active_count: total_keys,
    }
}

// --- Feature GET report helpers (shared by V1 / V2 / Module) ---

#[inline]
pub fn feature_report_clamp(total_len: usize, buf_len: usize) -> usize {
    total_len.min(buf_len)
}

#[inline]
pub fn feature_report_zero_prefix(buf: &mut [u8], cap: usize) {
    buf.iter_mut().take(cap).for_each(|b| *b = 0);
}

/// V1 Mini-style: `[0]=RID`, `0x0C 31 33 00`, ASCII payload from byte 5.
pub fn fill_feature_v1_fw_string_report(
    buf: &mut [u8],
    report_id: u8,
    total_len: usize,
    ascii: &[u8],
) -> Option<usize> {
    let cap = feature_report_clamp(total_len, buf.len());
    if cap == 0 {
        return None;
    }
    feature_report_zero_prefix(buf, cap);
    buf[0] = report_id;
    buf[1] = 0x0c;
    buf[2] = 0x31;
    buf[3] = 0x33;
    buf[4] = 0x00;
    let start = 5usize;
    let end = (start + ascii.len()).min(cap);
    buf[start..end].copy_from_slice(&ascii[..(end - start)]);
    Some(cap)
}

/// Zeroed buffer, `[0]=RID`, copy `ascii` starting at `ascii_start`.
pub fn fill_feature_rid_ascii(
    buf: &mut [u8],
    report_id: u8,
    total_len: usize,
    ascii_start: usize,
    ascii: &[u8],
) -> Option<usize> {
    let cap = feature_report_clamp(total_len, buf.len());
    if cap == 0 {
        return None;
    }
    feature_report_zero_prefix(buf, cap);
    buf[0] = report_id;
    let end = (ascii_start + ascii.len()).min(cap);
    if end > ascii_start {
        buf[ascii_start..end].copy_from_slice(&ascii[..(end - ascii_start)]);
    }
    Some(cap)
}

/// Main protocol FW GET (reports `0x04` / `0x05` / `0x07`): `[0]=RID`, `[1]=0x0C`, four zero bytes, ASCII from byte 6.
pub fn fill_feature_v2_fw_version_report(
    buf: &mut [u8],
    report_id: u8,
    total_len: usize,
    ascii: &[u8],
) -> Option<usize> {
    let cap = feature_report_clamp(total_len, buf.len());
    if cap == 0 {
        return None;
    }
    feature_report_zero_prefix(buf, cap);
    buf[0] = report_id;
    buf[1] = 0x0c;
    buf[2..6].fill(0);
    let start = 6usize;
    let end = (start + ascii.len()).min(cap);
    buf[start..end].copy_from_slice(&ascii[..(end - start)]);
    Some(cap)
}

/// Protocol handler trait for different StreamDeck versions
pub trait ProtocolHandlerTrait {
    /// Get protocol version
    fn version(&self) -> ProtocolVersion;

    /// Parse an Output Report (host -> device)
    fn parse_output_report(&mut self, data: &[u8]) -> OutputReportResult;

    /// Map physical button layout to protocol button order
    fn map_buttons(
        &self,
        physical_buttons: &[bool],
        cols: usize,
        rows: usize,
        left_to_right: bool,
    ) -> ButtonMapping;

    /// Generate HID report descriptor for this protocol
    fn hid_descriptor(&self) -> &'static [u8];

    /// Get input report format size
    fn input_report_size(&self, button_count: usize) -> usize;

    /// Format button state into input report
    fn format_button_report(&self, buttons: &ButtonMapping, report: &mut [u8]) -> usize;

    /// Process feature report commands
    fn handle_feature_report(&mut self, report_id: u8, data: &[u8]) -> Option<ModuleSetCommand>;

    /// Build feature GET report. Default is unhandled.
    fn get_feature_report(&mut self, _report_id: u8, _buf: &mut [u8]) -> Option<usize> {
        None
    }
}

// Legacy ProtocolCommand has been unified into ModuleSetCommand/ModuleGetCommand.

/// Enum-based protocol handler for no_std environment
#[derive(Debug)]
pub enum ProtocolHandler {
    V1(v1::V1Handler),
    V2(v2::V2Handler),
    Module6Keys(module_6::Module6KeysHandler),
}

impl ProtocolHandler {
    /// Create handler for a compile-time / runtime selected device (preferred).
    pub fn create_for_device(device: Device) -> Self {
        match device.usb_config().protocol {
            ProtocolVersion::V1 => ProtocolHandler::V1(v1::V1Handler::new()),
            ProtocolVersion::V2 => ProtocolHandler::V2(v2::V2Handler::new(device)),
            ProtocolVersion::Module6Keys => {
                ProtocolHandler::Module6Keys(module_6::Module6KeysHandler::new())
            }
        }
    }

    fn as_trait(&self) -> &dyn ProtocolHandlerTrait {
        match self {
            ProtocolHandler::V1(h) => h,
            ProtocolHandler::V2(h) => h,
            ProtocolHandler::Module6Keys(h) => h,
        }
    }

    fn as_trait_mut(&mut self) -> &mut dyn ProtocolHandlerTrait {
        match self {
            ProtocolHandler::V1(h) => h,
            ProtocolHandler::V2(h) => h,
            ProtocolHandler::Module6Keys(h) => h,
        }
    }

    /// Get protocol version
    pub fn version(&self) -> ProtocolVersion {
        self.as_trait().version()
    }

    /// Parse Output Report (host -> device)
    pub fn parse_output_report(&mut self, data: &[u8]) -> OutputReportResult {
        self.as_trait_mut().parse_output_report(data)
    }

    /// Map physical button layout to protocol button order
    pub fn map_buttons(
        &self,
        physical_buttons: &[bool],
        cols: usize,
        rows: usize,
        left_to_right: bool,
    ) -> ButtonMapping {
        self.as_trait()
            .map_buttons(physical_buttons, cols, rows, left_to_right)
    }

    /// Generate HID report descriptor for this protocol
    pub fn hid_descriptor(&self) -> &'static [u8] {
        self.as_trait().hid_descriptor()
    }

    /// Get input report format size
    pub fn input_report_size(&self, button_count: usize) -> usize {
        self.as_trait().input_report_size(button_count)
    }

    /// Format button state into input report
    pub fn format_button_report(&self, buttons: &ButtonMapping, report: &mut [u8]) -> usize {
        self.as_trait().format_button_report(buttons, report)
    }

    /// Process feature report commands
    pub fn handle_feature_report(
        &mut self,
        report_id: u8,
        data: &[u8],
    ) -> Option<ModuleSetCommand> {
        self.as_trait_mut().handle_feature_report(report_id, data)
    }

    /// Delegate feature GET report building to the specific handler
    pub fn get_feature_report(&mut self, report_id: u8, buf: &mut [u8]) -> Option<usize> {
        self.as_trait_mut().get_feature_report(report_id, buf)
    }
}

/// Image format utilities
pub mod image {
    use super::*;

    /// One RGB888 triplet to big-endian RGB565 bytes (ST7736 / SPI wire order).
    #[inline]
    pub fn rgb888_pixel_to_rgb565_be(r: u8, g: u8, b: u8) -> [u8; 2] {
        let r5 = (r >> 3) as u16;
        let g6 = (g >> 2) as u16;
        let b5 = (b >> 3) as u16;
        let rgb565 = (r5 << 11) | (g6 << 5) | b5;
        [(rgb565 >> 8) as u8, (rgb565 & 0xFF) as u8]
    }

    /// Convert RGB888 to RGB565 for display
    pub fn rgb888_to_rgb565(rgb888: &[u8]) -> Vec<u8, 2048> {
        let mut rgb565_data = Vec::new();

        for chunk in rgb888.chunks_exact(3) {
            if let [r, g, b] = chunk {
                let be = rgb888_pixel_to_rgb565_be(*r, *g, *b);
                let _ = rgb565_data.push(be[0]);
                let _ = rgb565_data.push(be[1]);
            }
        }

        rgb565_data
    }

    /// Rotate image 270 degrees clockwise (for Mini devices)
    pub fn rotate_270(
        image_data: &[u8],
        width: usize,
        height: usize,
    ) -> Vec<u8, IMAGE_BUFFER_SIZE> {
        let mut rotated = Vec::new();

        // 270° rotation: new[y][x] = old[width - 1 - x][y]
        for new_y in 0..width {
            for new_x in 0..height {
                let old_x = width - 1 - new_y;
                let old_y = new_x;

                let old_idx = (old_y * width + old_x) * 3;
                if old_idx + 2 < image_data.len() {
                    let _ = rotated.push(image_data[old_idx]); // R
                    let _ = rotated.push(image_data[old_idx + 1]); // G
                    let _ = rotated.push(image_data[old_idx + 2]); // B
                }
            }
        }

        rotated
    }

    /// Flip image horizontally
    pub fn flip_horizontal(
        image_data: &[u8],
        width: usize,
        height: usize,
    ) -> Vec<u8, IMAGE_BUFFER_SIZE> {
        let mut flipped = Vec::new();

        for y in 0..height {
            for x in 0..width {
                let src_x = width - 1 - x;
                let src_idx = (y * width + src_x) * 3;

                if src_idx + 2 < image_data.len() {
                    let _ = flipped.push(image_data[src_idx]); // R
                    let _ = flipped.push(image_data[src_idx + 1]); // G
                    let _ = flipped.push(image_data[src_idx + 2]); // B
                }
            }
        }

        flipped
    }

    /// Flip image vertically  
    pub fn flip_vertical(
        image_data: &[u8],
        width: usize,
        height: usize,
    ) -> Vec<u8, IMAGE_BUFFER_SIZE> {
        let mut flipped = Vec::new();

        for y in 0..height {
            let src_y = height - 1 - y;
            for x in 0..width {
                let src_idx = (src_y * width + x) * 3;

                if src_idx + 2 < image_data.len() {
                    let _ = flipped.push(image_data[src_idx]); // R
                    let _ = flipped.push(image_data[src_idx + 1]); // G
                    let _ = flipped.push(image_data[src_idx + 2]); // B
                }
            }
        }

        flipped
    }

    /// Apply device-specific image transformations
    pub fn apply_transformations(
        image_data: &[u8],
        width: usize,
        height: usize,
        needs_rotation: bool,
        should_flip_horizontal: bool,
        should_flip_vertical: bool,
    ) -> Vec<u8, IMAGE_BUFFER_SIZE> {
        let mut result_data = Vec::new();
        let _ = result_data.extend_from_slice(image_data);

        if needs_rotation {
            result_data = rotate_270(&result_data, width, height);
        }

        if should_flip_horizontal {
            result_data = flip_horizontal(&result_data, width, height);
        }

        if should_flip_vertical {
            result_data = flip_vertical(&result_data, width, height);
        }

        result_data
    }
}
