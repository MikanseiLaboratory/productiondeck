//! StreamDeck Module HID Protocol Handler (6keys)
//!
//! Implements the unified `ProtocolHandlerTrait` for the Elgato Stream Deck
//! Modules per public HID API docs. Image upload parsing is stubbed until we
//! confirm exact chunk layout from PCAPs.

use super::{ButtonMapping, OutputReportResult, ProtocolHandlerTrait};
use crate::device::ProtocolVersion;
use crate::protocol::module::{FirmwareType, ModuleGetCommand, ModuleSetCommand};

#[derive(Debug)]
pub struct Module6KeysHandler {}

impl Module6KeysHandler {
    pub fn new() -> Self {
        Self {}
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
                // Payload excludes Report ID. Per spec:
                // [Command=0x55, 0xAA, 0xD1, 0x01, Brightness]
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
                // Payload excludes Report ID.
                // Commands at data[0]
                if !data.is_empty() {
                    match data[0] {
                        0x63 => {
                            // data[1]: 0x00 Show Logo, 0x02 Update Boot Logo
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
                            // data[1..=4]: i32 seconds (LE)
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
            0x08 => Some(ModuleGetCommand::GetUnitInformation), // Module 6 compatibility
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
        b"1234567890"
    }
}

impl ProtocolHandlerTrait for Module6KeysHandler {
    fn version(&self) -> ProtocolVersion {
        ProtocolVersion::Module6Keys
    }

    fn parse_output_report(&mut self, data: &[u8]) -> OutputReportResult {
        let report_id = data[0];
        let command = data[1];

        match report_id {
            // https://docs.elgato.com/streamdeck/hid/module-6#upload-data-to-image-memory-bank
            0x02 => {
                if command == 0x01 {
                    let _chunk_index = data[2];
                    let _reserved = data[3];
                    let _show_image_flag = data[4];
                    let _key_index = data[5];
                    let _reserved = &data[6..0x10];
                    let _chunk_data = &data[0x10..];

                    OutputReportResult::Unhandled
                } else {
                    OutputReportResult::Unhandled
                }
            }
            _ => OutputReportResult::Unhandled,
        }
    }

    fn map_buttons(
        &self,
        physical_buttons: &[bool],
        cols: usize,
        rows: usize,
        left_to_right: bool,
    ) -> ButtonMapping {
        let mut mapped = [false; 32];

        for y in 0..rows {
            for x in 0..cols {
                let src_index = if left_to_right {
                    y * cols + x
                } else {
                    y * cols + (cols - 1 - x)
                };
                let dst_index = y * cols + x;
                if src_index < physical_buttons.len() && dst_index < 32 {
                    mapped[dst_index] = physical_buttons[src_index];
                }
            }
        }
        ButtonMapping {
            mapped_buttons: mapped,
            active_count: 6,
        }
    }

    fn hid_descriptor(&self) -> &'static [u8] {
        // Minimal descriptor covering Input(0x01), Output(0x02), Feature(0x03/0x04/0x05/0x07/0x08/0x0B/0xA0/0xA1/0xA2/0xA3)
        // This can be fine-tuned to match exact real devices if needed.
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
            0x95, 0x3F, //   Report Count (63) -> total 64 bytes incl. Report ID
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
            0x75, 0x08, 0x95, 0x10, 0xB1, 0x04, 0xC0, // End Collection
        ];
        DESC
    }

    fn input_report_size(&self, _button_count: usize) -> usize {
        65
    }

    fn format_button_report(&self, buttons: &ButtonMapping, report: &mut [u8]) -> usize {
        // 64 bytes total per packet: Report ID (1) + 63 data bytes
        const MAX_USB_SIZE: usize = 64;

        if report.len() < MAX_USB_SIZE {
            return 0;
        }

        // Set Report ID
        report[0] = 0x01;

        // Map up to 63 data bytes; Module 6 needs first 6
        let button_count = core::cmp::min(6, buttons.mapped_buttons.len());
        for i in 0..button_count {
            report[1 + i] = if buttons.mapped_buttons[i] { 1 } else { 0 };
        }

        // Zero out remaining bytes in the USB packet
        report
            .iter_mut()
            .take(MAX_USB_SIZE)
            .skip(1 + button_count)
            .for_each(|b| *b = 0);

        MAX_USB_SIZE
    }

    fn handle_feature_report(&mut self, report_id: u8, data: &[u8]) -> Option<ModuleSetCommand> {
        if let Some(cmd) = self.parse_module_set_command(report_id, data) {
            return Some(cmd);
        }
        None
    }

    fn get_feature_report(&mut self, report_id: u8, buf: &mut [u8]) -> Option<usize> {
        self.get_feature_report_bytes(report_id, buf)
    }
}

impl Module6KeysHandler {
    pub fn get_feature_report_bytes(&self, report_id: u8, buf: &mut [u8]) -> Option<usize> {
        let total_len = 32.min(buf.len());
        buf.iter_mut().take(total_len).for_each(|b| *b = 0);
        if let Some(cmd) = self.parse_module_get_command(report_id) {
            match cmd {
                ModuleGetCommand::GetFirmwareVersion(ftype) => {
                    let ver = self.get_firmware_version(ftype);
                    buf[0] = report_id;
                    // bytes 1..4 are N/A (0), version ASCII at offset 5
                    let start = 5;
                    let end = (start + ver.len()).min(total_len);
                    // bytes 1..4 already zeroed above
                    if end > start {
                        buf[start..end].copy_from_slice(&ver[..(end - start)]);
                    }
                    return Some(total_len);
                }
                ModuleGetCommand::GetUnitSerialNumber => {
                    let serial = self.get_unit_serial_number();
                    buf[0] = 0x03;
                    let start = 5;
                    let end = (start + serial.len()).min(total_len);
                    if end > start {
                        buf[start..end].copy_from_slice(&serial[..(end - start)]);
                    }
                    return Some(total_len);
                }
                ModuleGetCommand::GetIdleTime => {
                    buf[0] = 0xA3;
                    // Data length for INT32 duration is 4 bytes
                    buf[1] = 0x04;
                    let secs = crate::config::get_idle_time_seconds();
                    let le = secs.to_le_bytes();
                    buf[2] = le[0];
                    buf[3] = le[1];
                    buf[4] = le[2];
                    buf[5] = le[3];
                    return Some(total_len);
                }
                _ => {
                    return None;
                }
            }
        }
        None
    }
}
