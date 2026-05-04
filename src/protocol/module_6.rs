//! StreamDeck Module HID Protocol Handler (6 keys)
//!
//! Legacy Mini-family protocol per Elgato docs:
//! https://docs.elgato.com/streamdeck/hid/mini

use super::{
    feature_report_clamp, feature_report_zero_prefix, fill_feature_rid_ascii, map_buttons_grid,
    ButtonMapping, OutputReportResult, ProtocolHandlerTrait,
};
use crate::config::MODULE6_BMP_CAP;
use crate::device::ProtocolVersion;
use crate::protocol::module::{FirmwareType, ModuleGetCommand, ModuleSetCommand};
use heapless::Vec;

#[derive(Debug)]
pub struct Module6KeysHandler {
    image_buffer: Vec<u8, MODULE6_BMP_CAP>,
    receiving: bool,
    expected_key: u8,
}

impl Module6KeysHandler {
    pub fn new() -> Self {
        Self {
            image_buffer: Vec::new(),
            receiving: false,
            expected_key: 0,
        }
    }

    fn reset_rx(&mut self) {
        self.image_buffer.clear();
        self.receiving = false;
        self.expected_key = 0;
    }

    /// BMP file size from `bfSize` once `BM` magic is present.
    fn bmp_total_bytes(buf: &[u8]) -> Option<usize> {
        if buf.len() < 6 {
            return None;
        }
        if buf[0] != b'B' || buf[1] != b'M' {
            return None;
        }
        let bf_size = u32::from_le_bytes(buf[2..6].try_into().ok()?) as usize;
        if !(54..=MODULE6_BMP_CAP).contains(&bf_size) {
            return None;
        }
        Some(bf_size)
    }
}

impl Default for Module6KeysHandler {
    fn default() -> Self {
        Self::new()
    }
}

