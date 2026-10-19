//! Cora emulator CLI.

use std::path::PathBuf;

use anyhow::Result;
use clap::{Parser, ValueEnum};
use productiondeck_cora::emulator::{self, EmulatorConfig, Profile};

#[derive(Debug, Clone, Copy, ValueEnum)]
enum ProfileArg {
    Studio,
    NetworkDock,
}

#[derive(Debug, Parser)]
#[command(version, about = "Stream Deck Studio / Network Dock Cora emulator")]
struct Cli {
    /// Address to bind
    #[arg(long, default_value = "0.0.0.0")]
    bind: String,
    /// TCP port (Cora default 5343)
    #[arg(long, default_value_t = 5343)]
    port: u16,
    /// Device serial number
    #[arg(long, default_value = "PRODUCTIONDK01")]
    serial: String,
    /// MAC address, colon separated
    #[arg(long, default_value = "02:00:00:00:00:01")]
    mac: String,
    /// Firmware version string
    #[arg(long, default_value = "1.00.000")]
    firmware: String,
    /// mDNS instance name
    #[arg(long, default_value = "Stream Deck Studio")]
    mdns_name: String,
    /// Disable mDNS advertisement
    #[arg(long)]
    no_mdns: bool,
    /// Emulated profile
    #[arg(long, value_enum, default_value_t = ProfileArg::Studio)]
    profile: ProfileArg,
    /// Override the mDNS dt TXT value
    #[arg(long)]
    dt: Option<String>,
    /// Write a composited 2304x224 PPM after each image command
    #[arg(long)]
    render_out: Option<PathBuf>,
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    let cli = Cli::parse();
    let config = EmulatorConfig {
        bind: cli.bind,
        port: cli.port,
        serial: cli.serial,
        mac: emulator::parse_mac(&cli.mac)?,
        firmware: cli.firmware,
        mdns_name: cli.mdns_name,
        no_mdns: cli.no_mdns,
        profile: match cli.profile {
            ProfileArg::Studio => Profile::Studio,
            ProfileArg::NetworkDock => Profile::NetworkDock,
        },
        dt_override: cli.dt,
        render_out: cli.render_out,
    };
    emulator::run(config).await
}
