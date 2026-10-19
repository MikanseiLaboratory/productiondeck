//! Stream Deck Studio / Network Dock Cora emulator.

use std::path::PathBuf;

use anyhow::{bail, Context, Result};
use productiondeck_core::feature::{ChildInfo, FirmwareVersions, Identity};
use productiondeck_core::input::InputEvent;
use productiondeck_core::model::{self, ModelInfo};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::sync::mpsc;

use crate::command::DeckCommand;
use crate::discovery::{Advertisement, DiscoveryConfig};
use crate::render::VirtualStudio;
use crate::server::CoraServer;

/// Emulated device profile.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Profile {
    Studio,
    NetworkDock,
}

/// Emulator configuration (populated by the CLI).
#[derive(Debug, Clone)]
pub struct EmulatorConfig {
    pub bind: String,
    pub port: u16,
    pub serial: String,
    pub mac: [u8; 6],
    pub firmware: String,
    pub mdns_name: String,
    pub no_mdns: bool,
    pub profile: Profile,
    /// Override the mDNS `dt` value (capture decides the real Studio value).
    pub dt_override: Option<String>,
    pub render_out: Option<PathBuf>,
}

impl Default for EmulatorConfig {
    fn default() -> Self {
        Self {
            bind: "0.0.0.0".to_string(),
            port: 5343,
            serial: "PRODUCTIONDK01".to_string(),
            mac: [0x02, 0x00, 0x00, 0x00, 0x00, 0x01],
            firmware: "2.01.002".to_string(),
            mdns_name: "Stream Deck Studio".to_string(),
            no_mdns: false,
            profile: Profile::Studio,
            dt_override: None,
            render_out: None,
        }
    }
}

/// Run the emulator until `quit` / Ctrl-C.
pub async fn run(config: EmulatorConfig) -> Result<()> {
    let (model, default_dt, model_name) = match config.profile {
        Profile::Studio => (&model::STUDIO, "213", "Stream Deck Studio"),
        Profile::NetworkDock => (&model::NETWORK_DOCK, "215", "Stream Deck Network Dock"),
    };
    let identity = make_identity(&config, model);
    let (command_tx, mut command_rx) = mpsc::unbounded_channel::<DeckCommand>();

    let server = CoraServer::bind(
        (config.bind.as_str(), config.port),
        model,
        identity,
        command_tx,
    )
    .await
    .with_context(|| format!("start Cora server on {}:{}", config.bind, config.port))?;

    let _advertisement = if config.no_mdns {
        None
    } else {
        let discovery = DiscoveryConfig {
            instance: config.mdns_name.clone(),
            port: config.port,
            serial: config.serial.clone(),
            model_name: model_name.to_string(),
            firmware: config.firmware.clone(),
            dt: config
                .dt_override
                .clone()
                .unwrap_or_else(|| default_dt.to_string()),
            vid: format!("0x{:04X}", model.vid),
            pid: format!("0x{:04X}", model.pid),
            hwrev: "0200".to_string(),
            prt: Some("2".to_string()),
            mac: Some(format_mac(&config.mac)),
        };
        match Advertisement::start(&discovery) {
            Ok(adv) => {
                println!(
                    "mDNS: advertising {} on port {}",
                    adv.fullname(),
                    config.port
                );
                Some(adv)
            }
            Err(e) => {
                eprintln!("mDNS advertisement failed (continuing without): {e:#}");
                None
            }
        }
    };

    println!("ProductionDeck Cora emulator");
    println!("  profile:  {:?}", config.profile);
    println!("  listen:   {}", server.local_addr());
    println!("  serial:   {}", config.serial);
    println!("  firmware: {}", config.firmware);
    println!("  mac:      {}", format_mac(&config.mac));
    println!("Commands:");
    println!("  press <key> / release <key> / tap <key>");
    println!("  encoder_press <idx> / encoder_release <idx> / encoder_rotate <idx> <delta>");
    println!("  nfc <hex> / status / quit");

    let mut render = config.render_out.as_ref().map(|_| VirtualStudio::new());
    let mut lines = BufReader::new(tokio::io::stdin()).lines();
    loop {
        tokio::select! {
            Some(command) = command_rx.recv() => {
                handle_command(&command, &config, &server, &mut render);
            }
            line = lines.next_line() => {
                match line? {
                    Some(line) => {
                        if !handle_line(&line, model, &server).await? {
                            break;
                        }
                    }
                    // Keep the daemon alive when stdin is closed (background use).
                    None => std::future::pending::<()>().await,
                }
            }
            _ = tokio::signal::ctrl_c() => break,
        }
    }

    Ok(())
}

