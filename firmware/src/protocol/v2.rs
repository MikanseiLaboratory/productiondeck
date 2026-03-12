//! StreamDeck V2 Protocol Handler
//!
//! Handles Original V2, XL, MK2, and Plus devices using JPEG format

use super::{ButtonMapping, OutputReportResult, ProtocolHandlerTrait};
use crate::config::{
    IMAGE_COMMAND_V2, IMAGE_PROCESSING_BUFFER_SIZE, OUTPUT_REPORT_IMAGE, V2_COMMAND_BRIGHTNESS,
    V2_COMMAND_RESET,
};
use crate::device::ProtocolVersion;
use crate::protocol::module::ModuleSetCommand;
use heapless::Vec;

/// V2 Protocol Handler for JPEG-based StreamDeck devices
#[derive(Debug)]
pub struct V2Handler {
    image_buffer: Vec<u8, IMAGE_PROCESSING_BUFFER_SIZE>,
    receiving_image: bool,
    expected_key: u8,
    expected_sequence: u16,
}

impl V2Handler {
    pub fn new() -> Self {
        Self {
            image_buffer: Vec::new(),
            receiving_image: false,
            expected_key: 0,
            expected_sequence: 0,
        }
    }

    /// Reset image reception state
    fn reset_image_state(&mut self) {
        self.image_buffer.clear();
        self.receiving_image = false;
        self.expected_key = 0;
        self.expected_sequence = 0;
    }
}

impl Default for V2Handler {
    fn default() -> Self {
        Self::new()
    }
}

impl ProtocolHandlerTrait for V2Handler {
    fn version(&self) -> ProtocolVersion {
        ProtocolVersion::V2
    }

