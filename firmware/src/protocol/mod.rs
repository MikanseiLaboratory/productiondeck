//! StreamDeck protocol abstraction layer
//!
//! Handles different protocol versions (V1 and V2) with unified interface

pub mod module;
pub mod module_15_32;
pub mod module_6;
pub mod v1;
pub mod v2;

use crate::config::IMAGE_BUFFER_SIZE;
use crate::device::ProtocolVersion;
use crate::protocol::module::ModuleSetCommand;
use heapless::Vec;

/// Parsed outcome of an Output Report (host -> device)
#[derive(Debug)]
#[allow(clippy::large_enum_variant)]
pub enum OutputReportResult {
    /// Update Key Image (Module 15/32: cmd 0x07, Module 6: cmd 0x01 with ShowFlag=1)
    KeyImageComplete {
        key_id: u8,
        image: Vec<u8, IMAGE_BUFFER_SIZE>,
    },
    /// Update Full Screen Image (Module 15/32: cmd 0x08)
    FullScreenImageChunk,
    /// Update Boot Logo (Module 15/32: cmd 0x09, Module 6 uses Feature combo)
    BootLogoImageChunk,
    /// Output report not recognized/unsupported for current device
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
    pub mapped_buttons: [bool; 32], // Max buttons supported (XL has 32)
    pub active_count: usize,
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
    Module15_32Keys(module_15_32::Module15_32KeysHandler),
}

impl ProtocolHandler {
    /// Create appropriate protocol handler based on version
    pub fn create(version: ProtocolVersion) -> Self {
        match version {
            ProtocolVersion::V1 => ProtocolHandler::V1(v1::V1Handler::new()),
            ProtocolVersion::V2 => ProtocolHandler::V2(v2::V2Handler::new()),
            ProtocolVersion::Module6Keys => {
                ProtocolHandler::Module6Keys(module_6::Module6KeysHandler::new())
            }
            ProtocolVersion::Module15_32Keys => {
                ProtocolHandler::Module15_32Keys(module_15_32::Module15_32KeysHandler::new())
            }
        }
    }

    /// Get protocol version
    pub fn version(&self) -> ProtocolVersion {
        match self {
            ProtocolHandler::V1(_) => ProtocolVersion::V1,
            ProtocolHandler::V2(_) => ProtocolVersion::V2,
            ProtocolHandler::Module6Keys(_) => ProtocolVersion::Module6Keys,
            ProtocolHandler::Module15_32Keys(_) => ProtocolVersion::Module15_32Keys,
        }
    }

    /// Parse Output Report (host -> device)
    pub fn parse_output_report(&mut self, data: &[u8]) -> OutputReportResult {
        match self {
            ProtocolHandler::V1(handler) => handler.parse_output_report(data),
            ProtocolHandler::V2(handler) => handler.parse_output_report(data),
            ProtocolHandler::Module6Keys(handler) => handler.parse_output_report(data),
            ProtocolHandler::Module15_32Keys(handler) => handler.parse_output_report(data),
        }
    }

    /// Map physical button layout to protocol button order
    pub fn map_buttons(
        &self,
        physical_buttons: &[bool],
        cols: usize,
        rows: usize,
        left_to_right: bool,
    ) -> ButtonMapping {
        match self {
            ProtocolHandler::V1(handler) => {
                handler.map_buttons(physical_buttons, cols, rows, left_to_right)
            }
            ProtocolHandler::V2(handler) => {
                handler.map_buttons(physical_buttons, cols, rows, left_to_right)
            }
            ProtocolHandler::Module6Keys(handler) => {
                handler.map_buttons(physical_buttons, cols, rows, left_to_right)
            }
            ProtocolHandler::Module15_32Keys(handler) => {
                handler.map_buttons(physical_buttons, cols, rows, left_to_right)
            }
        }
    }

    /// Generate HID report descriptor for this protocol
    pub fn hid_descriptor(&self) -> &'static [u8] {
        match self {
            ProtocolHandler::V1(handler) => handler.hid_descriptor(),
            ProtocolHandler::V2(handler) => handler.hid_descriptor(),
            ProtocolHandler::Module6Keys(handler) => handler.hid_descriptor(),
            ProtocolHandler::Module15_32Keys(handler) => handler.hid_descriptor(),
        }
    }

    /// Get input report format size
    pub fn input_report_size(&self, button_count: usize) -> usize {
        match self {
            ProtocolHandler::V1(handler) => handler.input_report_size(button_count),
            ProtocolHandler::V2(handler) => handler.input_report_size(button_count),
            ProtocolHandler::Module6Keys(handler) => handler.input_report_size(button_count),
            ProtocolHandler::Module15_32Keys(handler) => handler.input_report_size(button_count),
        }
    }

    /// Format button state into input report
    pub fn format_button_report(&self, buttons: &ButtonMapping, report: &mut [u8]) -> usize {
        match self {
            ProtocolHandler::V1(handler) => handler.format_button_report(buttons, report),
            ProtocolHandler::V2(handler) => handler.format_button_report(buttons, report),
            ProtocolHandler::Module6Keys(handler) => handler.format_button_report(buttons, report),
            ProtocolHandler::Module15_32Keys(handler) => {
                handler.format_button_report(buttons, report)
            }
        }
    }

    /// Process feature report commands
    pub fn handle_feature_report(
        &mut self,
        report_id: u8,
        data: &[u8],
    ) -> Option<ModuleSetCommand> {
        match self {
            ProtocolHandler::V1(handler) => handler.handle_feature_report(report_id, data),
            ProtocolHandler::V2(handler) => handler.handle_feature_report(report_id, data),
            ProtocolHandler::Module6Keys(handler) => handler.handle_feature_report(report_id, data),
            ProtocolHandler::Module15_32Keys(handler) => {
                handler.handle_feature_report(report_id, data)
            }
        }
    }

    /// Delegate feature GET report building to the specific handler
    pub fn get_feature_report(&mut self, report_id: u8, buf: &mut [u8]) -> Option<usize> {
        match self {
            ProtocolHandler::V1(handler) => handler.get_feature_report(report_id, buf),
            ProtocolHandler::V2(handler) => handler.get_feature_report(report_id, buf),
            ProtocolHandler::Module6Keys(handler) => handler.get_feature_report(report_id, buf),
            ProtocolHandler::Module15_32Keys(handler) => handler.get_feature_report(report_id, buf),
        }
    }
}

/// Image format utilities
pub mod image {
    use super::*;

    /// Convert RGB888 to RGB565 for display
    pub fn rgb888_to_rgb565(rgb888: &[u8]) -> Vec<u8, 2048> {
        let mut rgb565_data = Vec::new();

        for chunk in rgb888.chunks_exact(3) {
            if let [r, g, b] = chunk {
                let r5 = (r >> 3) as u16;
                let g6 = (g >> 2) as u16;
                let b5 = (b >> 3) as u16;

                let rgb565 = (r5 << 11) | (g6 << 5) | b5;

                // Store as big-endian for display
                let _ = rgb565_data.push((rgb565 >> 8) as u8);
                let _ = rgb565_data.push((rgb565 & 0xFF) as u8);
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
