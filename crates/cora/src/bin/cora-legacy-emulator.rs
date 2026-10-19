//! Legacy (headerless 512/1024) Studio emulator.
//!
//! Real Studio firmware 2.01.002 speaks the legacy framing on TCP 5343:
//!   512-byte keep-alive, 1024-byte ACK/query, 512-byte feature response.
//! This binary advertises a Studio (`dt=212`) plus an XL child (`dt=213`,
//! port 20001) and serves both.

use std::time::Duration;

use anyhow::Result;
use productiondeck_cora::core::feature::{build_capabilities, ChildInfo, FirmwareVersions, Identity};
use productiondeck_cora::core::model;
use productiondeck_cora::discovery::{Advertisement, DiscoveryConfig};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

const STUDIO_PORT: u16 = 5343;
const CHILD_PORT: u16 = 20001;
const KA: usize = 512;
const CMD: usize = 1024;

fn checksum(version: &str) -> [u8; 4] {
    match version {
        "2.01.002" => [0xa2, 0xcc, 0x09, 0xb5],
        "1.01.012" => [0x93, 0x7c, 0x27, 0x00],
        "1.01.006" => [0x7a, 0x4d, 0x19, 0x00],
        "1.00.000" => [0x5c, 0xeb, 0x15, 0x00],
        _ => [0; 4],
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let child = ChildInfo {
        connected: true,
        vid: 0x0fd9,
        pid: model::XL.pid,
        serial: "EMULATORXL0001",
        port: CHILD_PORT,
        model_name: "Stream Deck XL",
        key_w: 96,
        key_h: 96,
        rows: 4,
        cols: 8,
        layout_type: 0,
    };
    let studio = Identity {
        model: &model::STUDIO,
        serial: "EMULATORSTUDIO1",
        mac: [0x80, 0x7b, 0x1e, 0x02, 0x56, 0xc1],
        versions: FirmwareVersions {
            ap2: "2.01.002",
            ap1: "1.00.000",
            ld: "1.00.000",
            encoder_ap2: "1.01.012",
            encoder_ld: "1.01.006",
        },
        child: Some(child),
    };
    let xl = Identity {
        model: &model::XL,
        serial: "EMULATORXL0001",
        mac: [0x80, 0x7b, 0x1e, 0x02, 0x56, 0xc2],
        versions: FirmwareVersions {
            ap2: "1.01.000",
            ap1: "1.01.000",
            ld: "1.01.000",
            encoder_ap2: "1.01.000",
            encoder_ld: "1.01.000",
        },
        child: None,
    };

    let parent = TcpListener::bind(("0.0.0.0", STUDIO_PORT)).await?;
    let child_listener = TcpListener::bind(("0.0.0.0", CHILD_PORT)).await?;
    tokio::spawn(accept(parent, studio));
    tokio::spawn(accept(child_listener, xl));

    let _adv_parent = Advertisement::start(&DiscoveryConfig {
        instance: "Stream Deck Studio (emu)".to_string(),
        port: STUDIO_PORT,
        serial: "EMULATORSTUDIO1".to_string(),
        model_name: "Stream Deck Studio 20GBO9901".to_string(),
        firmware: "2.01.002".to_string(),
        dt: "212".to_string(),
        vid: "0x0FD9".to_string(),
        pid: "0x00AA".to_string(),
        hwrev: "0412".to_string(),
        prt: None,
        mac: Some("80:7B:1E:02:56:C1".to_string()),
    })?;
    let _adv_child = Advertisement::start(&DiscoveryConfig {
        instance: "Stream Deck XL (emu)".to_string(),
        port: CHILD_PORT,
        serial: "EMULATORXL0001".to_string(),
        model_name: "Stream Deck XL 20GAT9901".to_string(),
        firmware: "1.01.000".to_string(),
        dt: "213".to_string(),
        vid: "0x0FD9".to_string(),
        pid: "0x006C".to_string(),
        hwrev: "0412".to_string(),
        prt: Some("1".to_string()),
        mac: Some("80:7B:1E:02:56:C2".to_string()),
    })?;

    println!("legacy Studio emulator: 5343, child XL: 20001");
    tokio::signal::ctrl_c().await?;
    Ok(())
}

async fn accept(listener: TcpListener, identity: Identity<'static>) {
    loop {
        let Ok((stream, _)) = listener.accept().await else {
            continue;
        };
        tokio::spawn(connection(stream, identity));
    }
}

async fn connection(mut stream: TcpStream, identity: Identity<'static>) {
    let mut keepalive = [0u8; KA];
    keepalive[0] = 0x01;
    keepalive[1] = 0x0a;
    keepalive[2] = 0x02;
    keepalive[4] = 0x01;
    if stream.write_all(&keepalive).await.is_err() {
        return;
    }
    let mut tick = tokio::time::interval(Duration::from_secs(2));
    let mut buf = [0u8; CMD];
    loop {
        tokio::select! {
            _ = tick.tick() => {
                if stream.write_all(&keepalive).await.is_err() { return; }
            }
            read = stream.read_exact(&mut buf) => {
                if read.is_err() { return; }
                if buf[0] == 0x03 {
                    if buf[1] == 0x1a { continue; }
                    let response = build_response(buf[1], &identity);
                    if stream.write_all(&response).await.is_err() { return; }
                }
            }
        }
    }
}

fn build_response(id: u8, identity: &Identity<'_>) -> [u8; KA] {
    let mut out = [0u8; KA];
    out[0] = 0x03;
    out[1] = id;
    match id {
        0x80 => {
            let m = identity.model;
            out[2] = m.rows;
            out[3] = m.cols;
            out[4..6].copy_from_slice(&m.key_w.to_le_bytes());
            out[6..8].copy_from_slice(&m.key_h.to_le_bytes());
            out[8..10].copy_from_slice(&m.lcd_w.to_le_bytes());
            out[10..12].copy_from_slice(&m.lcd_h.to_le_bytes());
            out[12..14].copy_from_slice(&m.vid.to_le_bytes());
            out[14..16].copy_from_slice(&m.pid.to_le_bytes());
        }
        0x83 => {
            out[2] = 0;
            out[3] = 0;
            out[4..8].copy_from_slice(&checksum(identity.versions.ap2));
            let v = identity.versions.ap2.as_bytes();
            out[8..8 + v.len().min(8)].copy_from_slice(&v[..v.len().min(8)]);
        }
        0x84 => {
            let s = identity.serial.as_bytes();
            let len = s.len().min(30);
            out[2] = 0;
            out[3] = len as u8;
            out[4..4 + len].copy_from_slice(&s[..len]);
        }
        0x85 => {
            out[2] = 6;
            out[3] = 0;
            out[4..10].copy_from_slice(&identity.mac);
        }
        0x86 => dual_fw(&mut out, identity.versions.encoder_ap2),
        0x8a => dual_fw(&mut out, identity.versions.encoder_ld),
        0x8f => {
            out[2] = 2;
            out[3] = 0;
            out[4] = 1;
        }
        0x1c => {
            if let Some(c) = &identity.child {
                let mut caps = [0u8; 128];
                let _ = build_capabilities(c, c.port, &mut caps);
                out[..128].copy_from_slice(&caps);
            }
        }
        _ => {}
    }
    out
}

fn dual_fw(out: &mut [u8; KA], version: &str) {
    let sum = checksum(version);
    let v = version.as_bytes();
    let n = v.len().min(8);
    out[2] = 0x18;
    out[3] = 0;
    out[4..4 + n].copy_from_slice(&v[..n]);
    out[12..16].copy_from_slice(&sum);
    out[16..16 + n].copy_from_slice(&v[..n]);
    out[24..28].copy_from_slice(&sum);
}