    fn parse_output_report(&mut self, data: &[u8]) -> OutputReportResult {
        if data.len() < 8 {
            return OutputReportResult::Unhandled;
        }

        // V2 Output Report: Command 0x07 (key), 0x08 (full LCD), 0x09 (boot logo)
        // Key image format primary: [0x02, 0x07, key_id, is_last, len_lo, len_hi, seq_lo, seq_hi, data...]
        // Some HID stacks strip the report ID before delivering data to set_report. Accept both forms.
        let (cmd, key_id, is_last, payload_len, sequence, data_start) =
            if data[0] == OUTPUT_REPORT_IMAGE {
                let cmd = data[1];
                if cmd == IMAGE_COMMAND_V2 {
                    (
                        cmd,
                        data[2],
                        data[3] != 0,
                        u16::from_le_bytes([data[4], data[5]]),
                        u16::from_le_bytes([data[6], data[7]]),
                        8,
                    )
                } else {
                    (cmd, 0, false, 0, 0, 0)
                }
            } else if data[0] == IMAGE_COMMAND_V2 && data.len() >= 7 {
                // Missing report ID (0x02) case for 0x07
                (
                    IMAGE_COMMAND_V2,
                    data[1],
                    data[2] != 0,
                    u16::from_le_bytes([data[3], data[4]]),
                    u16::from_le_bytes([data[5], data[6]]),
                    7,
                )
            } else {
                return OutputReportResult::Unhandled;
            };

        if cmd != IMAGE_COMMAND_V2 {
            // For now, only branch key updates. Full screen / boot logo recognized but not assembled here.
            return match cmd {
                0x08 => OutputReportResult::FullScreenImageChunk,
                0x09 => OutputReportResult::BootLogoImageChunk,
                _ => OutputReportResult::Unhandled,
            };
        }

        // First packet (sequence 0) starts image reception
        if sequence == 0 {
            self.reset_image_state();
            self.receiving_image = true;
            self.expected_key = key_id;
            self.expected_sequence = 0;
        }

        // Validate sequence and key
        if !self.receiving_image
            || key_id != self.expected_key
            || sequence != self.expected_sequence
        {
            // Reset and ignore to keep host happy
            self.reset_image_state();
            return OutputReportResult::Unhandled;
        }

        // Copy payload data
        let copy_len = (payload_len as usize).min(data.len() - data_start);

        if copy_len > 0
            && self
                .image_buffer
                .extend_from_slice(&data[data_start..data_start + copy_len])
                .is_err()
        {
            self.reset_image_state();
            return OutputReportResult::Unhandled;
        }

        self.expected_sequence += 1;

        if is_last {
            // Image complete
            let mut complete_image = Vec::new();
            let _ = complete_image.extend_from_slice(&self.image_buffer);
            let completed_key = self.expected_key;
            self.reset_image_state();

            OutputReportResult::KeyImageComplete {
                key_id: completed_key,
                image: complete_image,
            }
        } else {
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

        // V2 devices generally use left-to-right mapping
        for (physical_idx, &pressed) in physical_buttons.iter().take(total_keys).enumerate() {
            let mapped_idx = if left_to_right {
                physical_idx
            } else {
                // Right-to-left if needed (rare for V2 devices)
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
        // V2 StreamDeck HID descriptor (similar to V1 but optimized for V2 protocol)
        &[
            0x05, 0x0c, // Usage Page (Consumer)
            0x09, 0x01, // Usage (Consumer Control)
            0xa1, 0x01, // Collection (Application)
            0x09, 0x01, // Usage (Consumer Control)
            0x05, 0x09, // Usage Page (Button)
            0x19, 0x01, // Usage Minimum (0x01)
            0x29, 0x20, // Usage Maximum (0x20) - Support up to 32 buttons
            0x15, 0x00, // Logical Minimum (0)
            0x26, 0xff, 0x00, // Logical Maximum (255)
            0x75, 0x08, // Report Size (8)
            0x95, 0x20, // Report Count (32) - Support up to 32 buttons
            0x85, 0x01, // Report ID (0x01)
            0x81, 0x02, // Input (Data,Var,Abs)
            0x0a, 0x00, 0xff, // Usage (Button 255)
            0x15, 0x00, // Logical Minimum (0)
            0x26, 0xff, 0x00, // Logical Maximum (255)
            0x75, 0x08, // Report Size (8)
            0x96, 0x00, 0x04, // Report Count (1024) - Standard packet size
            0x85, 0x02, // Report ID (0x02)
            0x91, 0x02, // Output (Data,Var,Abs)
            0x0a, 0x00, 0xff, // Usage (Button 255)
            0x15, 0x00, // Logical Minimum (0)
            0x26, 0xff, 0x00, // Logical Maximum (255)
            0x75, 0x08, // Report Size (8)
            0x95, 0x20, // Report Count (32)
            0x85, 0x03, // Report ID (0x03)
            0xb1, 0x04, // Feature (Data,Array,Rel)
            0x0a, 0x00, 0xff, // Usage (Button 255)
            0x15, 0x00, // Logical Minimum (0)
            0x26, 0xff, 0x00, // Logical Maximum (255)
            0x75, 0x08, // Report Size (8)
            0x95, 0x20, // Report Count (32)
            0x85, 0x04, // Report ID (0x04)
            0xb1, 0x04, // Feature (Data,Array,Rel)
            0x0a, 0x00, 0xff, // Usage (Button 255)
            0x15, 0x00, // Logical Minimum (0)
            0x26, 0xff, 0x00, // Logical Maximum (255)
            0x75, 0x08, // Report Size (8)
            0x95, 0x20, // Report Count (32)
            0x85, 0x05, // Report ID (0x05)
            0xb1, 0x04, // Feature (Data,Array,Rel)
            0xc0, // End Collection
        ]
    }

    fn input_report_size(&self, button_count: usize) -> usize {
        // V2 input reports: 3-byte header + button states
        3 + button_count
    }

    fn format_button_report(&self, buttons: &ButtonMapping, report: &mut [u8]) -> usize {
        if report.len() < 4 {
            return 0;
        }

        // V2 format: [header_bytes, button_states...]
        report[0] = 0x00; // Header byte 1
        report[1] = 0x00; // Header byte 2
        report[2] = 0x00; // Header byte 3

        let button_bytes = (buttons.active_count).min(report.len() - 3);
        for i in 0..button_bytes {
            report[i + 3] = if buttons.mapped_buttons[i] { 1 } else { 0 };
        }

        // Fill remaining bytes with 0
        for b in report.iter_mut().skip(button_bytes + 3) {
            *b = 0;
        }

        3 + button_bytes
    }

    fn handle_feature_report(&mut self, report_id: u8, data: &[u8]) -> Option<ModuleSetCommand> {
        if report_id == 0x03 && data.len() >= 2 {
            // V2 commands: [0x03, command_byte, ...]
            match data[1] {
                V2_COMMAND_RESET => {
                    // V2 Reset: [0x03, 0x02, ...]
                    Some(ModuleSetCommand::Reset)
                }
                V2_COMMAND_BRIGHTNESS => {
                    // V2 Brightness: [0x03, 0x08, brightness, ...]
                    if data.len() >= 3 {
                        Some(ModuleSetCommand::SetBrightness { value: data[2] })
                    } else {
                        None
                    }
                }
                _ => None,
            }
        } else {
            None
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
            _ => None,
        }
    }
}
