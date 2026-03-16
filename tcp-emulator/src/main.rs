mod cora;
mod discovery;
mod feature_reports;
mod input;
mod server;
mod session;
mod studio;

use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::info;

use clap::Parser;

use crate::feature_reports::DeviceConfig;
use crate::input::{ButtonState, EncoderState};
use crate::server::{run_server, ServerState};
use crate::session::SessionCommand;
use crate::studio::DEFAULT_TCP_PORT;

/// Stream Deck Studio TCP emulator (Cora protocol)
#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Cli {
    /// TCP port to listen on
    #[arg(long, default_value_t = DEFAULT_TCP_PORT)]
    port: u16,

    /// Device serial number (feature report 0x84)
    #[arg(long, default_value = "EMULATOR001")]
    serial: String,

    /// MAC address (hex, colon-separated, e.g. 00:11:22:33:44:55) (feature report 0x85)
    #[arg(long, default_value = "00:11:22:33:44:55")]
    mac: String,

    /// Firmware version string reported to the host (feature report 0x83, 8 chars max)
    /// Use a version close to real Studio firmware, e.g. "6.06.001", to avoid
    /// "Device firmware is not supported" errors from the official software.
    #[arg(long, default_value = "6.06.001")]
    firmware: String,

    /// Disable mDNS advertisement
    #[arg(long)]
    no_mdns: bool,

    /// mDNS service instance name
    #[arg(long, default_value = "StreamDeck-Emulator")]
    mdns_name: String,
}

