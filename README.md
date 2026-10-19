# ProductionDeck

RP2040-based Stream Deck–compatible firmware, plus a sans-I/O protocol core and
a host-side Cora emulator/probe.

## Workspace

| Path | Crate | Purpose |
|---|---|---|
| `firmware/` | `productiondeck-firmware` | Embassy RP2040 firmware and device binaries |
| `crates/core/` | `productiondeck-core` | `no_std` model definitions, Gen2 HID, Cora framing/session |
| `crates/cora/` | `productiondeck-cora` | host Cora client/server, mDNS, emulator, probe |
| `tools/wireshark/` | — | Cora + Studio HID Wireshark dissectors |
| `docs/` | — | protocol notes and capture instructions |

## Prerequisites

```bash
rustup target add thumbv6m-none-eabi
cargo install elf2uf2-rs flip-link
```

## Building firmware

The workspace root is not pinned to the embedded target:

```bash
cargo build --release -p productiondeck-firmware --target thumbv6m-none-eabi --bin mini
# or
cargo firmware-release
# or, from firmware/: cd firmware && cargo build --release --bin mini
```

Other devices: `original`, `xl`, `plus`, `mk2`, `neo`, `plus-xl`, `module6`,
`module15`, `module32`, and the new `studio` / `studio-cora` profiles.

UF2 files: `target/thumbv6m-none-eabi/release/<device-name>.uf2`

## Host crates

```bash
cargo test -p productiondeck-core -p productiondeck-cora
cargo run -p productiondeck-cora --bin cora-emulator -- --help
cargo run -p productiondeck-cora --bin cora-probe -- --discover
```

The Studio emulator advertises `_elg._tcp`, answers Cora feature requests and
accepts keyboard/encoder/NFC input from stdin. See `docs/studio-protocol.md`
for captured protocol facts and `docs/capture.md` for Wireshark instructions.

## Flashing

1. Hold BOOTSEL, connect USB, release BOOTSEL
2. Copy the `.uf2` file to the `RPI-RP2` drive

## Documents

- Protocol facts: `docs/studio-protocol.md`
- Capture workflow: `docs/capture.md`
- Wireshark dissectors: `tools/wireshark/README.md`
- Elgato HID API: https://docs.elgato.com/streamdeck/hid/intro

## License

MIT
