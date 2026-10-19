//! Gen2 (Main / Expanded protocol) report handling.
//!
//! Layouts follow `node-elgato-stream-deck` `packages/core`:
//! - input reports: `[0x01, cmd, len16, payload...]`
//! - output reports: `[0x02, cmd, ...]`
//! - feature reports: `[0x03, cmd, ...]`
//!
//! The offsets here are for the *stripped* report (report id removed), which is
//! what a HID read returns to the protocol layer.

use crate::feature::{build_feature, FeatureStyle, Identity};
use crate::hid_descriptor::descriptor_for;
use crate::image::{ChunkInfo, ImageAssembler, ImageKind, Ingest};
use crate::input::{InputEvent, TouchKind};
use crate::model::{ModelInfo, MAX_ENCODERS, MAX_KEYS};
use crate::protocol::{OutputSink, ProtocolError};

const RID_INPUT: u8 = 0x01;
const RID_OUTPUT: u8 = 0x02;
const RID_FEATURE: u8 = 0x03;

/// Gen2 protocol handler for a fixed model.
#[derive(Debug)]
pub struct Gen2Handler<const IMG: usize> {
    model: &'static ModelInfo,
    assembler: ImageAssembler<IMG>,
    buttons: [u8; MAX_KEYS],
    button_count: u8,
    encoders: [u8; MAX_ENCODERS],
    encoder_count: u8,
}

impl<const IMG: usize> Gen2Handler<IMG> {
    pub const fn new(model: &'static ModelInfo) -> Self {
        Self {
            model,
            assembler: ImageAssembler::new(),
            buttons: [0; MAX_KEYS],
            button_count: model.key_count() as u8,
            encoders: [0; MAX_ENCODERS],
            encoder_count: model.encoder_count,
        }
    }

