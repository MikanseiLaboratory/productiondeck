//! StreamDeck V1 Protocol Handler
//!
//! Handles Original, Mini, and Revised Mini devices using BMP format

use super::{ButtonMapping, OutputReportResult, ProtocolHandlerTrait};
use crate::config::{
    FEATURE_REPORT_BRIGHTNESS_V1, IMAGE_PROCESSING_BUFFER_SIZE, STREAMDECK_BRIGHTNESS_RESET_MAGIC,
    STREAMDECK_MAGIC_1, STREAMDECK_MAGIC_2, STREAMDECK_MAGIC_3, STREAMDECK_RESET_MAGIC,
};
use crate::device::ProtocolVersion;
use crate::protocol::module::ModuleSetCommand;
use heapless::Vec;

/// V1 Protocol Handler for BMP-based StreamDeck devices
#[derive(Debug)]
pub struct V1Handler {
    image_buffer: Vec<u8, IMAGE_PROCESSING_BUFFER_SIZE>,
    receiving_image: bool,
    expected_key: u8,
}

impl V1Handler {
    pub fn new() -> Self {
        Self {
            image_buffer: Vec::new(),
            receiving_image: false,
            expected_key: 0,
        }
    }

    /// Reset image reception state
    fn reset_image_state(&mut self) {
        self.image_buffer.clear();
        self.receiving_image = false;
        self.expected_key = 0;
    }
}

impl Default for V1Handler {
    fn default() -> Self {
        Self::new()
    }
}

impl ProtocolHandlerTrait for V1Handler {
    fn version(&self) -> ProtocolVersion {
        ProtocolVersion::V1
    }

    fn parse_output_report(&mut self, data: &[u8]) -> OutputReportResult {
        if data.len() < 8 {
            return OutputReportResult::Unhandled;
        }

        // V1 Protocol format primary: [0x02, 0x01, packet_num, 0x00, 0x00, key_id, 0x00, 0x00, image_data...]
        // Accept variant where report ID (0x02) is stripped by HID stack: [0x01, packet_num, 0x00, 0x00, key_id, 0x00, 0x00, data...]
        let (packet_num, key_id, data_start) = if data[0] == 0x02 && data.len() >= 6 {
            (data[2], data[5], 8)
        } else if data[0] == 0x01 && data.len() >= 5 {
            (data[1], data[4], 7)
        } else {
            return OutputReportResult::Unhandled;
        };

        // First packet starts image reception
        if packet_num == 0x01 {
            self.reset_image_state();
            self.receiving_image = true;
            self.expected_key = key_id;

            // Skip header and copy image data
            if data.len() > data_start
                && self
                    .image_buffer
                    .extend_from_slice(&data[data_start..])
                    .is_err()
            {
                self.reset_image_state();
                return OutputReportResult::Unhandled;
            }

            OutputReportResult::Unhandled
        } else if packet_num == 0x02 && self.receiving_image && key_id == self.expected_key {
            // Second packet completes the image
            if data.len() > data_start
                && self
                    .image_buffer
                    .extend_from_slice(&data[data_start..])
                    .is_err()
            {
                self.reset_image_state();
                return OutputReportResult::Unhandled;
            }

            // V1 image is complete
            let mut complete_image = Vec::new();
            let _ = complete_image.extend_from_slice(&self.image_buffer);
            let completed_key = self.expected_key;
            self.reset_image_state();

            OutputReportResult::KeyImageComplete {
                key_id: completed_key,
                image: complete_image,
            }
        } else {
            // Ignore unexpected sequences for now
            OutputReportResult::Unhandled
        }
    }

    fn map_buttons(
        &self,
        physical_buttons: &[bool],
        cols: usize,
        rows: usize,
        left_to_right: bool,
    ) -> ButtonMapping {
        let mut mapped_buttons = [false; 32];
        let total_keys = cols * rows;

        for (physical_idx, &pressed) in physical_buttons.iter().take(total_keys).enumerate() {
            let mapped_idx = if left_to_right {
                physical_idx // Direct mapping for Mini and Revised Mini
            } else {
                // Right-to-left mapping for Original StreamDeck
                let row = physical_idx / cols;
                let col = physical_idx % cols;
                let reversed_col = cols - 1 - col;
                row * cols + reversed_col
            };

            if mapped_idx < 32 {
                mapped_buttons[mapped_idx] = pressed;
            }
        }

        ButtonMapping {
            mapped_buttons,
            active_count: total_keys,
        }
    }

