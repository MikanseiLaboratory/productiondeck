//! Feature-report builders.
//!
//! Three wire styles exist:
//! - [`FeatureStyle::Usb`]: 32-byte HID feature reports, report id at byte 0.
//! - [`FeatureStyle::CoraPrimary`]: Cora primary-port payloads, normally
//!   prefixed with `0x03`, up to 1024 bytes.
//! - [`FeatureStyle::CoraChild`]: verbatim HID feature reports for a child
//!   device (32 bytes, report id at byte 0).

use crate::model::{ModelId, ModelInfo, MAX_PANELS};

/// Firmware version strings reported to the host.
#[derive(Debug, Clone, Copy)]
pub struct FirmwareVersions<'a> {
    /// Main AP2 firmware (USB 0x05 / Cora 0x83).
    pub ap2: &'a str,
    /// AP1 firmware (USB 0x07).
    pub ap1: &'a str,
    /// LD firmware (USB 0x04).
    pub ld: &'a str,
    /// Encoder AP2 firmware (USB 0x11 / Cora 0x86).
    pub encoder_ap2: &'a str,
    /// Encoder LD firmware (USB 0x13 / Cora 0x8A).
    pub encoder_ld: &'a str,
}

impl Default for FirmwareVersions<'static> {
    fn default() -> Self {
        Self {
            ap2: "1.00.000",
            ap1: "1.00.000",
            ld: "1.00.000",
            encoder_ap2: "1.00.000",
            encoder_ld: "1.00.000",
        }
    }
}

/// Child device attached to a primary device (Cora 0x1C).
#[derive(Debug, Clone, Copy)]
pub struct ChildInfo<'a> {
    pub connected: bool,
    pub vid: u16,
    pub pid: u16,
    pub serial: &'a str,
    pub port: u16,
    pub model_name: &'a str,
    pub key_w: u16,
    pub key_h: u16,
    pub rows: u8,
    pub cols: u8,
    /// Layout/type byte placed at capabilities offset 4.
    pub layout_type: u8,
}

/// Identity used to build feature responses.
#[derive(Debug, Clone, Copy)]
pub struct Identity<'a> {
    pub model: &'static ModelInfo,
    pub serial: &'a str,
    pub mac: [u8; 6],
    pub versions: FirmwareVersions<'a>,
    pub child: Option<ChildInfo<'a>>,
}

impl<'a> Identity<'a> {
    pub const fn new(model: &'static ModelInfo, serial: &'a str) -> Self {
        Self {
            model,
            serial,
            mac: [0; 6],
            versions: FirmwareVersions {
                ap2: "1.00.000",
                ap1: "1.00.000",
                ld: "1.00.000",
                encoder_ap2: "1.00.000",
                encoder_ld: "1.00.000",
            },
            child: None,
        }
    }
}

/// Wire style for a feature response.
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FeatureStyle {
    /// USB HID feature report (32 bytes).
    Usb,
    /// Cora primary port (`0x03` prefix, 512/1024 bytes).
    CoraPrimary,
    /// Cora child HID report (32 bytes, no `0x03` prefix).
    CoraChild,
}

/// Feature build errors.
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FeatureError {
    /// Unknown / unsupported report id for this style and model.
    Unsupported,
    /// The caller-provided output buffer is too small.
    BufferFull,
}

fn put_ascii(dst: &mut [u8], offset: usize, value: &str) -> usize {
    let bytes = value.as_bytes();
    let end = (offset + bytes.len()).min(dst.len());
    if end > offset {
        dst[offset..end].copy_from_slice(&bytes[..end - offset]);
    }
    end - offset
}

fn put_u16_le(dst: &mut [u8], offset: usize, value: u16) {
    if offset + 2 <= dst.len() {
        dst[offset..offset + 2].copy_from_slice(&value.to_le_bytes());
    }
}

fn fw_report(report_id: u8, version: &str, out: &mut [u8]) -> Result<usize, FeatureError> {
    if out.len() < 14 {
        return Err(FeatureError::BufferFull);
    }
    out.fill(0);
    out[0] = report_id;
    out[1] = 0x0c;
    put_ascii(out, 6, version);
    Ok(14)
}

fn serial_report(report_id: u8, serial: &str, out: &mut [u8]) -> Result<usize, FeatureError> {
    if out.len() < 2 {
        return Err(FeatureError::BufferFull);
    }
    out.fill(0);
    out[0] = report_id;
    let max = out.len() - 2;
    let bytes = serial.as_bytes();
    let len = bytes.len().min(max).min(14);
    out[1] = len as u8;
    out[2..2 + len].copy_from_slice(&bytes[..len]);
    Ok(2 + len)
}

