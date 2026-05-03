//! Stream Deck Main / Expanded protocol (V2) — JPEG image chunks, feature reports per Elgato General Reference.

use super::{
    feature_report_clamp, feature_report_zero_prefix, fill_feature_v2_fw_version_report,
    map_buttons_grid, ButtonMapping, OutputReportResult, ProtocolHandlerTrait,
};
use crate::config::{
    IMAGE_COMMAND_V2, IMAGE_PROCESSING_BUFFER_SIZE, OUTPUT_REPORT_IMAGE, V2_COMMAND_BRIGHTNESS,
    V2_COMMAND_RESET,
};
use crate::device::Device;
use crate::device::ProtocolVersion;
use crate::protocol::module::ModuleSetCommand;
use crate::types::MAX_BUTTON_SLOTS;
use heapless::Vec;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum V2ImageKind {
    Key,
    FullScreen,
    Window,
    PartialWindow,
    Background,
}

/// Parsed 0x0C partial-window image chunk header.
#[derive(Debug, Clone, Copy)]
struct PartialWindowChunk {
    x: u16,
    y: u16,
    w: u16,
    h: u16,
    is_last: bool,
    chunk_index: u16,
    chunk_size: u16,
    data_start: usize,
}

/// Arguments for [`V2Handler::ingest_chunk`].
struct IngestChunkParams<'a> {
    kind: V2ImageKind,
    slot: u8,
    partial: (u16, u16, u16, u16),
    sequence: u16,
    is_last: bool,
    payload_len: usize,
    data_start: usize,
    data: &'a [u8],
}

/// V2 / Main protocol handler (device-specific GET unit info, optional window/background transfers).
#[derive(Debug)]
pub struct V2Handler {
    device: Device,
    image_buffer: Vec<u8, IMAGE_PROCESSING_BUFFER_SIZE>,
    receiving: bool,
    kind: V2ImageKind,
    /// Key index (0x07) or background index (0x0D)
    slot: u8,
    expected_sequence: u16,
    partial: (u16, u16, u16, u16),
}

impl V2Handler {
    pub fn new(device: Device) -> Self {
        Self {
            device,
            image_buffer: Vec::new(),
            receiving: false,
            kind: V2ImageKind::Key,
            slot: 0,
            expected_sequence: 0,
            partial: (0, 0, 0, 0),
        }
    }

    fn reset_transfer(&mut self) {
        self.image_buffer.clear();
        self.receiving = false;
        self.expected_sequence = 0;
        self.slot = 0;
        self.partial = (0, 0, 0, 0);
    }

    fn feature_command(data: &[u8], report_id: u8) -> Option<u8> {
        if data.is_empty() {
            return None;
        }
        if data[0] == report_id && data.len() > 1 {
            Some(data[1])
        } else {
            Some(data[0])
        }
    }

    /// Parse 0x07 / 0x08 / 0x0B style: cmd, byte2, is_last, size_le, index_le, payload@8
    fn parse_standard_image_chunk(data: &[u8]) -> Option<(u8, u8, bool, u16, u16, usize)> {
        if data.len() < 8 {
            return None;
        }
        let cmd = data[0];
        let b2 = data[1];
        let is_last = data[2] != 0;
        let size = u16::from_le_bytes([data[3], data[4]]);
        let index = u16::from_le_bytes([data[5], data[6]]);
        Some((cmd, b2, is_last, size, index, 7))
    }

    /// Background 0x0D: chunk index @3-4, size @5-6 (swapped vs key image)
    fn parse_background_chunk(data: &[u8]) -> Option<(u8, bool, u16, u16, usize)> {
        if data.len() < 8 {
            return None;
        }
        let bg_index = data[1];
        let is_last = data[2] != 0;
        let chunk_index = u16::from_le_bytes([data[3], data[4]]);
        let chunk_size = u16::from_le_bytes([data[5], data[6]]);
        Some((bg_index, is_last, chunk_index, chunk_size, 7))
    }

    /// Partial window 0x0C
    fn parse_partial_chunk(data: &[u8]) -> Option<PartialWindowChunk> {
        if data.len() < 0x11 {
            return None;
        }
        let x = u16::from_le_bytes([data[1], data[2]]);
        let y = u16::from_le_bytes([data[3], data[4]]);
        let w = u16::from_le_bytes([data[5], data[6]]);
        let h = u16::from_le_bytes([data[7], data[8]]);
        let is_last = data[9] != 0;
        let chunk_index = u16::from_le_bytes([data[10], data[11]]);
        let chunk_size = u16::from_le_bytes([data[12], data[13]]);
        Some(PartialWindowChunk {
            x,
            y,
            w,
            h,
            is_last,
            chunk_index,
            chunk_size,
            data_start: 0x10,
        })
    }

