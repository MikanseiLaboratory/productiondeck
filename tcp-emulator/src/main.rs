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
use crate::input::ButtonState;
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

    /// Device serial number
    #[arg(long, default_value = "EMULATOR001")]
    serial: String,

    /// MAC address (hex, colon-separated, e.g. 00:11:22:33:44:55)
    #[arg(long, default_value = "00:11:22:33:44:55")]
    mac: String,

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
        firmware_version: "1.00.000".to_string(),
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
    println!("Commands:");
    println!("  press <key>      - press button (0-31)");
    println!("  release <key>    - release button (0-31)");
    println!("  tap <key>        - press and immediately release button");
    println!("  status           - list connected clients");
    println!("  quit             - exit");

    let mut button_state = ButtonState::default();
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
