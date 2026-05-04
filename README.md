# ProductionDeck

RP2040-based StreamDeck compatible firmware.

## Prerequisites

```bash
rustup target add thumbv6m-none-eabi
cargo install elf2uf2-rs flip-link
```

## Building

```bash
cargo build --release --bin module6
```

Other devices: `original`, `xl`, `plus`, `module6`, etc.

UF2 files: `target/thumbv6m-none-eabi/release/<device-name>.uf2`

## Flashing

1. Hold BOOTSEL button, connect USB, release BOOTSEL
2. Copy `.uf2` file to `RPI-RP2` drive

## Documents

- https://docs.elgato.com/streamdeck/hid/intro


## License

MIT