    pub fn model(&self) -> &'static ModelInfo {
        self.model
    }

    pub fn hid_descriptor(&self) -> &'static [u8] {
        descriptor_for(self.model)
    }

    /// Current button state bytes (`0`/`1` per key).
    pub fn button_state(&self) -> &[u8] {
        &self.buttons[..self.button_count as usize]
    }

    /// Parse a host output report and emit commands to `sink`.
    pub fn handle_output(
        &mut self,
        report: &[u8],
        sink: &mut dyn OutputSink,
    ) -> Result<(), ProtocolError> {
        let report = strip(report, RID_OUTPUT);
        let cmd = *report.first().ok_or(ProtocolError::Short)?;

        match cmd {
            0x07 => {
                let (slot, is_last, size, seq, data_start) = parse_standard(report)?;
                let end = data_start
                    .checked_add(size as usize)
                    .ok_or(ProtocolError::Short)?;
                if end > report.len() {
                    return Err(ProtocolError::Short);
                }
                let info = ChunkInfo {
                    kind: ImageKind::Key,
                    slot,
                    x: 0,
                    y: 0,
                    w: 0,
                    h: 0,
                    sequence: seq,
                    is_last,
                };
                self.ingest_and_emit(info, &report[data_start..end], sink, |s, jpeg| {
                    s.key_image(slot, jpeg)
                })
            }
            0x08 => {
                let (_slot, is_last, size, seq, data_start) = parse_standard(report)?;
                let end = data_start
                    .checked_add(size as usize)
                    .ok_or(ProtocolError::Short)?;
                if end > report.len() {
                    return Err(ProtocolError::Short);
                }
                let info = ChunkInfo {
                    kind: ImageKind::FullScreen,
                    slot: 0,
                    x: 0,
                    y: 0,
                    w: 0,
                    h: 0,
                    sequence: seq,
                    is_last,
                };
                self.ingest_and_emit(info, &report[data_start..end], sink, |s, jpeg| {
                    s.full_screen(0, jpeg)
                })
            }
            0x0b => {
                let (_slot, is_last, size, seq, data_start) = parse_standard(report)?;
                let end = data_start
                    .checked_add(size as usize)
                    .ok_or(ProtocolError::Short)?;
                if end > report.len() {
                    return Err(ProtocolError::Short);
                }
                let info = ChunkInfo {
                    kind: ImageKind::Window,
                    slot: 0,
                    x: 0,
                    y: 0,
                    w: 0,
                    h: 0,
                    sequence: seq,
                    is_last,
                };
                self.ingest_and_emit(info, &report[data_start..end], sink, |s, jpeg| {
                    s.window(jpeg)
                })
            }
            0x0c => {
                let p = parse_partial(report)?;
                let end = p
                    .data_start
                    .checked_add(p.size as usize)
                    .ok_or(ProtocolError::Short)?;
                if end > report.len() {
                    return Err(ProtocolError::Short);
                }
                let info = ChunkInfo {
                    kind: ImageKind::Panel,
                    slot: 0,
                    x: p.x,
                    y: p.y,
                    w: p.w,
                    h: p.h,
                    sequence: p.sequence,
                    is_last: p.is_last,
                };
                self.ingest_and_emit(info, &report[p.data_start..end], sink, |s, jpeg| {
                    s.panel_image(p.x, p.y, p.w, p.h, jpeg)
                })
            }
            0x0d => {
                let (slot, is_last, size, seq, data_start) = parse_standard(report)?;
                let end = data_start
                    .checked_add(size as usize)
                    .ok_or(ProtocolError::Short)?;
                if end > report.len() {
                    return Err(ProtocolError::Short);
                }
                let info = ChunkInfo {
                    kind: ImageKind::Background,
                    slot,
                    x: 0,
                    y: 0,
                    w: 0,
                    h: 0,
                    sequence: seq,
                    is_last,
                };
                self.ingest_and_emit(info, &report[data_start..end], sink, |s, jpeg| {
                    s.background(slot, jpeg)
                })
            }
            0x0f => {
                // encoder ring: [cmd, enc, 24 * RGB]
                let enc = *report.get(1).ok_or(ProtocolError::Short)?;
                if enc >= self.encoder_count {
                    return Err(ProtocolError::BadEncoder(enc));
                }
                let rgb = report.get(2..2 + 72).ok_or(ProtocolError::Short)?;
                sink.encoder_ring(enc, rgb);
                Ok(())
            }
            0x10 => {
                let enc = *report.get(1).ok_or(ProtocolError::Short)?;
                if enc >= self.encoder_count {
                    return Err(ProtocolError::BadEncoder(enc));
                }
                let r = *report.get(2).ok_or(ProtocolError::Short)?;
                let g = *report.get(3).ok_or(ProtocolError::Short)?;
                let b = *report.get(4).ok_or(ProtocolError::Short)?;
                sink.encoder_led(enc, r, g, b);
                Ok(())
            }
            0x09 => Ok(()), // boot logo chunk
            _ => Ok(()),
        }
    }

    fn ingest_and_emit<F>(
        &mut self,
        info: ChunkInfo,
        chunk: &[u8],
        sink: &mut dyn OutputSink,
        emit: F,
    ) -> Result<(), ProtocolError>
    where
        F: FnOnce(&mut dyn OutputSink, &[u8]),
    {
        match self.assembler.ingest(info, chunk)? {
            Ingest::Complete => {
                emit(sink, self.assembler.bytes());
                self.assembler.reset();
            }
            Ingest::InProgress | Ingest::Ignored => {}
        }
        Ok(())
    }

    /// Handle a feature report set (`0x03` container or raw command).
    pub fn handle_set_feature(
        &mut self,
        report: &[u8],
        sink: &mut dyn OutputSink,
    ) -> Result<(), ProtocolError> {
        let report = strip(report, RID_FEATURE);
        let cmd = *report.first().ok_or(ProtocolError::Short)?;
        match cmd {
            0x02 => {
                sink.reset();
                Ok(())
            }
            0x05 => {
                if self.model.fullscreen_panels > 0 {
                    let panel = *report.get(1).unwrap_or(&0);
                    sink.clear_panel(panel);
                } else {
                    let r = *report.get(1).unwrap_or(&0);
                    let g = *report.get(2).unwrap_or(&0);
                    let b = *report.get(3).unwrap_or(&0);
                    sink.fill_lcd(r, g, b);
                }
                Ok(())
            }
            0x06 => {
                let key = *report.get(1).ok_or(ProtocolError::Short)?;
                let r = *report.get(2).ok_or(ProtocolError::Short)?;
                let g = *report.get(3).ok_or(ProtocolError::Short)?;
                let b = *report.get(4).ok_or(ProtocolError::Short)?;
                sink.key_rgb(key, r, g, b);
                Ok(())
            }
            0x08 => {
                let value = *report.get(1).ok_or(ProtocolError::Short)?;
                sink.brightness(value);
                Ok(())
            }
            0x0d => {
                if report.len() < 5 {
                    return Err(ProtocolError::Short);
                }
                let seconds = i32::from_le_bytes([report[1], report[2], report[3], report[4]]);
                sink.sleep(seconds);
                Ok(())
            }
            _ => Ok(()),
        }
    }

    /// Build a feature GET response.
    pub fn get_feature(
        &mut self,
        report_id: u8,
        style: FeatureStyle,
        identity: &Identity<'_>,
        out: &mut [u8],
    ) -> Result<usize, ProtocolError> {
        Ok(build_feature(style, report_id, identity, out)?)
    }

    /// Encode an input event into a complete HID input report.
    pub fn encode_input(
        &mut self,
        event: InputEvent,
        out: &mut [u8],
    ) -> Result<usize, ProtocolError> {
        match event {
            InputEvent::Button { index, pressed } => {
                if index >= self.button_count {
                    return Err(ProtocolError::BadKey(index));
                }
                self.buttons[index as usize] = u8::from(pressed);
                let n = self.button_count as usize;
                if out.len() < 4 + n {
                    return Err(ProtocolError::BufferFull);
                }
                out[0] = RID_INPUT;
                out[1] = 0x00;
                out[2..4].copy_from_slice(&(n as u16).to_le_bytes());
                out[4..4 + n].copy_from_slice(&self.buttons[..n]);
                Ok(4 + n)
            }
            InputEvent::EncoderPress { index, pressed } => {
                if index >= self.encoder_count {
                    return Err(ProtocolError::BadEncoder(index));
                }
                self.encoders[index as usize] = u8::from(pressed);
                let n = self.encoder_count as usize;
                self.encode_encoder(0x00, None, out, n)
            }
            InputEvent::EncoderRotate { index, delta } => {
                if index >= self.encoder_count {
                    return Err(ProtocolError::BadEncoder(index));
                }
                let n = self.encoder_count as usize;
                self.encode_encoder(0x01, Some((index, delta)), out, n)
            }
            InputEvent::Touch { kind, x, y, x2, y2 } => {
                let (type_byte, len) = match kind {
                    TouchKind::Tap => (0x01u8, 0x0au16),
                    TouchKind::Press => (0x02u8, 0x0au16),
                    TouchKind::Swipe => (0x03u8, 0x0eu16),
                };
                let total = 4 + len as usize;
                if out.len() < total {
                    return Err(ProtocolError::BufferFull);
                }
                out.fill(0);
                out[0] = RID_INPUT;
                out[1] = 0x02;
                out[2..4].copy_from_slice(&len.to_le_bytes());
                out[4] = type_byte;
                out[6..8].copy_from_slice(&x.to_le_bytes());
                out[8..10].copy_from_slice(&y.to_le_bytes());
                if kind == TouchKind::Swipe {
                    out[10..12].copy_from_slice(&x2.to_le_bytes());
                    out[12..14].copy_from_slice(&y2.to_le_bytes());
                }
                Ok(total)
            }
            InputEvent::Nfc { uid, len } => {
                let len = (len as usize).min(uid.len());
                let total = 4 + len;
                if out.len() < total {
                    return Err(ProtocolError::BufferFull);
                }
                out[0] = RID_INPUT;
                out[1] = 0x04;
                out[2..4].copy_from_slice(&(len as u16).to_le_bytes());
                out[4..4 + len].copy_from_slice(&uid[..len]);
                Ok(total)
            }
        }
    }

    fn encode_encoder(
        &self,
        subtype: u8,
        rotate: Option<(u8, i8)>,
        out: &mut [u8],
        n: usize,
    ) -> Result<usize, ProtocolError> {
        let total = 5 + n;
        if out.len() < total {
            return Err(ProtocolError::BufferFull);
        }
        out[0] = RID_INPUT;
        out[1] = 0x03;
        let payload_len = (n + 1) as u16;
        out[2..4].copy_from_slice(&payload_len.to_le_bytes());
        out[4] = subtype;
        for (i, slot) in out[5..5 + n].iter_mut().enumerate() {
            *slot = match rotate {
                Some((index, delta)) if index as usize == i => delta as u8,
                Some(_) => 0,
                None => self.encoders[i],
            };
        }
        Ok(total)
    }
}

