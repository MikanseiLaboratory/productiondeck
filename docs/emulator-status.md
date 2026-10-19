# Cora emulator status

## What works

- `cora-emulator --profile studio` advertises `_elg._tcp` and serves Cora on
  TCP 5343.
- `cora-probe --discover` resolves it (e.g. `192.168.3.44:5343`) with
  `dt/vid/pid/sn/prt` TXT keys.
- `cora-dock-emulator` advertises a two-service Network Dock layout
  (`dt=215` dock + `dt=213` XL child) and serves 5343 / 5344.
- `cora-probe <ip>` exercises feature reports `0x80/0x08/0x83/0x84/0x85/0x86/
  0x8a/0x8f/0x1a/0x1c` in both GET and official-app WRITE styles.

## Official app behaviour

The app discovers the emulator through mDNS:

```
DeviceDockManager mDNS added latent service
  'Stream Deck Network Dock (emu)' @ 192.168.3.44
```

but it does **not** open TCP 5343/5344 and removes the service after ~60 s.
This also happens with the real two-service dock layout, so it is not a
missing-service problem. The remaining gate is most likely the Cora pairing /
selection flow (`ESDCoraPairingDialog`), not the Cora framing itself.

## Next capture

1. Power the Studio over PoE and connect its Ethernet directly to a PC NIC (or
   the same switch).
2. `cora-probe --discover`, then `cora-probe <ip>`.
3. Capture `udp port 5353 or tcp port 5343 or tcp port 5344` while the app
   connects, and run `tools/wireshark/cora.lua`.
4. Record the real Studio's mDNS `dt` / `prt` / service count and the first TCP
   exchange; use it to drive the emulator's pairing state.
