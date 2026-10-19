# Stream Deck Studio protocol notes

Facts marked **[captured]** were observed on a real Stream Deck Studio
(firmware 2.01.002, serial `A9IJA53330GUMR`, USB `0FD9:00AA`, bcdDevice 4.12)
on 2026-10-19. Facts marked **[reference]** come from
`node-elgato-stream-deck`, `Haukcode.StreamDeck` and a real Network Dock
`pcapng`. Anything marked **[open]** still needs a capture.

## USB HID

### Device identity [captured]

| Field | Value |
|---|---|
| VID:PID | `0fd9:00aa` |
| bcdDevice | `0x0412` |
| Product string | `Stream Deck Studio` |
| Serial string | `A9IJA53330GUMR` |
| Firmware (0x05) | `2.01.002` |
| App model code | `20GBO9901` |

### HID report lengths [captured]

`HidP_GetCaps` on the consumer-control interface:

| Report | Bytes |
|---|---|
| Input | **512** |
| Output | **1024** |
| Feature | **32** |
| Input button caps | 0 |
| Input value caps | 1 |
| Feature button caps | 19 |

The Studio descriptor differs from the smaller Gen2 decks (48-byte input):
`productiondeck-core/src/hid_descriptor.rs` has a dedicated
`STUDIO_HID_DESCRIPTOR` with a 512-byte input report.

### Feature reports [captured]

All values below are the first bytes read with `HidD_GetFeature` (32-byte
feature report).

| Report | Bytes (hex) | Meaning |
|---|---|---|
| `0x05` | `05 0c a2 cc 09 b5 32 2e 30 31 2e 30 30 32` | AP2 firmware `2.01.002`, checksum `a2cc09b5` |
| `0x06` | `06 0e 41 39 49 4a 41 35 33 33 33 30 47 55 4d 52` | serial length 14 + ASCII |
| `0x08` | `08 02 10 90 00 70 00 90 05 18 01 02 00 00 00 04 ff` | rows=2, cols=16, key 144x112, LCD 1424x280, `bpp=2`, trailer `00 00 04 ff` |
| `0x11` | `11 18 31 2e 30 31 2e 30 31 32 93 7c 27 00 31 2e 30 31 2e 30 31 32 93 7c 27 00` | encoder AP2 `1.01.012`, two `version+checksum` records |
| `0x13` | `13 18 31 2e 30 31 2e 30 30 36 7a 4d 19 00 31 2e 30 31 2e 30 30 36 7a 4d 19 00` | encoder LD `1.01.006` |
| `0x1a` | `1a 0c 31 2e 30 30 2e 30 30 30 5c eb 15 00` | LD firmware `1.00.000`, checksum `5ceb1500` |

`0x80`, `0x83`, `0x84`, `0x85`, `0x86`, `0x8a`, `0x8f` and `0x1c` are **not**
declared as USB feature reports (they are Cora-only) and fail with
`HidD_GetFeature` on USB. **[captured]**

### Gen2 output commands

| Command | Layout |
|---|---|
| `0x02 0x07` | key, last, size16, chunk16, JPEG |
| `0x02 0x0c` | x16, y16, w16, h16, last, chunk16, size16, pad, JPEG |
| `0x02 0x0f` | encoder, 24 x RGB |
| `0x02 0x10` | encoder, R, G, B |
| `0x03 0x05` | Studio: panel index (clear one of two panels) |
| `0x03 0x06` | key, R, G, B |
| `0x03 0x08` | brightness 0..100 |
| `0x03 0x0d` | sleep seconds |

### Gen2 input reports

| Command | Layout |
|---|---|
| `0x01 0x00` | `len16` + button states |
| `0x01 0x03` | `len16` + subtype (`0` press, `1` rotate i8) + per-encoder value |
| `0x01 0x04` | `len16` + NFC UID |

The Studio input report is 512 bytes on the wire; short reports should be padded
to the declared size.

## Cora over TCP

### Frame [reference + captured]

```text
0..3   magic 43 93 8A 41
4..5   flags (u16 LE)
6      hidOp
7      reserved
8..11  messageId (u32 LE)
12..15 payloadLen (u32 LE)
16..   payload
```

Flags: `VERBATIM=0x8000`, `REQ_ACK=0x4000`, `ACK_NAK=0x0200`,
`RESULT=0x0100`. Always a bitfield.

### Network Dock reference capture [reference]

From `Haukcode.StreamDeck` `docs/Stream Deck Network Dock.pcapng`:

- mDNS TXT for the dock: `dt=215, mf=Elgato, md=..., pv=1.0, id=<MAC>, sn=...,
  fw=1.01.013, hwrev=0200`; child instance `dt=213, vid=0x0FD9, pid=0x0080,
  fw=1.02.000, prt=2`.
- Primary port 5343, child port 20001 (reported in the capabilities payload).
- Keep-alive payload starts `01 0a 02 00 01 <conn>` and is sent every ~2 s.
- Image write on the child port:
  `flags=0xc000 (REQ_ACK|VERBATIM)`, payload `02 07 00 01 <size16> <chunk16> <jpeg>`;
  the ACK is `flags=0x8200 (VERBATIM|ACK_NAK)` with a 4-byte zero payload and
  the same message id.
- Capabilities payload (`01 0b`): rows at 5, cols at 6, key width/height at
  8/10, VID/PID at 26/28, model at 62, serial at 94, child port at 126.

Use `tools/wireshark/cora.lua` to decode this capture directly.

### Studio Cora [open]

The real Studio was attached over USB but **no Cora listener was reachable on
the local `192.168.3.0/24` segment** during this session:

- mDNS browse (`dns-sd -B _elg._tcp`) returned nothing.
- A capture on the active NIC showed only the workstation's own mDNS traffic.
- TCP 5343 was not open on any live ARP neighbour (ping sweep + port scan).
- `cora_seen.json` remained empty while the app used the USB connection.

Next capture steps:

1. Confirm the Studio's Ethernet link and DHCP address (router client list or
   the device's own setup UI).
2. If it is on another subnet, put the workstation on that subnet and repeat
   `cargo run -p productiondeck-cora --bin cora-probe -- --discover`.
3. Capture `udp port 5353 or tcp port 5343 or tcp port 5344` while the official
   app connects, then run `cora-probe <ip>` and/or
   `tools/wireshark/cora.lua`.

Until then, Studio-specific Cora values remain **[open]**:
mDNS `dt`/`prt`, the 0x80/0x1c layouts, the 0x1a semantics, panel image
commands, and the legacy (headerless) packet size.
