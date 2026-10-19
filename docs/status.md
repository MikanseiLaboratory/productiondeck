# Implementation status

Last updated: 2026-10-19

| Area | Status | Evidence |
|---|---|---|
| Workspace (`firmware`, `core`, `cora`, `verify`) | done | `cargo metadata`, CI |
| Studio model (`0fd9:00aa`, 16x2, 144x112, 2 encoders, NFC, 2 panels) | done | `crates/core/src/model.rs` |
| Gen2 HID (input 512, output 1024, feature 32) | done | `hid_descriptor.rs`, real hardware |
| Feature reports (USB + Cora primary + legacy) | done | capture in `docs/studio-protocol.md` |
| Cora frame/session (bitflags, keep-alive/ACK, both request styles) | done | `crates/core/src/cora/*`, 31 tests |
| Legacy 512/1024 framing | done | `crates/cora/src/legacy.rs`, real Studio |
| mDNS advertise/discover | done | `crates/cora/src/discovery.rs`, `cora-probe --discover` |
| Emulator (Studio, Network Dock, legacy) | done | `cora-emulator`, `cora-dock-emulator`, `cora-legacy-emulator` |
| Wireshark dissectors | done | `tools/wireshark/cora.lua` on reference pcap |
| Studio USB firmware | done | `firmware/src/bin/studio.rs`, real hardware |
| Cora transport glue (`run_cora`, TCP 5343) | done | `firmware/src/network/*` |
| W5500 driver init + DHCP | done | `network/w5500.rs` for W5500-EVB-Pico (SPI0 GP16-21), `studio-cora` bin |
| CYW43 driver init | pending | entry point exists |`n| Firmware mDNS responder | pending | — |
| Network Dock hardware E2E | pending | waiting for dock |
| integrationdeck adapter | pending | `docs/integrationdeck.md` |

## Verification commands

```bash
cargo test -p productiondeck-core -p productiondeck-cora
cargo clippy -p productiondeck-core -p productiondeck-cora -p productiondeck-verify --all-targets -- -D warnings
cargo check -p productiondeck-firmware --target thumbv6m-none-eabi --features cora-net --bins
cargo run -p productiondeck-cora --bin cora-probe -- <ip> --legacy
```

## Pull requests

- productiondeck: https://github.com/MikanseiLaboratory/productiondeck/pull/61
- rust-streamdeck: https://github.com/ryankurte/rust-streamdeck/pull/43