fn make_identity(config: &EmulatorConfig, model: &'static ModelInfo) -> Identity<'static> {
    let serial = leak(config.serial.clone());
    let child = match config.profile {
        Profile::Studio => None,
        Profile::NetworkDock => Some(ChildInfo {
            connected: true,
            vid: 0x0fd9,
            pid: model::MK2.pid,
            serial: leak("A00SA5202LJPCY".to_string()),
            port: config.port.saturating_add(1),
            model_name: "Stream Deck MK.2",
            key_w: 72,
            key_h: 72,
            rows: 3,
            cols: 5,
            layout_type: 2,
        }),
    };
    Identity {
        model,
        serial,
        mac: config.mac,
        versions: match config.profile {
            Profile::Studio => FirmwareVersions {
                ap2: leak(config.firmware.clone()),
                ap1: leak("1.00.000".to_string()),
                ld: leak("1.00.000".to_string()),
                // Captured from a real Studio (fw 2.01.002).
                encoder_ap2: leak("1.01.012".to_string()),
                encoder_ld: leak("1.01.006".to_string()),
            },
            Profile::NetworkDock => FirmwareVersions {
                ap2: leak(config.firmware.clone()),
                ap1: leak(config.firmware.clone()),
                ld: leak(config.firmware.clone()),
                encoder_ap2: leak(config.firmware.clone()),
                encoder_ld: leak(config.firmware.clone()),
            },
        },
        child,
    }
}

fn handle_command(
    command: &DeckCommand,
    config: &EmulatorConfig,
    _server: &CoraServer,
    render: &mut Option<VirtualStudio>,
) {
    match command {
        DeckCommand::KeyImage { key, jpeg } => {
            println!("rx key image key={key} bytes={}", jpeg.len())
        }
        DeckCommand::PanelImage { x, y, w, h, jpeg } => {
            println!("rx panel image {w}x{h}@{x},{y} bytes={}", jpeg.len())
        }
        DeckCommand::FullScreen { panel, jpeg } => {
            println!("rx fullscreen panel={panel} bytes={}", jpeg.len())
        }
        DeckCommand::Window { jpeg } => println!("rx window image bytes={}", jpeg.len()),
        DeckCommand::Background { index, jpeg } => {
            println!("rx background {index} bytes={}", jpeg.len())
        }
        DeckCommand::Brightness(v) => println!("rx brightness {v}%"),
        DeckCommand::KeyRgb { key, r, g, b } => println!("rx key {key} rgb=({r},{g},{b})"),
        DeckCommand::FillLcd { r, g, b } => println!("rx fill lcd ({r},{g},{b})"),
        DeckCommand::ClearPanel(panel) => println!("rx clear panel {panel}"),
        DeckCommand::EncoderRing { enc, rgb } => {
            println!("rx encoder ring {enc} bytes={}", rgb.len())
        }
        DeckCommand::EncoderLed { enc, r, g, b } => {
            println!("rx encoder led {enc} rgb=({r},{g},{b})")
        }
        DeckCommand::Reset => println!("rx reset"),
        DeckCommand::Sleep(seconds) => println!("rx sleep {seconds}s"),
    }

    if let Some(panel) = render {
        panel.apply(command);
        if let Some(path) = &config.render_out {
            if let Err(e) = panel.write_ppm(path) {
                eprintln!("render write failed: {e}");
            }
        }
    }
}

async fn handle_line(line: &str, model: &'static ModelInfo, server: &CoraServer) -> Result<bool> {
    let parts: Vec<&str> = line.split_whitespace().collect();
    if parts.is_empty() {
        return Ok(true);
    }
    let key_count = model.key_count();
    match parts[0] {
        "press" | "release" | "tap" => {
            let Some(key) = parse_u8(&parts, 1) else {
                println!("usage: {} <key>", parts[0]);
                return Ok(true);
            };
            if key as usize >= key_count {
                println!("invalid key {key} (0..{})", key_count.saturating_sub(1));
                return Ok(true);
            }
            let pressed = parts[0] != "release";
            server.input(InputEvent::Button {
                index: key,
                pressed,
            });
            if parts[0] == "tap" {
                tokio::time::sleep(std::time::Duration::from_millis(50)).await;
                server.input(InputEvent::Button {
                    index: key,
                    pressed: false,
                });
            }
        }
        "encoder_press" | "encoder_release" => {
            let Some(index) = parse_u8(&parts, 1) else {
                println!("usage: {} <idx>", parts[0]);
                return Ok(true);
            };
            if index >= model.encoder_count {
                println!("invalid encoder {index}");
                return Ok(true);
            }
            server.input(InputEvent::EncoderPress {
                index,
                pressed: parts[0] == "encoder_press",
            });
        }
        "encoder_rotate" => {
            let index = parse_u8(&parts, 1);
            let delta = parts.get(2).and_then(|s| s.parse::<i8>().ok());
            match (index, delta) {
                (Some(index), Some(delta)) if index < model.encoder_count => {
                    server.input(InputEvent::EncoderRotate { index, delta });
                }
                _ => println!("usage: encoder_rotate <idx> <delta>"),
            }
        }
        "nfc" => {
            let Some(hex) = parts.get(1) else {
                println!("usage: nfc <hex>");
                return Ok(true);
            };
            let bytes = parse_hex(hex)?;
            server.input(InputEvent::nfc(&bytes));
        }
        "status" => println!("listening on {}", server.local_addr()),
        "help" => println!("press/release/tap/encoder_*/nfc/status/quit"),
        "quit" | "exit" => return Ok(false),
        other => println!("unknown command: {other}"),
    }
    Ok(true)
}

fn parse_u8(parts: &[&str], index: usize) -> Option<u8> {
    parts.get(index)?.parse().ok()
}

fn parse_hex(value: &str) -> Result<Vec<u8>> {
    let value = value.trim_start_matches("0x");
    if !value.len().is_multiple_of(2) {
        bail!("hex string must have an even number of digits");
    }
    let mut out = Vec::with_capacity(value.len() / 2);
    for i in (0..value.len()).step_by(2) {
        out.push(u8::from_str_radix(&value[i..i + 2], 16)?);
    }
    Ok(out)
}

fn format_mac(mac: &[u8; 6]) -> String {
    format!(
        "{:02X}:{:02X}:{:02X}:{:02X}:{:02X}:{:02X}",
        mac[0], mac[1], mac[2], mac[3], mac[4], mac[5]
    )
}

fn leak(value: String) -> &'static str {
    Box::leak(value.into_boxed_str())
}

/// Parse a colon-separated MAC address.
pub fn parse_mac(value: &str) -> Result<[u8; 6]> {
    let parts: Vec<&str> = value.split(':').collect();
    if parts.len() != 6 {
        bail!("MAC address must have 6 colon-separated octets");
    }
    let mut mac = [0u8; 6];
    for (i, part) in parts.iter().enumerate() {
        mac[i] =
            u8::from_str_radix(part, 16).with_context(|| format!("invalid MAC octet {part}"))?;
    }
    Ok(mac)
}
