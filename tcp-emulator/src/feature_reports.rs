/// Feature Report response builder for Stream Deck Studio (Cora / primary port).
///
/// Report payload layouts are derived from:
///   packages/tcp/src/connectionManager.ts  (TcpPropertiesService)
///   packages/tcp/src/hid-device/legacy.ts  (getDeviceInfo offset 12/14)
///   packages/core/src/services/properties/gen2.ts
///
/// The client (node-elgato-stream-deck) sends:
///   GET_REPORT flags=NONE payload=[0x03, report_id]  → primary port
///
/// Server responds with flags=RESULT, same message_id, payload as described.
use crate::studio::{BUTTON_COUNT, PRODUCT_ID, VENDOR_ID};

#[derive(Debug, Clone)]
pub struct DeviceConfig {
    pub serial: String,
    pub mac: [u8; 6],
    pub firmware_version: String,
}

impl Default for DeviceConfig {
    fn default() -> Self {
        Self {
            serial: "EMULATOR001".to_string(),
            mac: [0x00, 0x11, 0x22, 0x33, 0x44, 0x55],
            firmware_version: "6.06.001".to_string(),
        }
    }
}

/// Build a response payload for the given report_id.
/// Returns None for unknown / unsupported report IDs.
pub fn build_response(report_id: u8, config: &DeviceConfig) -> Option<Vec<u8>> {
    match report_id {
        0x80 => Some(build_device_info()),
        0x83 => Some(build_firmware_ap2(config)),
        0x84 => Some(build_serial(config)),
        0x85 => Some(build_mac(config)),
        0x86 => Some(build_firmware_encoder_ap2(config)),
        0x8a => Some(build_firmware_encoder_ld(config)),
        // 0x87, 0x88, 0x89, 0x8b..0x8e, 0x8f: Unknown firmware-component version reports.
        // Official Elgato software queries these; respond with same structure as 0x83.
        0x87 | 0x88 | 0x89 | 0x8b | 0x8c | 0x8d | 0x8e | 0x8f => {
            Some(build_firmware_generic(report_id, config))
        }
        // 0x08: Secondary device info (used in a race with 0x80 by getDeviceInfo()).
        // This emulator acts as Primary only; responding with Secondary info would
        // cause the client to treat this port as Secondary and then request 0x1C.
        // We intentionally do NOT respond to 0x08 so the 0x80 (Primary) race wins.
        // 0x1a: Device ready / operational status poll.
        // The official software polls this every ~2 seconds until the device is
        // ready to be shown in the UI. We respond with a minimal "ready" payload.
        // byte[2]=0x00 (status), byte[3]=BUTTON_COUNT, byte[4]=rows, byte[5]=cols
        0x1a => Some(build_device_status()),
        // 0x1c: Child device (Device 2) info.
        // After getDeviceInfo() resolves, autoConnectToSecondaries calls getChildDeviceInfo()
        // which sends 0x1C. parseDevice2Info() expects a 128-byte buffer:
        //   offset  4: connection status (0x02 = connected, anything else = not connected)
        //   offset 26-27: child VendorID (u16 LE)
        //   offset 28-29: child ProductID (u16 LE)
        //   offset 94-124: child serial number (ASCII, NULL-terminated)
        //   offset 126-127: child TCP port (u16 LE)
        // We return "no child device connected" (offset[4]=0x00).
        0x1c => Some(build_device2_info_none()),
        _ => None,
    }
}

/// 0x80 — Device Info
/// Layout (from legacy.ts getDeviceInfo):
///   offset 0-1:   [0x03, 0x80]
///   offset 12-13: vendorId (u16 LE)
///   offset 14-15: productId (u16 LE)
///   total: 1024 bytes (padded)
fn build_device_info() -> Vec<u8> {
    let mut buf = vec![0u8; 1024];
    buf[0] = 0x03;
    buf[1] = 0x80;
    buf[12..14].copy_from_slice(&VENDOR_ID.to_le_bytes());
    buf[14..16].copy_from_slice(&PRODUCT_ID.to_le_bytes());
    buf
}