    fn hid_descriptor(&self) -> &'static [u8] {
        // V1 StreamDeck HID descriptor (generic V1 implementation)
        // NOTE: Do not force the exact Mini (173-byte) descriptor here.
        // This generic descriptor suits V1 devices we target now.
        &[
            0x05, 0x0c, // Usage Page (Consumer)
            0x09, 0x01, // Usage (Consumer Control)
            0xa1, 0x01, // Collection (Application)
            0x09, 0x01, // Usage (Consumer Control)
            0x05, 0x09, // Usage Page (Button)
            0x19, 0x01, // Usage Minimum (0x01)
            0x29, 0x06, // Usage Maximum (0x06) - Mini has 6 buttons
            0x15, 0x00, // Logical Minimum (0)
            0x26, 0xff, 0x00, // Logical Maximum (255)
            0x75, 0x08, // Report Size (8)
            0x95, 0x06, // Report Count (6) - Mini buttons
            0x85, 0x01, // Report ID (0x01)
            0x81, 0x02, // Input (Data,Var,Abs)
            0x0a, 0x00, 0xff, // Usage (Button 255)
            0x15, 0x00, // Logical Minimum (0)
            0x26, 0xff, 0x00, // Logical Maximum (255)
            0x75, 0x08, // Report Size (8)
            0x96, 0xff, 0x03, // Report Count (1023)
            0x85, 0x02, // Report ID (0x02)
            0x91, 0x02, // Output (Data,Var,Abs)
            0x0a, 0x00, 0xff, // Usage (Button 255)
            0x15, 0x00, // Logical Minimum (0)
            0x26, 0xff, 0x00, // Logical Maximum (255)
            0x75, 0x08, // Report Size (8)
            0x95, 0x10, // Report Count (16)
            0x85, 0x03, // Report ID (0x03)
            0xb1, 0x04, // Feature (Data,Array,Rel)
            0x0a, 0x00, 0xff, // Usage (Button 255)
            0x15, 0x00, // Logical Minimum (0)
            0x26, 0xff, 0x00, // Logical Maximum (255)
            0x75, 0x08, // Report Size (8)
            0x95, 0x10, // Report Count (16)
            0x85, 0x04, // Report ID (0x04)
            0xb1, 0x04, // Feature (Data,Array,Rel)
            0x0a, 0x00, 0xff, // Usage (Button 255)
            0x15, 0x00, // Logical Minimum (0)
            0x26, 0xff, 0x00, // Logical Maximum (255)
            0x75, 0x08, // Report Size (8)
            0x95, 0x10, // Report Count (16)
            0x85, 0x05, // Report ID (0x05)
            0xb1, 0x04, // Feature (Data,Array,Rel)
            0x0a, 0x00, 0xff, // Usage (Button 255)
            0x15, 0x00, // Logical Minimum (0)
            0x26, 0xff, 0x00, // Logical Maximum (255)
            0x75, 0x08, // Report Size (8)
            0x95, 0x10, // Report Count (16)
            0x85, 0x07, // Report ID (0x07)
            0xb1, 0x04, // Feature (Data,Array,Rel)
            0x0a, 0x00, 0xff, // Usage (Button 255)
            0x15, 0x00, // Logical Minimum (0)
            0x26, 0xff, 0x00, // Logical Maximum (255)
            0x75, 0x08, // Report Size (8)
            0x95, 0x10, // Report Count (16)
            0x85, 0x0b, // Report ID (0x0b)
            0xb1, 0x04, // Feature (Data,Array,Rel)
            0x0a, 0x00, 0xff, // Usage (Button 255)
            0x15, 0x00, // Logical Minimum (0)
            0x26, 0xff, 0x00, // Logical Maximum (255)
            0x75, 0x08, // Report Size (8)
            0x95, 0x10, // Report Count (16)
            0x85, 0xa0, // Report ID (0xa0)
            0xb1, 0x04, // Feature (Data,Array,Rel)
            0x0a, 0x00, 0xff, // Usage (Button 255)
            0x15, 0x00, // Logical Minimum (0)
            0x26, 0xff, 0x00, // Logical Maximum (255)
            0x75, 0x08, // Report Size (8)
            0x95, 0x10, // Report Count (16)
            0x85, 0xa1, // Report ID (0xa1)
            0xb1, 0x04, // Feature (Data,Array,Rel)
            0x0a, 0x00, 0xff, // Usage (Button 255)
            0x15, 0x00, // Logical Minimum (0)
            0x26, 0xff, 0x00, // Logical Maximum (255)
            0x75, 0x08, // Report Size (8)
            0x95, 0x10, // Report Count (16)
            0x85, 0xa2, // Report ID (0xa2)
            0xb1, 0x04, // Feature (Data,Array,Rel)
            0xc0, // End Collection
        ]
    }

    fn input_report_size(&self, button_count: usize) -> usize {
        // V1 input reports: Report ID (1 byte) + button states (RP2040 USB hardware limitation)
        1 + button_count
    }

    fn format_button_report(&self, buttons: &ButtonMapping, report: &mut [u8]) -> usize {
        if report.is_empty() {
            return 0;
        }

        // V1 format: [0x01, button_states...]
        report[0] = 0x01; // Report ID

        let button_bytes = buttons.active_count.min(report.len() - 1);

        // Write actual button states
        for i in 0..button_bytes {
            report[i + 1] = if buttons.mapped_buttons[i] { 1 } else { 0 };
        }

        // Fill remaining bytes with 0
        for b in report.iter_mut().skip(button_bytes + 1) {
            *b = 0;
        }

        1 + button_bytes
    }

    fn handle_feature_report(&mut self, report_id: u8, data: &[u8]) -> Option<ModuleSetCommand> {
        match report_id {
            FEATURE_REPORT_BRIGHTNESS_V1 => {
                // V1 Brightness/Reset: [0x05, 0x55, 0xAA, 0xD1, 0x01, value, ...]
                if data.len() >= 6
                    && data[1] == STREAMDECK_MAGIC_1
                    && data[2] == STREAMDECK_MAGIC_2
                    && data[3] == STREAMDECK_MAGIC_3
                    && data[4] == 0x01
                {
                    if data[5] == STREAMDECK_BRIGHTNESS_RESET_MAGIC {
                        Some(ModuleSetCommand::Reset)
                    } else {
                        Some(ModuleSetCommand::SetBrightness { value: data[5] })
                    }
                } else {
                    None
                }
            }
            // Handle both V1 Reset and Module Idle Time (both use report 0x0B)
            0x0B => {
                if data.len() >= 6 && data[1] == crate::config::IDLE_TIME_COMMAND {
                    // Module Idle Time: [0x0B, 0xA2, seconds_le...]
                    let secs = i32::from_le_bytes([data[2], data[3], data[4], data[5]]);
                    Some(ModuleSetCommand::SetIdleTime { seconds: secs })
                } else if data.len() >= 2 && data[1] == STREAMDECK_RESET_MAGIC {
                    // V1 Reset: [0x0B, 0x63, ...]
                    Some(ModuleSetCommand::Reset)
                } else {
                    None
                }
            }
            _ => None,
        }
    }

    fn get_feature_report(&mut self, report_id: u8, buf: &mut [u8]) -> Option<usize> {
        match report_id {
            0xA0..=0xA2 => {
                let total_len = 32.min(buf.len());
                buf.iter_mut().take(total_len).for_each(|b| *b = 0);
                buf[0] = report_id;
                buf[1] = 0x0c; // Length
                buf[2] = 0x31; // Type
                buf[3] = 0x33; // Type
                buf[4] = 0x00; // Null terminator
                let version = b"3.00.000";
                let start = 5;
                let end = (start + version.len()).min(total_len);
                buf[start..end].copy_from_slice(&version[..(end - start)]);
                Some(total_len)
            }
            0x03 => {
                let total_len = 32.min(buf.len());
                buf.iter_mut().take(total_len).for_each(|b| *b = 0);
                buf[0] = report_id;
                buf[1] = 0x0c; // Length
                buf[2] = 0x31; // Type
                buf[3] = 0x33; // Type
                buf[4] = 0x00; // Null terminator
                let serial = crate::config::USB_SERIAL.as_bytes();
                let start = 5;
                let end = (start + serial.len()).min(total_len);
                buf[start..end].copy_from_slice(&serial[..(end - start)]);
                Some(total_len)
            }
            0x04 => {
                let total_len = 17.min(buf.len());
                buf.iter_mut().take(total_len).for_each(|b| *b = 0);
                buf[0] = report_id;
                let version = b"3.00.000";
                let start = 5;
                let end = (start + version.len()).min(total_len);
                buf[start..end].copy_from_slice(&version[..(end - start)]);
                Some(total_len)
            }
            0x05 => {
                let total_len = 32.min(buf.len());
                buf.iter_mut().take(total_len).for_each(|b| *b = 0);
                buf[0] = report_id;
                buf[1] = 0x0c; // Length
                buf[2] = 0x31; // Type
                buf[3] = 0x33; // Type
                buf[4] = 0x00; // Null terminator
                let version = b"3.00.000";
                let start = 5;
                let end = (start + version.len()).min(total_len);
                buf[start..end].copy_from_slice(&version[..(end - start)]);
                Some(total_len)
            }
            crate::config::FEATURE_REPORT_GET_IDLE_TIME => {
                let total_len = 32.min(buf.len());
                buf.iter_mut().take(total_len).for_each(|b| *b = 0);
                buf[0] = report_id;
                buf[1] = 0x06;
                let seconds = crate::config::get_idle_time_seconds();
                let secs_le = seconds.to_le_bytes();
                buf[2] = secs_le[0];
                buf[3] = secs_le[1];
                buf[4] = secs_le[2];
                buf[5] = secs_le[3];
                Some(total_len)
            }
            0x07 => {
                let total_len = 16.min(buf.len());
                buf.iter_mut().take(total_len).for_each(|b| *b = 0);
                buf[0] = report_id;
                Some(total_len)
            }
            _ => None,
        }
    }
}
