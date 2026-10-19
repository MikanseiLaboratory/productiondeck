//! Studio / Stream Deck verification harness.
//!
//! Usage:
//!   cargo run -p productiondeck-verify --bin studio-verify -- streamdeck
//!   cargo run -p productiondeck-verify --bin studio-verify -- hardware
//!
//! `streamdeck` drives the `streamdeck` crate (rust-streamdeck) which supports
//! the classic / Mk.2 / XL / Plus / Module models. It deliberately shows that
//! Studio (`0x00AA`) is not yet known upstream.
//!
//! `hardware` speaks directly to a connected Studio through `hidapi`: it reads
//! the captured feature reports, sets brightness and sends a JPEG to a key.

use anyhow::{bail, Context, Result};
use hidapi::HidApi;
use image::codecs::jpeg::JpegEncoder;
use image::ExtendedColorType;

const ELGATO_VID: u16 = 0x0fd9;
const STUDIO_PID: u16 = 0x00aa;

/// Supported-by-upstream PIDs (rust-streamdeck 0.10).
const STREAMDECK_PIDS: &[(u16, &str)] = &[
    (0x0063, "Mini"),
    (0x0090, "Mini 2022"),
    (0x0060, "Original"),
    (0x006d, "Original V2"),
    (0x006c, "XL"),
    (0x0080, "Mk.2"),
    (0x0084, "Plus"),
    (0x00b8, "Module 6"),
    (0x00b9, "Module 15"),
    (0x00ba, "Module 32"),
];

fn main() -> Result<()> {
    let mode = std::env::args().nth(1).unwrap_or_else(|| "all".to_string());
    match mode.as_str() {
        "streamdeck" => {
            streamdeck_probe();
            Ok(())
        }
        "hardware" => hardware(),
        "all" => {
            streamdeck_probe();
            println!();
            hardware()
        }
        other => bail!("unknown mode {other:?}; use streamdeck | hardware | all"),
    }
}

/// Exercise the upstream `streamdeck` crate (rust-streamdeck).
fn streamdeck_probe() {
    println!("== rust-streamdeck (crate `streamdeck`) ==");
    match streamdeck::StreamDeck::probe() {
        Ok(devices) => {
            if devices.is_empty() {
                println!("no upstream-supported Stream Deck detected");
            }
            for device in devices {
                println!("detected: {device:?}");
            }
        }
        Err(e) => println!("probe failed: {e}"),
    }

    for (pid, name) in STREAMDECK_PIDS {
        match streamdeck::StreamDeck::connect(ELGATO_VID, *pid, None) {
            Ok(mut deck) => {
                let product = deck.product().unwrap_or_else(|_| name.to_string());
                let serial = deck.serial().unwrap_or_default();
                let version = deck.version().unwrap_or_default();
                println!(
                    "{name} ({pid:#06x}): product={product:?} serial={serial:?} fw={version:?}"
                );
                let _ = deck.set_brightness(80);
            }
            Err(e) => println!("{name} ({pid:#06x}): {e}"),
        }
    }

    match streamdeck::StreamDeck::connect(ELGATO_VID, STUDIO_PID, None) {
        Ok(_) => println!("Studio: supported by upstream `streamdeck`"),
        Err(e) => println!("Studio ({STUDIO_PID:#06x}): upstream crate returned {e} (expected)"),
    }
}

/// Direct HID verification against a real Studio.
fn hardware() -> Result<()> {
    println!("== direct HID verification (Stream Deck Studio) ==");
    let api = HidApi::new().context("init hidapi")?;
    let info = api
        .device_list()
        .find(|d| d.vendor_id() == ELGATO_VID && d.product_id() == STUDIO_PID)
        .context("no 0fd9:00aa Studio found")?;
    let device = info
        .open_device(&api)
        .context("open Studio HID interface")?;

    let product = device
        .get_product_string()
        .ok()
        .flatten()
        .unwrap_or_default();
    let serial = device
        .get_serial_number_string()
        .ok()
        .flatten()
        .unwrap_or_default();
    println!("product={product:?} serial={serial:?}");

    for report_id in [0x05u8, 0x06, 0x08, 0x11, 0x13, 0x1a, 0x1c, 0x80] {
        let mut buf = [0u8; 64];
        buf[0] = report_id;
        match device.get_feature_report(&mut buf) {
            Ok(n) => println!("feature {report_id:#04x}: {}", hex(&buf[..n.min(32)])),
            Err(e) => println!("feature {report_id:#04x}: {e}"),
        }
    }

    // Brightness round trip (restore 80%).
    device
        .send_feature_report(&[0x03, 0x08, 50])
        .context("set brightness 50")?;
    std::thread::sleep(std::time::Duration::from_millis(100));
    device
        .send_feature_report(&[0x03, 0x08, 80])
        .context("set brightness 80")?;
    println!("brightness set to 50 then restored to 80");

    // Send a red JPEG to key 0 and key 31.
    let jpeg = solid_jpeg(144, 112, [255, 0, 0])?;
    for key in [0u8, 31] {
        match send_key_image(&device, key, &jpeg) {
            Ok(()) => println!("key {key}: {} byte JPEG sent", jpeg.len()),
            Err(e) => println!("key {key}: image write failed: {e}"),
        }
    }

    // Read input reports for a couple of seconds.
    println!("reading input reports for 2s (press keys / turn a dial)...");
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
    let mut buf = [0u8; 512];
    while std::time::Instant::now() < deadline {
        match device.read_timeout(&mut buf, 250) {
            Ok(n) if n > 0 => {
                println!("input report ({} bytes): {}", n, hex(&buf[..n.min(32)]));
            }
            Ok(_) => {}
            Err(e) => {
                println!("input read error: {e}");
                break;
            }
        }
    }

    Ok(())
}

fn send_key_image(device: &hidapi::HidDevice, key: u8, jpeg: &[u8]) -> Result<()> {
    // Gen2 key image: [0x02, 0x07, key, last, size16, chunk16, payload]
    const HEADER: usize = 8;
    const REPORT: usize = 1024;
    const MAX_PAYLOAD: usize = REPORT - HEADER;
    let chunks: Vec<&[u8]> = jpeg.chunks(MAX_PAYLOAD).collect();
    if chunks.is_empty() {
        bail!("empty image");
    }
    for (index, chunk) in chunks.iter().enumerate() {
        let is_last = index == chunks.len() - 1;
        let mut report = [0u8; REPORT];
        report[0] = 0x02;
        report[1] = 0x07;
        report[2] = key;
        report[3] = u8::from(is_last);
        report[4..6].copy_from_slice(&(chunk.len() as u16).to_le_bytes());
        report[6..8].copy_from_slice(&(index as u16).to_le_bytes());
        report[HEADER..HEADER + chunk.len()].copy_from_slice(chunk);
        let written = device.write(&report).context("HID output report write")?;
        if written != REPORT {
            bail!("short HID write: {written}/{REPORT}");
        }
    }
    Ok(())
}

fn solid_jpeg(width: u32, height: u32, rgb: [u8; 3]) -> Result<Vec<u8>> {
    let mut pixels = Vec::with_capacity((width * height * 3) as usize);
    for _ in 0..(width * height) {
        pixels.extend_from_slice(&rgb);
    }
    let mut out = Vec::new();
    let mut encoder = JpegEncoder::new_with_quality(&mut out, 85);
    encoder
        .encode(&pixels, width, height, ExtendedColorType::Rgb8)
        .context("encode JPEG")?;
    Ok(out)
}

fn hex(bytes: &[u8]) -> String {
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        s.push_str(&format!("{b:02x}"));
    }
    s
}
