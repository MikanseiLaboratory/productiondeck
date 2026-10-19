//! Cora probe: connect to a device (or discover via mDNS) and dump/inspect the
//! feature reports. Intended for the capture phase of Issue #23.

use std::time::Duration;

use anyhow::Result;
use clap::Parser;
use productiondeck_cora::client::CoraClient;
use productiondeck_cora::discovery;
use productiondeck_cora::FeatureStyle;

#[derive(Debug, Parser)]
#[command(version, about = "Probe a Cora Stream Deck device")]
struct Cli {
    /// Host / IP address
    host: Option<String>,
    /// TCP port
    #[arg(long, default_value_t = 5343)]
    port: u16,
    /// Browse mDNS for `_elg._tcp` and print results, then exit
    #[arg(long)]
    discover: bool,
    /// Query feature reports with the official-app WRITE style as well
    #[arg(long)]
    write_style: bool,
    /// Seconds to listen for frames after the probes
    #[arg(long, default_value_t = 0)]
    listen: u64,
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    if cli.discover {
        let found = discovery::discover(Duration::from_secs(5))?;
        if found.is_empty() {
            println!("no _elg._tcp services found");
        }
        for service in found {
            println!(
                "{}  {}:{}  dock={}  txt={:?}",
                service.fullname,
                service
                    .addresses
                    .first()
                    .map(|a| a.to_string())
                    .unwrap_or_else(|| service.host.clone()),
                service.port,
                service.is_network_dock(),
                service.txt
            );
        }
        return Ok(());
    }

    let host = cli
        .host
        .ok_or_else(|| anyhow::anyhow!("host is required unless --discover is used"))?;
    let mut client = CoraClient::connect((host.as_str(), cli.port)).await?;
    println!("connected to {host}:{}", cli.port);
    client.handshake().await?;
    println!("keep-alive acknowledged");

    let primaries = [0x80u8, 0x08, 0x83, 0x84, 0x85, 0x86, 0x8a, 0x8f, 0x1a, 0x1c];
    for id in primaries {
        match client.get_feature(id, FeatureStyle::CoraPrimary).await {
            Ok(payload) => {
                println!(
                    "GET  {id:#04x} (primary): {}",
                    hex(&payload[..payload.len().min(64)])
                );
                decode(id, &payload);
            }
            Err(e) => println!("GET  {id:#04x} (primary): {e:#}"),
        }
        if cli.write_style {
            match client.get_feature_write_style(id).await {
                Ok(payload) => {
                    println!(
                        "WRITE {id:#04x}: {}",
                        hex(&payload[..payload.len().min(64)])
                    );
                    decode(id, &payload);
                }
                Err(e) => println!("WRITE {id:#04x}: {e:#}"),
            }
        }
    }

    // Child-style probes used by node's primary/secondary race.
    for id in [0x05u8, 0x06, 0x08, 0x0b] {
        match client.get_feature(id, FeatureStyle::CoraChild).await {
            Ok(payload) => println!(
                "GET  {id:#04x} (child):   {}",
                hex(&payload[..payload.len().min(32)])
            ),
            Err(e) => println!("GET  {id:#04x} (child):   {e:#}"),
        }
    }

    if cli.listen > 0 {
        println!("listening for {0}s ...", cli.listen);
        let deadline = tokio::time::Instant::now() + Duration::from_secs(cli.listen);
        while tokio::time::Instant::now() < deadline {
            let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
            match client.recv_frame(remaining).await {
                Ok(Some(frame)) => {
                    println!(
                        "RX flags={:#06x} op={:?} id={:#x} len={} {}",
                        frame.flags.bits(),
                        frame.hid_op,
                        frame.message_id,
                        frame.payload.len(),
                        hex(&frame.payload[..frame.payload.len().min(32)])
                    );
                    if frame.is_keep_alive() {
                        client.ack_keep_alive(&frame).await?;
                    }
                }
                Ok(None) => break,
                Err(e) => {
                    println!("RX error: {e:#}");
                    break;
                }
            }
        }
    }

    Ok(())
}

fn hex(bytes: &[u8]) -> String {
    let mut s = String::with_capacity(bytes.len() * 3);
    for b in bytes {
        s.push_str(&format!("{b:02x} "));
    }
    s
}

fn decode(report_id: u8, payload: &[u8]) {
    match report_id {
        0x80 if payload.len() >= 16 => {
            let vid = u16::from_le_bytes([payload[12], payload[13]]);
            let pid = u16::from_le_bytes([payload[14], payload[15]]);
            println!("      -> VID:PID = {vid:#06x}:{pid:#06x}");
        }
        0x83 | 0x86 | 0x8a | 0x8f if payload.len() >= 16 => {
            let text: Vec<u8> = payload[8..16]
                .iter()
                .copied()
                .take_while(|b| *b != 0)
                .collect();
            println!("      -> firmware = {}", String::from_utf8_lossy(&text));
        }
        0x84 if payload.len() >= 4 => {
            let len = payload[3] as usize;
            let end = (4 + len).min(payload.len());
            println!(
                "      -> serial len={len} = {}",
                String::from_utf8_lossy(&payload[4..end])
            );
        }
        0x85 if payload.len() >= 10 => {
            let mac = &payload[4..10];
            println!(
                "      -> MAC = {:02X}:{:02X}:{:02X}:{:02X}:{:02X}:{:02X}",
                mac[0], mac[1], mac[2], mac[3], mac[4], mac[5]
            );
        }
        0x1c if payload.len() >= 128 => {
            let status = payload[4];
            let vid = u16::from_le_bytes([payload[26], payload[27]]);
            let pid = u16::from_le_bytes([payload[28], payload[29]]);
            let port = u16::from_le_bytes([payload[126], payload[127]]);
            println!(
                "      -> child status={status:#04x} vid:pid={vid:#06x}:{pid:#06x} port={port}"
            );
        }
        0x1a if payload.len() >= 7 => {
            println!(
                "      -> 0x1a bytes: ready={} total={} rows={} cols={} encoders={}",
                payload[2], payload[3], payload[4], payload[5], payload[6]
            );
        }
        _ => {}
    }
}