    fn ingest_chunk(&mut self, p: IngestChunkParams<'_>) -> OutputReportResult {
        let IngestChunkParams {
            kind,
            slot,
            partial,
            sequence,
            is_last,
            payload_len,
            data_start,
            data,
        } = p;

        if sequence == 0 {
            self.reset_transfer();
            self.receiving = true;
            self.kind = kind;
            self.slot = slot;
            self.partial = partial;
            self.expected_sequence = 0;
        }

        if !self.receiving
            || self.kind != kind
            || self.slot != slot
            || (kind == V2ImageKind::PartialWindow && self.partial != partial)
            || sequence != self.expected_sequence
        {
            self.reset_transfer();
            return OutputReportResult::Unhandled;
        }

        let copy_len = payload_len.min(data.len().saturating_sub(data_start));
        if copy_len > 0
            && self
                .image_buffer
                .extend_from_slice(&data[data_start..data_start + copy_len])
                .is_err()
        {
            self.reset_transfer();
            return OutputReportResult::Unhandled;
        }

        self.expected_sequence = self.expected_sequence.wrapping_add(1);

        if is_last {
            let mut out = Vec::new();
            let _ = out.extend_from_slice(&self.image_buffer);
            self.reset_transfer();
            match kind {
                V2ImageKind::Key => OutputReportResult::KeyImageComplete {
                    key_id: slot,
                    image: out,
                },
                V2ImageKind::FullScreen => {
                    OutputReportResult::FullScreenImageComplete { image: out }
                }
                V2ImageKind::Window => OutputReportResult::WindowImageComplete { image: out },
                V2ImageKind::PartialWindow => OutputReportResult::PartialWindowImageComplete {
                    x: partial.0,
                    y: partial.1,
                    width: partial.2,
                    height: partial.3,
                    image: out,
                },
                V2ImageKind::Background => OutputReportResult::BackgroundImageComplete {
                    index: slot,
                    image: out,
                },
            }
        } else {
            OutputReportResult::Unhandled
        }
    }
}

impl ProtocolHandlerTrait for V2Handler {
    fn version(&self) -> ProtocolVersion {
        ProtocolVersion::V2
    }