fn parse_mac(s: &str) -> anyhow::Result<[u8; 6]> {
    let parts: Vec<&str> = s.split(':').collect();
    if parts.len() != 6 {
        anyhow::bail!("MAC address must have 6 colon-separated octets");
    }
    let mut mac = [0u8; 6];
    for (i, p) in parts.iter().enumerate() {
        mac[i] = u8::from_str_radix(p, 16)?;
    }
    Ok(mac)
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    let cli = Cli::parse();

    let mac = parse_mac(&cli.mac)?;
    let config = Arc::new(DeviceConfig {
        serial: cli.serial.clone(),
        mac,
        firmware_version: cli.firmware.clone(),
    });

    // Start mDNS advertisement
    let _mdns = if !cli.no_mdns {
        match discovery::MdnsAdvertiser::start(&cli.mdns_name, cli.port, &cli.serial) {
            Ok(adv) => Some(adv),
            Err(e) => {
                tracing::warn!("mDNS advertisement failed (continuing without): {}", e);
                None
            }
        }
    } else {
        info!("mDNS advertisement disabled");
        None
    };

    let state = Arc::new(Mutex::new(ServerState::default()));
    let state_cli = state.clone();

    // Spawn TCP server
    let server_handle = {
        let config = config.clone();
        let state = state.clone();
        tokio::spawn(async move {
            if let Err(e) = run_server(cli.port, config, state).await {
                tracing::error!("Server error: {}", e);
            }
        })
    };

    // Interactive CLI loop (stdin)
    println!("Stream Deck Studio Emulator started on port {}", cli.port);
    println!("  serial:   {}", cli.serial);
    println!("  firmware: {}", cli.firmware);
    println!("  mac:      {}", cli.mac);
    println!("Commands:");
    println!("  press <key>               - press button (0-31)");
    println!("  release <key>             - release button (0-31)");
    println!("  tap <key>                 - press and immediately release button");
    println!("  encoder_press <idx>       - press encoder (0-1)");
    println!("  encoder_release <idx>     - release encoder (0-1)");
    println!("  encoder_rotate <idx> <d>  - rotate encoder (0-1) by delta ticks (+/-)");
    println!("  status                    - list connected clients");
    println!("  quit                      - exit");

    let mut button_state = ButtonState::default();
    let mut encoder_state = EncoderState::default();
    let mut line = String::new();

    loop {
        line.clear();
        if tokio::io::AsyncBufReadExt::read_line(
            &mut tokio::io::BufReader::new(tokio::io::stdin()),
            &mut line,
        )
        .await?
            == 0
        {
            break; // EOF
        }

        let parts: Vec<&str> = line.trim().split_whitespace().collect();
        if parts.is_empty() {
            continue;
        }

        match parts[0] {
            "press" => {
                if let Some(idx) = parts.get(1).and_then(|s| s.parse::<usize>().ok()) {
                    if button_state.press(idx) {
                        broadcast_input(&state_cli, &button_state).await;
                        println!("Pressed button {}", idx);
                    } else {
                        println!("Invalid button index {}", idx);
                    }
                } else {
                    println!("Usage: press <key>");
                }
            }
            "release" => {
                if let Some(idx) = parts.get(1).and_then(|s| s.parse::<usize>().ok()) {
                    if button_state.release(idx) {
                        broadcast_input(&state_cli, &button_state).await;
                        println!("Released button {}", idx);
                    } else {
                        println!("Invalid button index {}", idx);
                    }
                } else {
                    println!("Usage: release <key>");
                }
            }
            "tap" => {
                if let Some(idx) = parts.get(1).and_then(|s| s.parse::<usize>().ok()) {
                    if button_state.press(idx) {
                        broadcast_input(&state_cli, &button_state).await;
                        // Small delay then release
                        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
                        button_state.release(idx);
                        broadcast_input(&state_cli, &button_state).await;
                        println!("Tapped button {}", idx);
                    } else {
                        println!("Invalid button index {}", idx);
                    }
                } else {
                    println!("Usage: tap <key>");
                }
            }
            "encoder_press" => {
                if let Some(idx) = parts.get(1).and_then(|s| s.parse::<usize>().ok()) {
                    if encoder_state.set_press(idx, true) {
                        broadcast_encoder(&state_cli, &encoder_state, None).await;
                        println!("Pressed encoder {}", idx);
                    } else {
                        println!("Invalid encoder index {}", idx);
                    }
                } else {
                    println!("Usage: encoder_press <idx>");
                }
            }
            "encoder_release" => {
                if let Some(idx) = parts.get(1).and_then(|s| s.parse::<usize>().ok()) {
                    if encoder_state.set_press(idx, false) {
                        broadcast_encoder(&state_cli, &encoder_state, None).await;
                        println!("Released encoder {}", idx);
                    } else {
                        println!("Invalid encoder index {}", idx);
                    }
                } else {
                    println!("Usage: encoder_release <idx>");
                }
            }
            "encoder_rotate" => {
                let idx = parts.get(1).and_then(|s| s.parse::<usize>().ok());
                let delta = parts.get(2).and_then(|s| s.parse::<i8>().ok());
                match (idx, delta) {
                    (Some(i), Some(d)) => {
                        if i < crate::studio::ENCODER_COUNT {
                            broadcast_encoder(&state_cli, &encoder_state, Some((i, d))).await;
                            println!("Rotated encoder {} by {}", i, d);
                        } else {
                            println!("Invalid encoder index {}", i);
                        }
                    }
                    _ => println!("Usage: encoder_rotate <idx> <delta>"),
                }
            }
            "status" => {
                let s = state_cli.lock().await;
                if s.sessions.is_empty() {
                    println!("No clients connected");
                } else {
                    println!("Connected clients ({}):", s.sessions.len());
                    for addr in s.sessions.keys() {
                        println!("  {}", addr);
                    }
                }
            }
            "quit" | "exit" => {
                println!("Shutting down...");
                // Disconnect all sessions
                let s = state_cli.lock().await;
                for tx in s.sessions.values() {
                    let _ = tx.try_send(SessionCommand::Disconnect);
                }
                break;
            }
            other => {
                println!("Unknown command: {}. Type 'quit' to exit.", other);
            }
        }
    }

    server_handle.abort();
    Ok(())
}

async fn broadcast_input(state: &Arc<Mutex<ServerState>>, button_state: &ButtonState) {
    let payload = button_state.to_payload();
    let s = state.lock().await;
    for tx in s.sessions.values() {
        let _ = tx.try_send(SessionCommand::SendInput(payload.clone()));
    }
}

/// Broadcast an encoder event to all connected sessions.
/// If `rotation` is Some((index, delta)), sends a rotation payload; otherwise sends press state.
async fn broadcast_encoder(
    state: &Arc<Mutex<ServerState>>,
    encoder_state: &EncoderState,
    rotation: Option<(usize, i8)>,
) {
    let payload = match rotation {
        Some((idx, delta)) => encoder_state.to_rotation_payload(idx, delta),
        None => encoder_state.to_payload(),
    };
    let s = state.lock().await;
    for tx in s.sessions.values() {
        let _ = tx.try_send(SessionCommand::SendInput(payload.clone()));
    }
}
