# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with the ProductionDeck repository.

## Project Overview

ProductionDeck is an open-source RP2040-based Stream Deck–compatible firmware in Rust (Embassy). Multiple **device profiles** (Mini, Classic/Mk.2, XL, Neo, +, modules, etc.) share the same codebase; each profile selects USB PID, layout, and **Legacy (Mini)** vs **Main/Expanded** HID handling. Physical hardware is often a small key matrix + one ST7735 region unless you build a larger layout.

**Current Status**: Alpha - Firmware compiles successfully, ready for hardware testing.

## Quick Start Commands

### Essential Build Commands
```bash
# Check compilation without building
cargo check

# Build in debug mode
cargo build

# Build in release mode (recommended for embedded)
cargo build --release

# Format code
cargo fmt

# Run linter
cargo clippy
```

### Development Commands
```bash
# Clean build artifacts
cargo clean

# Update dependencies (use with caution)
cargo update

# Generate documentation
cargo doc --open
```

## Prerequisites

1. **Rust Toolchain** - Latest stable Rust (1.75+ recommended)
2. **thumbv6m-none-eabi target** - `rustup target add thumbv6m-none-eabi`
3. **elf2uf2-rs** - `cargo install elf2uf2-rs` (for UF2 conversion)
4. **flip-link** - `cargo install flip-link` (stack overflow protection)

## Project Structure

### Core Source Files
- `src/bin/*.rs` - One binary per target device (`module6`, `revised-mini`, `xl`, `mk2`, `neo`, `plus-xl`, …)
- `src/lib.rs` - Library root (`productiondeck` crate)
- `src/config.rs` - Hardware configuration constants and pin assignments
- `src/device/mod.rs` - USB PID, layout, and protocol family per `Device`
- `src/protocol/v1.rs` / `v2.rs` / `module_6.rs` - HID protocol handlers
- `src/usb.rs` - USB HID class and routing to protocol + channels
- `src/display.rs` - Display handling and graphics rendering
- `src/buttons.rs` - Button matrix / direct scanning

### Configuration Files
- `Cargo.toml` - Rust project manifest and dependencies
- `.cargo/config.toml` - Build configuration and target settings
- `memory.x` - RP2040 memory layout for linker
- `build.rs` - Build script for memory layout

### Documentation
- `README.md` - Main project documentation
- `LICENSE` - MIT License
- `StreamDeck_Protocol_Reference.md` - Protocol documentation (Elgato HID API alignment)

## Architecture Overview

### Hardware Configuration
- **Target**: Raspberry Pi Pico (RP2040 dual-core ARM Cortex-M0+)
- **USB Identity**: VID `0x0FD9` (Elgato); PID depends on selected `Device` (see `src/device/mod.rs`)
- **Display**: Often 1× ST7735; layout/size depend on profile (`display_config`)
- **Buttons**: Matrix or direct wiring per `hardware.rs` / `buttons.rs`
- **Protocol**: Legacy Mini family (`v1`) or Main/Expanded family (`v2`) per Elgato HID docs

### Core Architecture
- **Async Embassy framework**: Modern Rust async/await for embedded
- **Dual-core design**: Core 0 handles USB/protocol, Core 1 handles displays/buttons
- **USB HID interface**: Stream Deck Mini (legacy) or Main protocol (JPEG + feature reports) per build
- **Channel communication**: Embassy channels for inter-task communication
- **Hardware abstraction**: Configurable pin assignments via `config.rs`

### Pin Assignments (Critical for Hardware)
```
RP2040 Pin Layout:
├── Buttons (Matrix Scan):
│   ├── ROW0: GP2  ┐
│   ├── ROW1: GP3  ├─ Button Matrix (3x2)
│   ├── COL0: GP4  │
│   ├── COL1: GP5  │
│   └── COL2: GP6  ┘
│
├── SPI Display (ST7735):
│   ├── MOSI: GP19 (Data)
│   ├── SCK:  GP18 (Clock)
│   ├── DC:   GP14 (Data/Command)
│   ├── RST:  GP15 (Reset)
│   ├── CS:   GP8  (Chip Select)
│   └── BLK:  GP17 (Backlight PWM)
│
├── Control:
│   ├── Status LED: GP25 (Built-in)
│   ├── USB LED:    GP20 (Connection status)
│   └── Error LED:  GP21 (Error indication)
│
└── Debug:
    ├── UART TX: GP0 (Debug output)
    └── UART RX: GP1 (Debug input)
```