/// Strip an optional report id from a report.
fn strip(report: &[u8], rid: u8) -> &[u8] {
    if report.first() == Some(&rid) {
        &report[1..]
    } else {
        report
    }
}

/// Standard image chunk header: `[cmd, slot, last, size16, index16, data...]`.
fn parse_standard(report: &[u8]) -> Result<(u8, bool, u16, u16, usize), ProtocolError> {
    if report.len() < 8 {
        return Err(ProtocolError::Short);
    }
    let slot = report[1];
    let is_last = report[2] != 0;
    let size = u16::from_le_bytes([report[3], report[4]]);
    let sequence = u16::from_le_bytes([report[5], report[6]]);
    Ok((slot, is_last, size, sequence, 7))
}

/// Partial-window (`0x0c`) header.
struct Partial {
    x: u16,
    y: u16,
    w: u16,
    h: u16,
    is_last: bool,
    sequence: u16,
    size: u16,
    data_start: usize,
}

fn parse_partial(report: &[u8]) -> Result<Partial, ProtocolError> {
    if report.len() < 15 {
        return Err(ProtocolError::Short);
    }
    Ok(Partial {
        x: u16::from_le_bytes([report[1], report[2]]),
        y: u16::from_le_bytes([report[3], report[4]]),
        w: u16::from_le_bytes([report[5], report[6]]),
        h: u16::from_le_bytes([report[7], report[8]]),
        is_last: report[9] != 0,
        sequence: u16::from_le_bytes([report[10], report[11]]),
        size: u16::from_le_bytes([report[12], report[13]]),
        data_start: 15,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model;

    #[derive(Default)]
    struct Recorder {
        key: Option<(u8, usize)>,
        panel: Option<(u16, u16, u16, u16, usize)>,
        ring: Option<(u8, usize)>,
        led: Option<(u8, u8, u8, u8)>,
        brightness: Option<u8>,
        clear_panel: Option<u8>,
        reset: bool,
    }

    impl OutputSink for Recorder {
        fn key_image(&mut self, key: u8, jpeg: &[u8]) {
            self.key = Some((key, jpeg.len()));
        }
        fn panel_image(&mut self, x: u16, y: u16, w: u16, h: u16, jpeg: &[u8]) {
            self.panel = Some((x, y, w, h, jpeg.len()));
        }
        fn encoder_ring(&mut self, enc: u8, rgb: &[u8]) {
            self.ring = Some((enc, rgb.len()));
        }
        fn encoder_led(&mut self, enc: u8, r: u8, g: u8, b: u8) {
            self.led = Some((enc, r, g, b));
        }
        fn brightness(&mut self, percent: u8) {
            self.brightness = Some(percent);
        }
        fn clear_panel(&mut self, panel: u8) {
            self.clear_panel = Some(panel);
        }
        fn reset(&mut self) {
            self.reset = true;
        }
    }

    #[test]
    fn key_image_chunk_is_assembled() {
        let mut h = Gen2Handler::<64>::new(&model::STUDIO);
        let mut sink = Recorder::default();
        // report: [0x02, 0x07, key, last, size, idx_lo, idx_hi, data...]
        let mut report = [0u8; 16];
        report[0] = 0x02;
        report[1] = 0x07;
        report[2] = 5;
        report[3] = 1;
        report[4..6].copy_from_slice(&3u16.to_le_bytes());
        report[6..8].copy_from_slice(&0u16.to_le_bytes());
        report[8..11].copy_from_slice(b"JPX");
        h.handle_output(&report, &mut sink).unwrap();
        assert_eq!(sink.key, Some((5, 3)));
    }

    #[test]
    fn partial_window_uses_node_offsets() {
        let mut h = Gen2Handler::<64>::new(&model::STUDIO);
        let mut sink = Recorder::default();
        // stripped layout: [cmd, x, y, w, h, last, idx, size, pad, data...]
        let mut report = [0u8; 18];
        report[0] = 0x0c;
        report[1..3].copy_from_slice(&0u16.to_le_bytes());
        report[3..5].copy_from_slice(&0u16.to_le_bytes());
        report[5..7].copy_from_slice(&2304u16.to_le_bytes());
        report[7..9].copy_from_slice(&112u16.to_le_bytes());
        report[9] = 1;
        report[12..14].copy_from_slice(&2u16.to_le_bytes());
        report[15..17].copy_from_slice(b"AB");
        h.handle_output(&report, &mut sink).unwrap();
        assert_eq!(sink.panel, Some((0, 0, 2304, 112, 2)));
    }

    #[test]
    fn encoder_ring_and_led() {
        let mut h = Gen2Handler::<64>::new(&model::STUDIO);
        let mut sink = Recorder::default();
        let mut ring = [0u8; 2 + 72];
        ring[0] = 0x0f;
        ring[1] = 1;
        h.handle_output(&ring, &mut sink).unwrap();
        assert_eq!(sink.ring, Some((1, 72)));

        let led = [0x10u8, 0, 10, 20, 30];
        h.handle_output(&led, &mut sink).unwrap();
        assert_eq!(sink.led, Some((0, 10, 20, 30)));
    }

    #[test]
    fn brightness_and_panel_clear() {
        let mut h = Gen2Handler::<64>::new(&model::STUDIO);
        let mut sink = Recorder::default();
        h.handle_set_feature(&[0x03, 0x08, 0x64], &mut sink)
            .unwrap();
        assert_eq!(sink.brightness, Some(100));
        h.handle_set_feature(&[0x03, 0x05, 0x01, 0, 0, 0], &mut sink)
            .unwrap();
        assert_eq!(sink.clear_panel, Some(1));
    }

    #[test]
    fn button_and_encoder_input() {
        let mut h = Gen2Handler::<64>::new(&model::STUDIO);
        let mut out = [0u8; 64];
        let n = h
            .encode_input(
                InputEvent::Button {
                    index: 31,
                    pressed: true,
                },
                &mut out,
            )
            .unwrap();
        assert_eq!(n, 4 + 32);
        assert_eq!(out[0], 0x01);
        assert_eq!(out[1], 0x00);
        assert_eq!(out[4 + 31], 1);

        let n = h
            .encode_input(
                InputEvent::EncoderRotate {
                    index: 1,
                    delta: -2,
                },
                &mut out,
            )
            .unwrap();
        assert_eq!(n, 5 + 2);
        assert_eq!(out[1], 0x03);
        assert_eq!(out[4], 0x01);
        assert_eq!(out[5 + 1] as i8, -2);
    }

    #[test]
    fn nfc_input() {
        let mut h = Gen2Handler::<64>::new(&model::STUDIO);
        let mut out = [0u8; 64];
        let n = h
            .encode_input(InputEvent::nfc(&[1, 2, 3, 4]), &mut out)
            .unwrap();
        assert_eq!(n, 4 + 4);
        assert_eq!(&out[2..4], &4u16.to_le_bytes());
        assert_eq!(&out[4..8], &[1, 2, 3, 4]);
    }
}
