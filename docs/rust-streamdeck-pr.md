# rust-streamdeck Studio PR (prepared)

Patch: [`tools/rust-streamdeck-studio.patch`](../tools/rust-streamdeck-studio.patch)

Upstream: https://github.com/ryankurte/rust-streamdeck (crate `streamdeck` 0.10.0)

## Changes

- `pids::STUDIO = 0x00AA`
- `Kind::Studio`
- 32 keys, 16 columns, `key_data_offset = 3`
- 144x112 JPEG, no rotation, no mirror, RGB order
- 1024-byte image report, 8-byte Gen2 header
- empty BMP base

## Suggested PR

Title: `feat: add Stream Deck Studio (0x00AA) USB HID support`

Body:

> Add the Stream Deck Studio to the device table so the crate can open and
> drive it over USB HID.
>
> - PID `0x00AA`
> - 32 keys (16x2), 144x112 JPEG
> - Gen2 image header (8 bytes), 1024-byte output reports
> - no rotation/mirror
>
> Verified against a real Studio (firmware 2.01.002): product/serial read,
> brightness set, 144x112 JPEG written to key 0 and key 31.
>
> Cora (TCP 5343) is a separate transport and is not part of this change.

## Verification already performed

`productiondeck-verify` opens the Studio through `hidapi` (the same backend as
this crate) and confirms the feature reports, brightness round-trip and key
image writes. After this patch, `StreamDeck::connect(0x0fd9, 0x00aa, None)`
should select `Kind::Studio` instead of returning `UnrecognisedPID`.
