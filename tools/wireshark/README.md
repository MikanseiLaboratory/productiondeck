# Wireshark tooling

## `cora.lua`

Dissector for Elgato's Cora framing (Stream Deck Studio / Network Dock).

- Registered on TCP 5343, 5344 and 20001 (observed Network Dock child port),
  plus a heuristic for dynamic child ports whose payload starts with the Cora
  magic.
- Fields use the `sdcora.*` prefix (the protocol display filter is `cora`).
- Decodes keep-alive, capabilities, feature responses (`0x80`, `0x83/0x86/0x8a`,
  `0x84`, `0x85`, `0x1c`), and output image/LED commands.

Usage:

```powershell
# GUI
wireshark -X lua_script:tools/wireshark/cora.lua

# CLI
tshark -X lua_script:tools/wireshark/cora.lua -r capture.pcapng -Y cora
tshark -X lua_script:tools/wireshark/cora.lua -r capture.pcapng `
  -Y cora -T fields -e sdcora.flags -e sdcora.hid_op -e sdcora.message_id `
  -e sdcora.payload_type -e sdcora.feature.id -e sdcora.image.key
```

Validated against
`Haukcode.StreamDeck/docs/Stream Deck Network Dock.pcapng`: the child-port
image write decodes as `flags=0xc000`, `msgId=0x7b6`, `key=0`, `size=770`, and
the ACK as `flags=0x8200`, same message id.

## `sdhid.lua`

Post-dissector for Stream Deck HID reports carried in `usbhid.data`. It
annotates report id, button state, key image chunks and encoder LED reports.
Enable it in the GUI under `Tools > Lua` or with
`-X lua_script:tools/wireshark/sdhid.lua`.

## Adding new fields

Keep field names lower-case and prefixed with `sdcora.` (multi-byte integers
must be displayed with `add_le`). After editing, run:

```powershell
tshark -X lua_script:tools/wireshark/cora.lua -r test.pcapng -Y cora -V
```

and confirm there is no `Lua Error` line.