/// 0x83 — Firmware Version (AP2)
/// Layout (from connectionManager.ts getFirmwareVersion):
///   offset 0-1:  [0x03, 0x83]
///   offset 8-15: ASCII firmware version, up to 8 chars
fn build_firmware_ap2(config: &DeviceConfig) -> Vec<u8> {
    build_firmware_generic(0x83, config)
}

/// 0x84 — Serial Number
/// Layout (from connectionManager.ts getSerialNumber):
///   offset 0-1: [0x03, 0x84]
///   offset 3:   length (u8)
///   offset 4..: ASCII serial
fn build_serial(config: &DeviceConfig) -> Vec<u8> {
    let mut buf = vec![0u8; 64];
    buf[0] = 0x03;
    buf[1] = 0x84;
    let serial = config.serial.as_bytes();
    let len = serial.len().min(255) as u8;
    buf[3] = len;
    buf[4..4 + len as usize].copy_from_slice(&serial[..len as usize]);
    buf
}

/// 0x85 — MAC Address
/// Layout (from tcpWrapper.ts getMacAddress):
///   offset 0-1:  [0x03, 0x85]
///   offset 4-9:  MAC address (6 bytes)
fn build_mac(config: &DeviceConfig) -> Vec<u8> {
    let mut buf = vec![0u8; 64];
    buf[0] = 0x03;
    buf[1] = 0x85;
    buf[4..10].copy_from_slice(&config.mac);
    buf
}

/// 0x86 — Encoder firmware AP2
fn build_firmware_encoder_ap2(config: &DeviceConfig) -> Vec<u8> {
    build_firmware_generic(0x86, config)
}

/// 0x8a — Encoder firmware LD
fn build_firmware_encoder_ld(config: &DeviceConfig) -> Vec<u8> {
    build_firmware_generic(0x8a, config)
}

/// Generic firmware report builder: [0x03, id, 0x00*6, version(8 bytes), 0x00...]
fn build_firmware_generic(report_id: u8, config: &DeviceConfig) -> Vec<u8> {
    let mut buf = vec![0u8; 64];
    buf[0] = 0x03;
    buf[1] = report_id;
    let ver = config.firmware_version.as_bytes();
    let len = ver.len().min(8);
    buf[8..8 + len].copy_from_slice(&ver[..len]);
    buf
}

/// 0x1a — Device panel/display ready status.
/// The official software polls this every ~2 seconds after "Wake up".
/// byte[2] = number of panels currently ready; byte[3] = total panel count.
/// Software waits until byte[2] == byte[3] (all panels ready) before proceeding.
/// Stream Deck Studio: 32 LCD panels (4 rows x 8 cols), 2 encoders.
///
/// NOTE: The exact byte layout below is inferred from node-elgato source code and
/// has NOT been verified against a real device capture. If the official software
/// stalls polling on 0x1A without progressing to UI display, capture the real
/// device response and update the offsets here (TODO: verify with hardware capture).
fn build_device_status() -> Vec<u8> {
    let mut buf = vec![0u8; 64];
    buf[0] = 0x03;
    buf[1] = 0x1a;
    buf[2] = BUTTON_COUNT as u8;           // ready panels = 32 (all ready)
    buf[3] = BUTTON_COUNT as u8;           // total panels = 32
    buf[4] = 4;                            // rows
    buf[5] = 8;                            // cols
    buf[6] = 2;                            // encoder count
    buf
}

/// 0x1c — Child device (Device 2) info.
/// Returns "no child device connected": 128-byte buffer with offset[4]=0x00.
/// parseDevice2Info() checks data[4] == 0x02 for "connected"; any other value
/// means "not connected" and the client skips secondary auto-connect.
fn build_device2_info_none() -> Vec<u8> {
    let mut buf = vec![0u8; 128];
    buf[0] = 0x03;
    buf[1] = 0x1c;
    // buf[4] = 0x00 (default) → not connected
    buf
}