## USB Protocol Implementation

The device implements StreamDeck Mini's exact USB HID protocol:
- **Input reports**: Button states (6 bytes for 6 keys)
- **Output reports**: Image data packets (1024 bytes)
- **Feature reports**: Commands (brightness, reset, version)
- **Embassy USB stack** with usbd-hid for HID functionality

### Key Protocol Details
- **VID**: 0x0fd9 (Elgato Systems)
- **PID**: 0x0063 (StreamDeck Mini)
- **Class**: HID (Human Interface Device)
- **Image format**: 80x80 pixels, RGB565 color format

## Dependencies

### Core Framework
- `embassy-rp` - RP2040 hardware abstraction
- `embassy-usb` - USB stack implementation
- `embassy-time` - Time and timer functionality
- `embassy-executor` - Async executor
- `embassy-sync` - Synchronization primitives

### Hardware Abstraction
- `embedded-hal` / `embedded-hal-async` - Hardware abstraction layer
- `embedded-hal-bus` - Bus abstraction for SPI

### Display and Graphics
- `st7735-lcd` - ST7735 display driver
- `embedded-graphics` - Graphics primitives

### USB HID
- `usbd-hid` - USB HID device implementation

### Utilities
- `heapless` - No-alloc data structures
- `defmt` - Debug formatting
- `defmt-rtt` - RTT transport for debug output

## Development Notes

### Build Configuration
- **Target**: thumbv6m-none-eabi (Cortex-M0+)
- **Linker**: flip-link for stack overflow protection
- **Debug**: RTT (Real-Time Transfer) with defmt logging
- **Optimization**: Release builds use size optimization (-Os)

### Debug Levels
Debug output is controlled via the `DEFMT_LOG` environment variable:
- Set to `debug` for detailed logging
- Set to `info` for basic information
- Set to `warn` for warnings only
- Set to `off` to disable logging

### Memory Layout
- Memory layout defined in `memory.x` for RP2040
- Boot2 section properly configured
- Stack overflow protection enabled

### Current Status
- **Version**: 0.1.0
- **Status**: Alpha - Code compiles successfully
- **Build**: ✅ Compilation working
- **Hardware**: Manual wiring required (no PCB design yet)
- **Testing**: Ready for hardware validation

## Common Tasks for Claude

### When Adding Features
1. Check existing code structure in relevant modules
2. Follow async/await patterns established in the codebase
3. Use Embassy channels for inter-task communication
4. Update pin assignments in `config.rs` if needed
5. Test with `cargo check` before committing

### When Debugging
1. Use `defmt` logging for debug output
2. Check UART output on GP0/GP1 for debug messages
3. Verify USB VID/PID match StreamDeck Mini exactly
4. Test button matrix wiring and SPI connections

### When Modifying Hardware Configuration
1. Update pin assignments in `src/config.rs`
2. Ensure pin conflicts are avoided
3. Test with `cargo check` to verify compilation
4. Update documentation if pin layout changes

## Important Notes

### Critical Configuration
All USB descriptors and protocol handling must exactly match StreamDeck Mini for software compatibility. Changes to VID/PID or protocol structure will break compatibility with official StreamDeck software.

### Hardware Requirements
- Raspberry Pi Pico (RP2040)
- 1x ST7735 TFT display (80x80 pixels)
- 6x tactile switches
- Basic passive components (resistors, capacitors)
- Optional: Status LEDs, enclosure

### Legal Notice
This project is not affiliated with Elgato Systems. StreamDeck is a trademark of Elgato Systems. This project implements a compatible device through reverse engineering for educational and interoperability purposes.