fn unit_info_report(model: &ModelInfo, out: &mut [u8]) -> Result<usize, FeatureError> {
    if out.len() < 17 {
        return Err(FeatureError::BufferFull);
    }
    out.fill(0);
    out[0] = 0x08;
    out[1] = model.rows;
    out[2] = model.cols;
    put_u16_le(out, 3, model.key_w);
    put_u16_le(out, 5, model.key_h);
    put_u16_le(out, 7, model.lcd_w);
    put_u16_le(out, 9, model.lcd_h);
    if model.id == ModelId::Studio {
        // Captured Studio 0x08 response:
        // 08 02 10 90 00 70 00 90 05 18 01 02 00 00 00 04 ff
        out[11] = 2;
        out[15] = 0x04;
        out[16] = 0xff;
    } else {
        out[11] = 24; // bits per pixel
    }
    Ok(17)
}

/// Captured Studio firmware checksums (u32 LE at the report offset).
fn studio_checksum(version: &str) -> [u8; 4] {
    match version {
        "2.01.002" => [0xa2, 0xcc, 0x09, 0xb5],
        "1.01.012" => [0x93, 0x7c, 0x27, 0x00],
        "1.01.006" => [0x7a, 0x4d, 0x19, 0x00],
        "1.00.000" => [0x5c, 0xeb, 0x15, 0x00],
        _ => [0; 4],
    }
}

/// Studio AP2/LD firmware report: `[id, 0x0c, checksum, version @6]`.
fn studio_fw_report(report_id: u8, version: &str, out: &mut [u8]) -> Result<usize, FeatureError> {
    if out.len() < 14 {
        return Err(FeatureError::BufferFull);
    }
    out.fill(0);
    out[0] = report_id;
    out[1] = 0x0c;
    out[2..6].copy_from_slice(&studio_checksum(version));
    put_ascii(out, 6, version);
    Ok(14)
}

/// Studio 0x1A report: `[0x1a, 0x0c, version @2, checksum @10]`.
fn studio_1a_report(version: &str, out: &mut [u8]) -> Result<usize, FeatureError> {
    if out.len() < 14 {
        return Err(FeatureError::BufferFull);
    }
    out.fill(0);
    out[0] = 0x1a;
    out[1] = 0x0c;
    put_ascii(out, 2, version);
    out[10..14].copy_from_slice(&studio_checksum(version));
    Ok(14)
}

/// Studio encoder firmware report: two copies of `version + checksum`.
/// Captured 0x11: `11 18 <ver8> <sum4> <ver8> <sum4>`.
fn studio_dual_fw_report(
    report_id: u8,
    version: &str,
    out: &mut [u8],
) -> Result<usize, FeatureError> {
    if out.len() < 26 {
        return Err(FeatureError::BufferFull);
    }
    out.fill(0);
    out[0] = report_id;
    out[1] = 0x18;
    let checksum = studio_checksum(version);
    put_ascii(out, 2, version);
    out[10..14].copy_from_slice(&checksum);
    put_ascii(out, 14, version);
    out[22..26].copy_from_slice(&checksum);
    Ok(26)
}

fn sleep_report(seconds: i32, out: &mut [u8]) -> Result<usize, FeatureError> {
    if out.len() < 6 {
        return Err(FeatureError::BufferFull);
    }
    out.fill(0);
    out[0] = 0x0a;
    out[1] = 4;
    out[2..6].copy_from_slice(&seconds.to_le_bytes());
    Ok(6)
}

/// Build a feature response for the requested style.
///
/// Returns the number of meaningful bytes written to `out`. Bytes past the
/// returned length are zero padding expected by the transport.
pub fn build_feature(
    style: FeatureStyle,
    report_id: u8,
    identity: &Identity<'_>,
    out: &mut [u8],
) -> Result<usize, FeatureError> {
    match style {
        FeatureStyle::Usb | FeatureStyle::CoraChild => build_usb_like(report_id, identity, out),
        FeatureStyle::CoraPrimary => build_cora_primary(report_id, identity, out),
    }
}