    fn parse_output_report(&mut self, data: &[u8]) -> OutputReportResult {
        if data.len() < 2 {
            return OutputReportResult::Unhandled;
        }

        let payload = if data[0] == OUTPUT_REPORT_IMAGE {
            &data[1..]
        } else {
            data
        };

        if payload.is_empty() {
            return OutputReportResult::Unhandled;
        }

        let cmd = payload[0];

        match cmd {
            IMAGE_COMMAND_V2 => {
                let Some((_, key_id, is_last, psize, seq, start)) =
                    Self::parse_standard_image_chunk(payload)
                else {
                    return OutputReportResult::Unhandled;
                };
                self.ingest_chunk(IngestChunkParams {
                    kind: V2ImageKind::Key,
                    slot: key_id,
                    partial: (0, 0, 0, 0),
                    sequence: seq,
                    is_last,
                    payload_len: psize as usize,
                    data_start: start,
                    data: payload,
                })
            }
            0x08 => {
                let Some((_, _res, is_last, psize, seq, start)) =
                    Self::parse_standard_image_chunk(payload)
                else {
                    return OutputReportResult::Unhandled;
                };
                self.ingest_chunk(IngestChunkParams {
                    kind: V2ImageKind::FullScreen,
                    slot: 0,
                    partial: (0, 0, 0, 0),
                    sequence: seq,
                    is_last,
                    payload_len: psize as usize,
                    data_start: start,
                    data: payload,
                })
            }
            0x0B if self.device.supports_window_image_commands() => {
                let Some((_, _res, is_last, psize, seq, start)) =
                    Self::parse_standard_image_chunk(payload)
                else {
                    return OutputReportResult::Unhandled;
                };
                self.ingest_chunk(IngestChunkParams {
                    kind: V2ImageKind::Window,
                    slot: 0,
                    partial: (0, 0, 0, 0),
                    sequence: seq,
                    is_last,
                    payload_len: psize as usize,
                    data_start: start,
                    data: payload,
                })
            }
            0x0C if self.device.supports_window_image_commands() => {
                let Some(PartialWindowChunk {
                    x,
                    y,
                    w,
                    h,
                    is_last,
                    chunk_index,
                    chunk_size,
                    data_start: start,
                }) = Self::parse_partial_chunk(payload)
                else {
                    return OutputReportResult::Unhandled;
                };
                self.ingest_chunk(IngestChunkParams {
                    kind: V2ImageKind::PartialWindow,
                    slot: 0,
                    partial: (x, y, w, h),
                    sequence: chunk_index,
                    is_last,
                    payload_len: chunk_size as usize,
                    data_start: start,
                    data: payload,
                })
            }
            0x0D if self.device.supports_background_feature() => {
                let Some((bg_index, is_last, seq, psize, start)) =
                    Self::parse_background_chunk(payload)
                else {
                    return OutputReportResult::Unhandled;
                };
                self.ingest_chunk(IngestChunkParams {
                    kind: V2ImageKind::Background,
                    slot: bg_index,
                    partial: (0, 0, 0, 0),
                    sequence: seq,
                    is_last,
                    payload_len: psize as usize,
                    data_start: start,
                    data: payload,
                })
            }
            0x09 => OutputReportResult::BootLogoImageChunk,
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
        map_buttons_grid(physical_buttons, cols, rows, left_to_right)
    }

    fn hid_descriptor(&self) -> &'static [u8] {
        const DESC: &[u8] = &[
            0x05, 0x0c, 0x09, 0x01, 0xa1, 0x01, 0x09, 0x01, 0x05, 0x09, 0x19, 0x01, 0x29, 0x30,
            0x15, 0x00, 0x26, 0xff, 0x00, 0x75, 0x08, 0x95, 0x30, 0x85, 0x01, 0x81, 0x02, 0x0a,
            0x00, 0xff, 0x15, 0x00, 0x26, 0xff, 0x00, 0x75, 0x08, 0x96, 0x00, 0x04, 0x85, 0x02,
            0x91, 0x02, 0x0a, 0x00, 0xff, 0x15, 0x00, 0x26, 0xff, 0x00, 0x75, 0x08, 0x95, 0x20,
            0x85, 0x03, 0xb1, 0x04, 0x0a, 0x00, 0xff, 0x15, 0x00, 0x26, 0xff, 0x00, 0x75, 0x08,
            0x95, 0x20, 0x85, 0x04, 0xb1, 0x04, 0x0a, 0x00, 0xff, 0x15, 0x00, 0x26, 0xff, 0x00,
            0x75, 0x08, 0x95, 0x20, 0x85, 0x05, 0xb1, 0x04, 0x0a, 0x00, 0xff, 0x15, 0x00, 0x26,
            0xff, 0x00, 0x75, 0x08, 0x95, 0x20, 0x85, 0x06, 0xb1, 0x04, 0x0a, 0x00, 0xff, 0x15,
            0x00, 0x26, 0xff, 0x00, 0x75, 0x08, 0x95, 0x20, 0x85, 0x07, 0xb1, 0x04, 0x0a, 0x00,
            0xff, 0x15, 0x00, 0x26, 0xff, 0x00, 0x75, 0x08, 0x95, 0x20, 0x85, 0x08, 0xb1, 0x04,
            0x0a, 0x00, 0xff, 0x15, 0x00, 0x26, 0xff, 0x00, 0x75, 0x08, 0x95, 0x20, 0x85, 0x0a,
            0xb1, 0x04, 0xc0,
        ];
        DESC
    }

    fn input_report_size(&self, button_count: usize) -> usize {
        // Report ID 0x01 + Command + UINT16 length + payload (per General Reference)
        1 + 1 + 2 + button_count
    }

    fn format_button_report(&self, buttons: &ButtonMapping, report: &mut [u8]) -> usize {
        let n = buttons
            .active_count
            .min(MAX_BUTTON_SLOTS)
            .min(report.len().saturating_sub(4));
        if report.len() < 4 + n {
            return 0;
        }
        report[0] = 0x01;
        report[1] = 0x00;
        let len = n as u16;
        report[2] = (len & 0xff) as u8;
        report[3] = ((len >> 8) & 0xff) as u8;
        for i in 0..n {
            report[4 + i] = if buttons.mapped_buttons[i] { 1 } else { 0 };
        }
        for b in report.iter_mut().skip(4 + n) {
            *b = 0;
        }
        4 + n
    }

