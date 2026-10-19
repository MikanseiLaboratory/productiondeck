# integrationdeck integration notes

If the official Stream Deck app does not support Stream Deck Studio over LAN,
integrationdeck is the better host. The useful boundary is
`productiondeck-cora` (Rust, std), which already contains the transport and
protocol work.

## Reusable pieces

| API | Purpose |
|---|---|
| `discovery::discover` / `Advertisement` | `_elg._tcp` browse/advertise |
| `client::CoraClient` | Cora client + handshake + feature GET |
| `legacy::LegacyClient` | Studio fw 2.01.002 legacy 512/1024 client |
| `emulator` / `cora-legacy-emulator` | test devices without hardware |
| `command::DeckCommand` | decoded key image / LED / brightness commands |
| `core` | model info, Gen2 input/output, feature builders |

## Suggested facade

Add a `StudioDevice` in integrationdeck (or in this crate) that:

1. Connects to the mDNS address.
2. Detects framing: Cora magic `43 93 8A 41` vs legacy `01 0a`/`01 0b`.
3. Queries identity: 0x80/0x83/0x84/0x85 and 0x1C (child).
4. Exposes:
   - `brightness(u8)` (`03 08`)
   - `key_image(key, jpeg)` (Gen2 `02 07` chunks, or legacy 1024-byte writes)
   - `encoder_ring` / `encoder_led` (`02 0F` / `02 10`)
   - `child()` (XL over port 20001)
   - an input stream (buttons/encoders/NFC) from the device's 512-byte input reports
5. Reconnects on the 5 s timeout / keep-alive loss.

The transport layer stays in `productiondeck-cora`; integrationdeck only maps
`DeckCommand`/`InputEvent` into its own device-action model and plugin API.

## Network Dock next

A real Network Dock is the cleanest Cora validation:

- `dt=215` dock + `dt=213` child service
- Cora magic framing on 5343 / 20001 (per the reference pcap)
- `0x1C` child info and the dynamic child port

Once it is on the LAN, run:

```bash
cargo run -p productiondeck-cora --bin cora-probe -- --discover
cargo run -p productiondeck-cora --bin cora-probe -- <dock-ip>
```

and compare against `cora-dock-emulator`.
