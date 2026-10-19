//! Emulate a Network Dock plus an attached Stream Deck XL child.
//!
//! Advertises two `_elg._tcp` services, matching a real dock capture:
//!   dt=215 (dock, port 5343) + dt=213 (child, port 5344)
//!
//! Usage: cargo run -p productiondeck-cora --bin cora-dock-emulator

use std::time::Duration;

use anyhow::Result;
use productiondeck_cora::command::DeckCommand;
use productiondeck_cora::core::feature::{ChildInfo, FirmwareVersions, Identity};
use productiondeck_cora::core::model;
use productiondeck_cora::discovery::{Advertisement, DiscoveryConfig};
use productiondeck_cora::server::CoraServer;
use tokio::sync::mpsc;

const PARENT_PORT: u16 = 5343;
const CHILD_PORT: u16 = 5344;

fn main() -> Result<()> {
    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?
        .block_on(run())
}

async fn run() -> Result<()> {
    let (tx, mut rx) = mpsc::unbounded_channel::<DeckCommand>();
    let parent_identity = Identity {
        model: &model::NETWORK_DOCK,
        serial: "EMULATORDOCK1",
        mac: [0x02, 0, 0, 0, 0, 1],
        versions: FirmwareVersions {
            ap2: "1.01.013",
            ap1: "1.01.013",
            ld: "1.01.013",
            encoder_ap2: "1.01.013",
            encoder_ld: "1.01.013",
        },
        child: Some(ChildInfo {
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
            layout_type: 2,
        }),
    };
    let child_identity = Identity {
        model: &model::XL,
        serial: "EMULATORXL0001",
        mac: [0x02, 0, 0, 0, 0, 2],
        versions: FirmwareVersions {
            ap2: "3.00.000",
            ap1: "3.00.000",
            ld: "3.00.000",
            encoder_ap2: "3.00.000",
            encoder_ld: "3.00.000",
        },
        child: None,
    };

    let parent = CoraServer::bind(
        ("0.0.0.0", PARENT_PORT),
        &model::NETWORK_DOCK,
        parent_identity,
        tx.clone(),
    )
    .await?;
    let child = CoraServer::bind(
        ("0.0.0.0", CHILD_PORT),
        &model::XL,
        child_identity,
        tx,
    )
    .await?;

    let _parent_adv = Advertisement::start(&DiscoveryConfig {
        instance: "Stream Deck Network Dock (emu)".to_string(),
        port: PARENT_PORT,
        serial: "EMULATORDOCK1".to_string(),
        model_name: "Stream Deck Network Dock".to_string(),
        firmware: "1.01.013".to_string(),
        dt: "215".to_string(),
        vid: "0x0FD9".to_string(),
        pid: "0xFFFF".to_string(),
        hwrev: "0200".to_string(),
        prt: Some("2".to_string()),
        mac: Some("02:00:00:00:00:01".to_string()),
    })?;
    let _child_adv = Advertisement::start(&DiscoveryConfig {
        instance: "Stream Deck XL (emu)".to_string(),
        port: CHILD_PORT,
        serial: "EMULATORXL0001".to_string(),
        model_name: "Stream Deck XL".to_string(),
        firmware: "3.00.000".to_string(),
        dt: "213".to_string(),
        vid: "0x0FD9".to_string(),
        pid: "0x006C".to_string(),
        hwrev: "0200".to_string(),
        prt: None,
        mac: Some("02:00:00:00:00:02".to_string()),
    })?;

    println!("dock:  {}", parent.local_addr());
    println!("child: {}", child.local_addr());
    println!("advertising dt=215 (dock) and dt=213 (XL child)");

    let commands = tokio::spawn(async move {
        while let Some(command) = rx.recv().await {
            match command {
                DeckCommand::KeyImage { key, jpeg } => {
                    println!("child rx key image key={key} bytes={}", jpeg.len())
                }
                DeckCommand::Brightness(v) => println!("rx brightness {v}%"),
                other => println!("rx {other:?}"),
            }
        }
    });

    tokio::select! {
        _ = tokio::signal::ctrl_c() => {}
        _ = commands => {}
        _ = tokio::time::sleep(Duration::from_secs(3600)) => {}
    }
    Ok(())
}
