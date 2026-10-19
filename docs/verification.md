# Verification

The approved plan calls for verification against real hardware and the
`rust-streamdeck` crate (`streamdeck` on crates.io).

## Harness

```bash
cargo run -p productiondeck-verify --bin studio-verify -- streamdeck
cargo run -p productiondeck-verify --bin studio-verify -- hardware
cargo run -p productiondeck-verify --bin studio-verify -- all
```

- `streamdeck` uses the upstream `streamdeck` crate for the models it knows
  about (Mini/Original/Mk.2/XL/Plus/Modules), then explicitly tries Studio.
- `hardware` speaks to a real Studio through `hidapi`: it reads the captured
  feature reports, round-trips brightness, sends a 144x112 JPEG to key 0 and
  key 31, and reads input reports for two seconds.

## Results (real Studio, 2026-10-19)

Upstream `streamdeck` 0.10:

- `StreamDeck::probe()` returns `Err(UnrecognisedPID)` because PID `0x00AA`
  is not in its `Kind` table.
- `StreamDeck::connect(0x0fd9, 0x00aa, None)` returns
  `unrecognised pid`; the upstream crate needs a Studio `Kind` before it can be
  used for the Studio USB path. Our harness therefore uses the same `hidapi`
  foundation directly for Studio.

Direct HID (`-- hardware`) against firmware 2.01.002:

| Check | Result |
|---|---|
| product / serial | `Stream Deck Studio` / `A9IJA53330GUMR` |
| 0x05 | `050ca2cc09b5322e30312e303032…` (2.01.002, checksum `a2cc09b5`) |
| 0x06 | `060e4139494a41353333333047554d52…` (serial) |
| 0x08 | `08021090007000900518010200000004ff…` (2x16, 144x112, LCD 1424x280) |
| 0x11 / 0x13 | encoder `1.01.012` / `1.01.006` (two version+checksum records) |
| 0x1a | `1a0c312e30302e3030305ceb1500…` (LD 1.00.000) |
| brightness 50 → 80 | accepted |
| key image | 1071-byte JPEG written to key 0 and key 31 (1024-byte output reports) |
| input read | 2 s window opened without error |

## Remaining verification

1. **Firmware USB**: flash `studio` to an RP2040 and repeat `-- hardware`
   against the emulated device; compare feature reports and key image
   rendering with the real Studio.
2. **Firmware Cora**: flash `studio-cora` (W5500 / CYW43) and drive it with
   `cora-probe` plus `tools/wireshark/cora.lua`.
3. **Cora real device**: once the Studio's LAN address is known, run
   `cora-probe <ip>` and capture mDNS + TCP 5343.
4. **Emulator**: `cargo test -p productiondeck-cora` covers client/server
   feature exchange, keep-alive/ACK, image ACK and input broadcast.
5. **rust-streamdeck upstream**: a natural follow-up is adding a `Studio`
   `Kind` (PID 0x00AA, 16x2, 144x112, JPEG, offset 3, no flip) and encoder LED
   commands upstream so both projects share the verification client.
