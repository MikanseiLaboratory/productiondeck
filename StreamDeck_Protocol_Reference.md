# Stream Deck USB HID — ProductionDeck reference

This project follows the **Elgato Stream Deck HID API** documentation:

- [General Reference (Main / Expanded protocol)](https://docs.elgato.com/streamdeck/hid/general)
- [Stream Deck Mini (Legacy protocol)](https://docs.elgato.com/streamdeck/hid/mini)

There are **two protocol families**:

| Family | Devices (examples) | Notes |
|--------|-------------------|--------|
| **Legacy / Mini** | Mini `0x0063`, Mini 2022 `0x0090`, Mini Discord `0x00B3`, 6-key module `0x00B8` | Different report IDs and image path (BMP); see Mini HID page |
| **Main / Expanded** | Classic `0x006D`, Mk.2 `0x0080`, XL `0x006C` / `0x008F`, Neo `0x009A`, + `0x0084`, 15/32-key modules `0x00B9`/`0x00BA`, … | JPEG chunks, input report header `RID + cmd + UINT16 len + payload`; General Reference |

**PID 0x0060** (first-gen Stream Deck): not documented under the Main protocol; firmware may still expose it as legacy V1-style for compatibility.

## ProductionDeck `Device` → USB PID (VID `0x0FD9`)

| Variant | PID | Protocol module |
|---------|-----|-------------------|
| Mini | `0x0063` | `src/protocol/v1.rs` |
| RevisedMini (Mini 2022) | `0x0090` | V1 |
| MiniDiscord | `0x00B3` | V1 |
| Original | `0x0060` | V1 (non-doc) |
| OriginalV2 (Classic 2019) | `0x006D` | `src/protocol/v2.rs` |
| Mk2 | `0x0080` | V2 |
| Mk2ScissorKeys | `0x00A5` | V2 |
| Xl | `0x006C` | V2 |
| Xl2022 | `0x008F` | V2 |
| Plus | `0x0084` | V2 |
| PlusXl | `0x0084` | V2 |
| Neo | `0x009A` | V2 |
| Module6Keys | `0x00B8` | `src/protocol/module_6.rs` |
| Module15Keys | `0x00B9` | V2 (same as Classic) |
| Module32Keys | `0x00BA` | V2 (same as XL) |

Plus and + XL share PID `0x0084` in Elgato’s HID summary table; this firmware distinguishes them by build target (`plus` vs `plus-xl`) and [`config::init_runtime_device`](src/config.rs).

## Main protocol — implemented highlights (`v2.rs`)

- **Input (buttons)**: `[0x01, cmd=0x00, len_lo, len_hi, …key bytes]` (`format_button_report`).
- **Output `0x02`**: `0x07` key JPEG, `0x08` full LCD, `0x0B` / `0x0C` window (when device supports), `0x0D` background (Classic/XL family).
- **Feature setter `0x03`**: `0x02` reset, `0x05` fill LCD RGB, `0x06` fill key RGB, `0x08` brightness, `0x0D` sleep seconds, `0x13` show background (XL).
- **Feature getters**: `0x04`/`0x05`/`0x07` firmware, `0x06` serial, `0x08` unit info (from `Device::unit_information_tail()`), `0x0A` sleep duration.
- **Helpers** (for future touch/encoder wiring): `V2Handler::format_input_touch_tap`, `V2Handler::format_input_encoder_rotate`.

## Firmware binaries (`src/bin/`)

| Binary | `Device` |
|--------|----------|
| `mini`, `revised-mini` | Mini / Mini 2022 |
| `original`, `original-v2` | Original / Classic 2019 |
| `mk2` | Mk.2 |
| `xl`, `plus`, `neo`, `plus-xl` | XL / + / Neo / + XL |
| `module6`, `module15`, `module32` | Modules |

Use `ProtocolHandler::create_for_device(device)` so V2 unit information and image rules match the selected `Device` (`src/usb.rs`).