    fn handle_feature_report(&mut self, report_id: u8, data: &[u8]) -> Option<ModuleSetCommand> {
        if report_id == 0x03 {
            let cmd = Self::feature_command(data, 0x03)?;
            match cmd {
                V2_COMMAND_RESET => Some(ModuleSetCommand::Reset),
                0x05 if data.len() >= 5 => Some(ModuleSetCommand::FillLcdColor {
                    r: data[2],
                    g: data[3],
                    b: data[4],
                }),
                0x06 if data.len() >= 6 => Some(ModuleSetCommand::SetKeyColor {
                    key_index: data[2],
                    r: data[3],
                    g: data[4],
                    b: data[5],
                }),
                V2_COMMAND_BRIGHTNESS if data.len() >= 3 => {
                    Some(ModuleSetCommand::SetBrightness { value: data[2] })
                }
                0x0D if data.len() >= 6 => {
                    let secs = i32::from_le_bytes([data[2], data[3], data[4], data[5]]);
                    Some(ModuleSetCommand::SetIdleTime { seconds: secs })
                }
                0x13 if self.device.supports_background_feature() && data.len() >= 3 => {
                    Some(ModuleSetCommand::ShowBackgroundByIndex { index: data[2] })
                }
                _ => None,
            }
        } else {
            None
        }
    }

    fn get_feature_report(&mut self, report_id: u8, buf: &mut [u8]) -> Option<usize> {
        const FW_VER: &[u8] = b"3.00.000";
        let total_len = 32.min(buf.len());
        match report_id {
            0x04 | 0x05 | 0x07 => {
                fill_feature_v2_fw_version_report(buf, report_id, total_len, FW_VER)
            }
            0x06 => {
                let cap = feature_report_clamp(total_len, buf.len());
                if cap == 0 {
                    return None;
                }
                feature_report_zero_prefix(buf, cap);
                buf[0] = 0x06;
                let serial = crate::config::usb_serial_bytes();
                let dl = core::cmp::min(serial.len(), 14) as u8;
                buf[1] = dl;
                let start = 2usize;
                let end = (start + dl as usize).min(cap);
                buf[start..end].copy_from_slice(&serial[..(end - start)]);
                Some(cap)
            }
            0x08 => {
                let cap = feature_report_clamp(total_len, buf.len());
                if cap == 0 {
                    return None;
                }
                feature_report_zero_prefix(buf, cap);
                buf[0] = 0x08;
                let tail = self.device.unit_information_tail();
                let copy = core::cmp::min(tail.len(), cap.saturating_sub(1));
                buf[1..1 + copy].copy_from_slice(&tail[..copy]);
                Some(cap)
            }
            0x0A => {
                let cap = feature_report_clamp(total_len, buf.len());
                if cap == 0 {
                    return None;
                }
                feature_report_zero_prefix(buf, cap);
                buf[0] = 0x0A;
                buf[1] = 0x04;
                let seconds = crate::config::get_idle_time_seconds();
                let le = seconds.to_le_bytes();
                buf[2..6].copy_from_slice(&le);
                Some(cap)
            }
            _ => None,
        }
    }
}

impl V2Handler {
    /// Main protocol input: touch **TAP** (command `0x02`, payload length `0x0A` per Elgato + / + XL docs).
    /// Writes report ID `0x01` at `[0]`, then command, length, and fields.
    pub fn format_input_touch_tap(x: u16, y: u16, out: &mut [u8]) -> usize {
        if out.len() < 10 {
            return 0;
        }
        out[0] = 0x01;
        out[1] = 0x02;
        out[2] = 0x0a;
        out[3] = 0x00;
        out[4] = 0x01;
        out[5] = 0x00;
        out[6..8].copy_from_slice(&x.to_le_bytes());
        out[8..10].copy_from_slice(&y.to_le_bytes());
        10
    }

    /// Main protocol input: encoder **ROTATE** (`0x03`, sub-type `0x01`, `ticks` i8 per encoder).
    pub fn format_input_encoder_rotate(ticks: &[i8], out: &mut [u8]) -> usize {
        let n = ticks.len().min(8);
        let need = 5 + n;
        if out.len() < need {
            return 0;
        }
        let plen = (n + 1) as u16;
        out[0] = 0x01;
        out[1] = 0x03;
        out[2] = (plen & 0xff) as u8;
        out[3] = ((plen >> 8) & 0xff) as u8;
        out[4] = 0x01;
        for (i, &t) in ticks.iter().take(n).enumerate() {
            out[5 + i] = t as u8;
        }
        need
    }
}