fn build_usb_like(
    report_id: u8,
    identity: &Identity<'_>,
    out: &mut [u8],
) -> Result<usize, FeatureError> {
    let versions = &identity.versions;
    if identity.model.id == ModelId::Studio {
        return match report_id {
            0x05 => studio_fw_report(0x05, versions.ap2, out),
            0x06 => serial_report(0x06, identity.serial, out),
            0x08 => unit_info_report(identity.model, out),
            0x11 => studio_dual_fw_report(0x11, versions.encoder_ap2, out),
            0x13 => studio_dual_fw_report(0x13, versions.encoder_ld, out),
            0x1a => studio_1a_report(versions.ap1, out),
            0x04 => studio_fw_report(0x04, versions.ld, out),
            0x07 => studio_fw_report(0x07, versions.ap1, out),
            0x0a => sleep_report(0, out),
            _ => Err(FeatureError::Unsupported),
        };
    }
    match report_id {
        0x04 => fw_report(0x04, versions.ld, out),
        0x05 => fw_report(0x05, versions.ap2, out),
        0x06 => serial_report(0x06, identity.serial, out),
        0x07 => fw_report(0x07, versions.ap1, out),
        0x08 => unit_info_report(identity.model, out),
        0x0a => sleep_report(0, out),
        0x11 => fw_report(0x11, versions.encoder_ap2, out),
        0x13 => fw_report(0x13, versions.encoder_ld, out),
        _ => Err(FeatureError::Unsupported),
    }
}

fn build_cora_primary(
    report_id: u8,
    identity: &Identity<'_>,
    out: &mut [u8],
) -> Result<usize, FeatureError> {
    if out.len() < 16 {
        return Err(FeatureError::BufferFull);
    }
    out.fill(0);
    out[0] = 0x03;
    out[1] = report_id;
    match report_id {
        0x80 => {
            if out.len() < 16 {
                return Err(FeatureError::BufferFull);
            }
            put_u16_le(out, 12, identity.model.vid);
            put_u16_le(out, 14, identity.model.pid);
            Ok(16)
        }
        0x83 => {
            put_ascii(out, 8, identity.versions.ap2);
            Ok(16)
        }
        0x86 => {
            put_ascii(out, 8, identity.versions.encoder_ap2);
            Ok(16)
        }
        0x8a => {
            put_ascii(out, 8, identity.versions.encoder_ld);
            Ok(16)
        }
        0x8f => {
            put_ascii(out, 8, identity.versions.ap2);
            Ok(16)
        }
        0x84 => {
            let bytes = identity.serial.as_bytes();
            let len = bytes.len().min(out.len() - 4).min(30);
            out[3] = len as u8;
            out[4..4 + len].copy_from_slice(&bytes[..len]);
            Ok(4 + len)
        }
        0x85 => {
            if out.len() < 10 {
                return Err(FeatureError::BufferFull);
            }
            out[4..10].copy_from_slice(&identity.mac);
            Ok(10)
        }
        0x1a => {
            // Provisional panel-ready report. Replaced by captured layout.
            if out.len() < 7 {
                return Err(FeatureError::BufferFull);
            }
            out[2] = identity.model.fullscreen_panels; // ready
            out[3] = identity.model.fullscreen_panels; // total
            out[4] = identity.model.rows;
            out[5] = identity.model.cols;
            out[6] = identity.model.encoder_count;
            Ok(7)
        }
        0x1c => {
            // Child device info (128-byte layout observed on Network Dock).
            if out.len() < 128 {
                return Err(FeatureError::BufferFull);
            }
            let Some(child) = identity.child else {
                out[4] = 0;
                return Ok(5);
            };
            out[4] = if child.connected { 0x02 } else { 0x00 };
            out[5] = child.rows;
            out[6] = child.cols;
            out[7] = child.key_w as u8;
            put_u16_le(out, 8, child.key_w);
            put_u16_le(out, 10, child.key_h);
            put_u16_le(out, 26, child.vid);
            put_u16_le(out, 28, child.pid);
            put_ascii(out, 30, "Elgato");
            put_ascii(out, 62, child.model_name);
            put_ascii(out, 94, child.serial);
            put_u16_le(out, 126, child.port);
            Ok(128)
        }
        _ => Err(FeatureError::Unsupported),
    }
}