impl Module6KeysHandler {
    fn parse_module_set_command(&self, report_id: u8, data: &[u8]) -> Option<ModuleSetCommand> {
        match report_id {
            0x05 => {
                if data.len() >= 5
                    && data[0] == 0x55
                    && data[1] == 0xAA
                    && data[2] == 0xD1
                    && data[3] == 0x01
                {
                    Some(ModuleSetCommand::SetBrightness { value: data[4] })
                } else {
                    None
                }
            }
            0x0B => {
                if !data.is_empty() {
                    match data[0] {
                        0x63 => {
                            if data.len() >= 2 {
                                match data[1] {
                                    0x00 => Some(ModuleSetCommand::ShowLogo),
                                    0x02 => {
                                        let slice = if data.len() >= 3 { data[2] } else { 0 };
                                        Some(ModuleSetCommand::UpdateBootLogo { slice })
                                    }
                                    _ => None,
                                }
                            } else {
                                None
                            }
                        }
                        0xA2 => {
                            if data.len() >= 5 {
                                let secs = i32::from_le_bytes([data[1], data[2], data[3], data[4]]);
                                Some(ModuleSetCommand::SetIdleTime { seconds: secs })
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
            _ => None,
        }
    }

    fn parse_module_get_command(&self, report_id: u8) -> Option<ModuleGetCommand> {
        match report_id {
            0xA0 => Some(ModuleGetCommand::GetFirmwareVersion(FirmwareType::LD)),
            0xA1 => Some(ModuleGetCommand::GetFirmwareVersion(FirmwareType::AP2)),
            0xA2 => Some(ModuleGetCommand::GetFirmwareVersion(FirmwareType::AP1)),
            0x03 => Some(ModuleGetCommand::GetUnitSerialNumber),
            0xA3 => Some(ModuleGetCommand::GetIdleTime),
            0x08 => Some(ModuleGetCommand::GetUnitInformation),
            _ => None,
        }
    }
}

impl Module6KeysHandler {
    fn get_firmware_version(&self, firmware_type: FirmwareType) -> &'static [u8] {
        match firmware_type {
            FirmwareType::LD => b"1.00.003",
            FirmwareType::AP2 => b"1.03.000",
            FirmwareType::AP1 => b"1.03.000",
        }
    }

    fn get_unit_serial_number(&self) -> &'static [u8] {
        crate::config::usb_serial_bytes()
    }
}

impl ProtocolHandlerTrait for Module6KeysHandler {
    fn version(&self) -> ProtocolVersion {
        ProtocolVersion::Module6Keys
    }

    fn parse_output_report(&mut self, data: &[u8]) -> OutputReportResult {
        // Upload Data to Image Memory Bank — Report ID 0x02, Command 0x01.
        // Layout: [0]=RID 0x02, [1]=Cmd 0x01, [2]=chunk idx, [3]=0x00, [4]=show flag,
        //          [5]=key idx, [6..0x10]=reserved, [0x10..]=payload.
        if data.len() < 17 {
            return OutputReportResult::Unhandled;
        }
        if data[0] != 0x02 || data[1] != 0x01 {
            return OutputReportResult::Unhandled;
        }

        let chunk_idx = data[2];
        let key_idx = data[5];
        let chunk_payload = data.get(0x10..).unwrap_or(&[]);

        if chunk_idx == 0 {
            self.reset_rx();
            self.receiving = true;
            self.expected_key = key_idx;
        } else if !self.receiving || key_idx != self.expected_key {
            self.reset_rx();
            return OutputReportResult::Unhandled;
        }

        if self.image_buffer.extend_from_slice(chunk_payload).is_err() {
            self.reset_rx();
            return OutputReportResult::Unhandled;
        }

        let Some(total) = Self::bmp_total_bytes(&self.image_buffer) else {
            return OutputReportResult::Unhandled;
        };

        if self.image_buffer.len() < total {
            return OutputReportResult::Unhandled;
        }

        let mut image = Vec::new();
        let slice = self.image_buffer.as_slice().get(..total).unwrap_or(&[]);
        if image.extend_from_slice(slice).is_err() {
            self.reset_rx();
            return OutputReportResult::Unhandled;
        }

        let completed_key = self.expected_key;
        self.reset_rx();

        OutputReportResult::KeyImageComplete {
            key_id: completed_key,
            image,
        }
    }

    fn map_buttons(
        &self,
        physical_buttons: &[bool],
        cols: usize,
        rows: usize,
        left_to_right: bool,
    ) -> ButtonMapping {
        map_buttons_grid(physical_buttons, cols, rows, left_to_right)
    }

    fn hid_descriptor(&self) -> &'static [u8] {
        const DESC: &[u8] = &[
            0x05, 0x0C, // Usage Page (Consumer)
            0x09, 0x01, // Usage (Consumer Control)
            0xA1, 0x01, // Collection (Application)
            // Input report 0x01 (keys)
            0x85, 0x01, //   Report ID 0x01
            0x05, 0x09, //   Usage Page (Button)
            0x19, 0x01, //   Usage Minimum (Button 1)
            0x29, 0x20, //   Usage Maximum (Button 32)
            0x15, 0x00, //   Logical Minimum (0)
            0x26, 0xFF, 0x00, //   Logical Maximum (255)
            0x75, 0x08, //   Report Size (8)
            0x95, 0x3F, //   Report Count (63)
            0x81, 0x02, //   Input (Data,Var,Abs)
            // Output report 0x02 (image/data chunks)
            0x85, 0x02, //   Report ID 0x02
            0x0A, 0x00, 0xFF, //   Usage (Vendor-Defined 0xFF00)
            0x15, 0x00, //   Logical Minimum (0)
            0x26, 0xFF, 0x00, //   Logical Maximum (255)
            0x75, 0x08, //   Report Size (8)
            0x96, 0xFF, 0x03, //   Report Count (1023)
            0x91, 0x02, //   Output (Data,Var,Abs)
            // Feature reports (common IDs)
            0x85, 0x03, 0x0A, 0x00, 0xFF, 0x15, 0x00, 0x26, 0xFF, 0x00, 0x75, 0x08, 0x95, 0x10,
            0xB1, 0x04, 0x85, 0x04, 0x0A, 0x00, 0xFF, 0x15, 0x00, 0x26, 0xFF, 0x00, 0x75, 0x08,
            0x95, 0x10, 0xB1, 0x04, 0x85, 0x05, 0x0A, 0x00, 0xFF, 0x15, 0x00, 0x26, 0xFF, 0x00,
            0x75, 0x08, 0x95, 0x10, 0xB1, 0x04, 0x85, 0x07, 0x0A, 0x00, 0xFF, 0x15, 0x00, 0x26,
            0xFF, 0x00, 0x75, 0x08, 0x95, 0x10, 0xB1, 0x04, 0x85, 0x08, 0x0A, 0x00, 0xFF, 0x15,
            0x00, 0x26, 0xFF, 0x00, 0x75, 0x08, 0x95, 0x10, 0xB1, 0x04, 0x85, 0x0B, 0x0A, 0x00,
            0xFF, 0x15, 0x00, 0x26, 0xFF, 0x00, 0x75, 0x08, 0x95, 0x10, 0xB1, 0x04, 0x85, 0xA0,
            0x0A, 0x00, 0xFF, 0x15, 0x00, 0x26, 0xFF, 0x00, 0x75, 0x08, 0x95, 0x10, 0xB1, 0x04,
            0x85, 0xA1, 0x0A, 0x00, 0xFF, 0x15, 0x00, 0x26, 0xFF, 0x00, 0x75, 0x08, 0x95, 0x10,
            0xB1, 0x04, 0x85, 0xA2, 0x0A, 0x00, 0xFF, 0x15, 0x00, 0x26, 0xFF, 0x00, 0x75, 0x08,
            0x95, 0x10, 0xB1, 0x04, 0x85, 0xA3, 0x0A, 0x00, 0xFF, 0x15, 0x00, 0x26, 0xFF, 0x00,
            0x75, 0x08, 0x95, 0x10, 0xB1, 0x04, 0xC0,
        ];
        DESC
    }

    fn input_report_size(&self, _button_count: usize) -> usize {
        65
    }

    fn format_button_report(&self, buttons: &ButtonMapping, report: &mut [u8]) -> usize {
        const MAX_USB_SIZE: usize = 64;

        if report.len() < MAX_USB_SIZE {
            return 0;
        }

        report[0] = 0x01;

        let button_count = core::cmp::min(6, buttons.mapped_buttons.len());
        for i in 0..button_count {
            report[1 + i] = if buttons.mapped_buttons[i] { 1 } else { 0 };
        }

        report
            .iter_mut()
            .take(MAX_USB_SIZE)
            .skip(1 + button_count)
            .for_each(|b| *b = 0);

        MAX_USB_SIZE
    }

    fn handle_feature_report(&mut self, report_id: u8, data: &[u8]) -> Option<ModuleSetCommand> {
        self.parse_module_set_command(report_id, data)
    }

    fn get_feature_report(&mut self, report_id: u8, buf: &mut [u8]) -> Option<usize> {
        self.get_feature_report_bytes(report_id, buf)
    }
}

impl Module6KeysHandler {
    pub fn get_feature_report_bytes(&self, report_id: u8, buf: &mut [u8]) -> Option<usize> {
        let total_len = 32.min(buf.len());
        if let Some(cmd) = self.parse_module_get_command(report_id) {
            match cmd {
                ModuleGetCommand::GetFirmwareVersion(ftype) => {
                    let ver = self.get_firmware_version(ftype);
                    fill_feature_rid_ascii(buf, report_id, total_len, 5, ver)
                }
                ModuleGetCommand::GetUnitSerialNumber => {
                    fill_feature_rid_ascii(buf, 0x03, total_len, 5, self.get_unit_serial_number())
                }
                ModuleGetCommand::GetIdleTime => {
                    let cap = feature_report_clamp(total_len, buf.len());
                    if cap == 0 {
                        return None;
                    }
                    feature_report_zero_prefix(buf, cap);
                    buf[0] = 0xA3;
                    buf[1] = 0x04;
                    let secs = crate::config::get_idle_time_seconds();
                    let le = secs.to_le_bytes();
                    buf[2] = le[0];
                    buf[3] = le[1];
                    buf[4] = le[2];
                    buf[5] = le[3];
                    Some(cap)
                }
                ModuleGetCommand::GetUnitInformation => {
                    let tail = crate::device::Device::Module6Keys.unit_information_tail();
                    let cap = feature_report_clamp(total_len, buf.len());
                    if cap < 5 + tail.len() {
                        return None;
                    }
                    feature_report_zero_prefix(buf, cap);
                    buf[0] = 0x08;
                    buf[1..5].copy_from_slice(&[0u8; 4]);
                    buf[5..5 + tail.len()].copy_from_slice(&tail);
                    Some(cap)
                }
            }
        } else {
            None
        }
    }
}
