# Capturing Stream Deck Studio traffic

Tools available on this workstation:

- Wireshark / `tshark` 4.6.7 (`C:\Program Files\Wireshark`)
- USBPcap (`C:\Program Files\Wireshark\extcap\USBPcapCMD.exe`)
- Bonjour `dns-sd` (`C:\WINDOWS\system32\dns-sd.exe`)
- Official Stream Deck app (device log path
  `%APPDATA%\Elgato\StreamDeck\logs\StreamDeck.log`)

## Cora over TCP

1. Find the device:
   - `dns-sd -B _elg._tcp local.`
   - or `cargo run -p productiondeck-cora --bin cora-probe -- --discover`
   - or a port scan for 5343/5344 on the device's subnet.
2. Capture (replace the interface name):
   ```powershell
   & 'C:\Program Files\Wireshark\tshark.exe' -i "Ethernet 6" `
     -f "udp port 5353 or tcp port 5343 or tcp port 5344" `
     -w target\studio-cora.pcapng
   ```
3. Decode with the bundled dissector:
   ```powershell
   & 'C:\Program Files\Wireshark\tshark.exe' `
     -X lua_script:tools/wireshark/cora.lua `
     -r target\studio-cora.pcapng `
     -Y cora -T fields -e frame.number -e sdcora.flags -e sdcora.message_id `
     -e sdcora.payload_type -e sdcora.feature.id
   ```
   In the GUI, `Analyze > Decode As > TCP port 5343/5344 -> CORA` also works.
   Dynamic child ports can be added with `Decode As`; the dissector registers
   5343, 5344 and the observed 20001.
4. Export a normalized transcript for diffing:
   ```powershell
   & 'C:\Program Files\Wireshark\tshark.exe' `
     -X lua_script:tools/wireshark/cora.lua `
     -r target\studio-cora.pcapng -Y cora -T json > target\studio-cora.json
   ```

## USB HID (USBPcap)

1. The Studio enumerates as `USB\VID_0FD9&PID_00AA\<serial>` with a single
   consumer-control HID interface. USBPcap is already bound to the device stack
   on this machine.
2. Plug the device in **while** capturing so the report descriptor is included:
   ```powershell
   & 'C:\Program Files\Wireshark\extcap\USBPcapCMD.exe' `
     -d \\.\USBPcap1 -A --inject-descriptors `
     -o target\studio-usb.pcapng
   ```
   Use `-d \\.\USBPcap2` (etc.) if the device is on another root hub. Stop the
   capture with Ctrl-C after the app has connected.
3. Inspect the descriptor and reports:
   ```powershell
   & 'C:\Program Files\Wireshark\tshark.exe' -r target\studio-usb.pcapng `
     -Y "usb.idVendor == 0x0fd9 && usb.idProduct == 0x00aa" -V
   ```
4. The protocol-level facts can also be read directly through the Windows HID
   API without USBPcap. The one-off probe used during this session is recorded
   in `docs/studio-protocol.md`; it opens the interface, calls
   `HidP_GetCaps` and `HidD_GetFeature`, and prints the captured values
   (`input=512, output=1024, feature=32`, reports `0x05/0x06/0x08/0x11/0x13/0x1a`).

## What to capture next

- The Studio's **Cora** side: mDNS TXT, 5343 handshake, `0x80/0x08/0x1c`,
  `0x1a`, panel image commands and legacy framing.
- Official app **USB** traffic: key image writes, encoder rings, panel
  clear/fill, and every feature query.
- Network Dock comparison: `Haukcode.StreamDeck`'s
  `docs/Stream Deck Network Dock.pcapng` is a known-good Cora reference and is
  decoded by `tools/wireshark/cora.lua`.