/// Build a Cora capabilities event (`0x01 0x0b`) for a child device.
pub fn build_capabilities(
    child: &ChildInfo<'_>,
    port: u16,
    out: &mut [u8],
) -> Result<usize, FeatureError> {
    if out.len() < 128 {
        return Err(FeatureError::BufferFull);
    }
    out.fill(0);
    out[0] = 0x01;
    out[1] = 0x0b;
    let section_len: u16 = 124;
    put_u16_le(out, 2, section_len);
    out[4] = if child.connected { 0x02 } else { 0x00 };
    out[5] = child.rows;
    out[6] = child.cols;
    out[7] = child.layout_type;
    put_u16_le(out, 8, child.key_w);
    put_u16_le(out, 10, child.key_h);
    put_u16_le(out, 26, child.vid);
    put_u16_le(out, 28, child.pid);
    put_ascii(out, 30, "Elgato");
    put_ascii(out, 62, child.model_name);
    put_ascii(out, 94, child.serial);
    let _ = port;
    put_u16_le(out, 126, child.port);
    let _ = MAX_PANELS;
    Ok(128)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model;

    #[test]
    fn usb_firmware_report() {
        let identity = Identity::new(&model::STUDIO, "TESTSERIAL0001");
        let mut buf = [0u8; 32];
        let n = build_feature(FeatureStyle::Usb, 0x05, &identity, &mut buf).unwrap();
        assert_eq!(n, 14);
        assert_eq!(buf[0], 0x05);
        assert_eq!(buf[1], 0x0c);
        assert_eq!(&buf[6..14], b"1.00.000");
    }

    #[test]
    fn cora_primary_device_info() {
        let identity = Identity::new(&model::STUDIO, "TESTSERIAL0001");
        let mut buf = [0u8; 64];
        let n = build_feature(FeatureStyle::CoraPrimary, 0x80, &identity, &mut buf).unwrap();
        assert_eq!(n, 16);
        assert_eq!(buf[0], 0x03);
        assert_eq!(buf[1], 0x80);
        assert_eq!(u16::from_le_bytes([buf[12], buf[13]]), 0x0fd9);
        assert_eq!(u16::from_le_bytes([buf[14], buf[15]]), 0x00aa);
    }

    #[test]
    fn cora_serial_is_length_prefixed() {
        let identity = Identity::new(&model::STUDIO, "TESTSERIAL0001");
        let mut buf = [0u8; 64];
        let n = build_feature(FeatureStyle::CoraPrimary, 0x84, &identity, &mut buf).unwrap();
        assert_eq!(buf[3] as usize, "TESTSERIAL0001".len());
        assert_eq!(&buf[4..n], b"TESTSERIAL0001");
    }

    #[test]
    fn unsupported_report() {
        let identity = Identity::new(&model::STUDIO, "X");
        let mut buf = [0u8; 64];
        assert_eq!(
            build_feature(FeatureStyle::Usb, 0x99, &identity, &mut buf),
            Err(FeatureError::Unsupported)
        );
    }

    #[test]
    fn studio_captured_usb_reports() {
        // Captured from a real Studio (fw 2.01.002) via HidD_GetFeature.
        let identity = Identity {
            model: &model::STUDIO,
            serial: "A9IJA53330GUMR",
            mac: [0; 6],
            versions: FirmwareVersions {
                ap2: "2.01.002",
                ap1: "1.00.000",
                ld: "1.00.000",
                encoder_ap2: "1.01.012",
                encoder_ld: "1.01.006",
            },
            child: None,
        };
        let mut buf = [0u8; 32];
        let n = build_feature(FeatureStyle::Usb, 0x05, &identity, &mut buf).unwrap();
        assert_eq!(
            &buf[..n],
            &[0x05, 0x0c, 0xa2, 0xcc, 0x09, 0xb5, b'2', b'.', b'0', b'1', b'.', b'0', b'0', b'2']
        );

        let n = build_feature(FeatureStyle::Usb, 0x08, &identity, &mut buf).unwrap();
        assert_eq!(
            &buf[..n],
            &[
                0x08, 0x02, 0x10, 0x90, 0x00, 0x70, 0x00, 0x90, 0x05, 0x18, 0x01, 0x02, 0x00, 0x00,
                0x00, 0x04, 0xff
            ]
        );

        let n = build_feature(FeatureStyle::Usb, 0x11, &identity, &mut buf).unwrap();
        assert_eq!(n, 26);
        assert_eq!(&buf[..10], b"\x11\x181.01.012");
        assert_eq!(&buf[10..14], &[0x93, 0x7c, 0x27, 0x00]);

        let n = build_feature(FeatureStyle::Usb, 0x1a, &identity, &mut buf).unwrap();
        assert_eq!(&buf[..6], b"\x1a\x0c1.00");
        assert_eq!(&buf[10..14], &[0x5c, 0xeb, 0x15, 0x00]);
        assert_eq!(n, 14);
    }
}
