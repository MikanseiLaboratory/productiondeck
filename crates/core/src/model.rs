//! Device model definitions.
//!
//! This is the single source of truth for per-model layout, image format,
//! feature capability and transport support. It intentionally contains no
//! functions and no I/O, so it can be consumed from firmware and host code.

/// Maximum number of physical keys / protocol button slots handled by the core.
pub const MAX_KEYS: usize = 64;
/// Maximum number of encoders handled by the core.
pub const MAX_ENCODERS: usize = 4;
/// Maximum number of fullscreen panels handled by the core.
pub const MAX_PANELS: usize = 2;

/// Image format a model expects on the wire.
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImageFormat {
    /// Legacy BMP (Mini / Original).
    Bmp,
    /// Gen2 JPEG.
    Jpeg,
}

/// HID protocol family.
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProtocolFamily {
    /// Legacy Mini-family protocol.
    Gen1,
    /// Main / Expanded (Gen2) protocol.
    Gen2,
    /// 6-key Module legacy command set.
    Module6,
}

/// Stable model identifier.
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum ModelId {
    Mini = 0,
    RevisedMini = 1,
    MiniDiscord = 2,
    Original = 3,
    OriginalV2 = 4,
    Mk2 = 5,
    Mk2ScissorKeys = 6,
    Xl = 7,
    Xl2022 = 8,
    Plus = 9,
    PlusXl = 10,
    Neo = 11,
    Module6Keys = 12,
    Module15Keys = 13,
    Module32Keys = 14,
    /// Stream Deck Studio (USB HID + native Cora TCP).
    Studio = 15,
    /// Stream Deck Network Dock (Cora only, no physical controls).
    NetworkDock = 16,
}

/// Static capability / geometry description of a model.
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
#[derive(Debug, Clone, Copy)]
pub struct ModelInfo {
    pub id: ModelId,
    pub name: &'static str,
    pub vid: u16,
    pub pid: u16,
    pub protocol: ProtocolFamily,

    pub cols: u8,
    pub rows: u8,
    pub left_to_right: bool,

    pub key_w: u16,
    pub key_h: u16,
    pub image_format: ImageFormat,
    pub flip_h: bool,
    pub flip_v: bool,
    pub rotate: bool,

    /// Offset of the first key state byte in a Gen1/Gen2 input payload
    /// (i.e. after the HID report id has been stripped).
    pub key_data_offset: u8,

    pub rgb_keys: bool,
    pub nfc: bool,
    pub fullscreen_panels: u8,
    pub child_devices: bool,

    pub encoder_count: u8,
    pub encoder_ring_steps: u8,
    /// Per-encoder rotation offset applied when packing ring colours.
    pub encoder_ring_offsets: [u8; MAX_ENCODERS],

    pub lcd_w: u16,
    pub lcd_h: u16,

    pub usb: bool,
    pub tcp: bool,
}

impl ModelInfo {
    /// Total number of keys (`cols * rows`).
    pub const fn key_count(&self) -> usize {
        self.cols as usize * self.rows as usize
    }
}

const NO_RING: [u8; MAX_ENCODERS] = [0; MAX_ENCODERS];

pub const MINI: ModelInfo = ModelInfo {
    id: ModelId::Mini,
    name: "Stream Deck Mini",
    vid: 0x0fd9,
    pid: 0x0063,
    protocol: ProtocolFamily::Gen1,
    cols: 3,
    rows: 2,
    left_to_right: true,
    key_w: 80,
    key_h: 80,
    image_format: ImageFormat::Bmp,
    flip_h: false,
    flip_v: false,
    rotate: true,
    key_data_offset: 1,
    rgb_keys: false,
    nfc: false,
    fullscreen_panels: 0,
    child_devices: false,
    encoder_count: 0,
    encoder_ring_steps: 0,
    encoder_ring_offsets: NO_RING,
    lcd_w: 320,
    lcd_h: 240,
    usb: true,
    tcp: false,
};

pub const REVISED_MINI: ModelInfo = ModelInfo {
    id: ModelId::RevisedMini,
    pid: 0x0090,
    name: "Stream Deck Mini 2022",
    ..MINI
};

pub const MINI_DISCORD: ModelInfo = ModelInfo {
    id: ModelId::MiniDiscord,
    pid: 0x00b3,
    name: "Stream Deck Mini Discord",
    ..MINI
};

pub const ORIGINAL: ModelInfo = ModelInfo {
    id: ModelId::Original,
    name: "Stream Deck Original",
    vid: 0x0fd9,
    pid: 0x0060,
    protocol: ProtocolFamily::Gen1,
    cols: 5,
    rows: 3,
    left_to_right: false,
    key_w: 72,
    key_h: 72,
    image_format: ImageFormat::Bmp,
    flip_h: true,
    flip_v: false,
    rotate: false,
    key_data_offset: 1,
    rgb_keys: false,
    nfc: false,
    fullscreen_panels: 0,
    child_devices: false,
    encoder_count: 0,
    encoder_ring_steps: 0,
    encoder_ring_offsets: NO_RING,
    lcd_w: 480,
    lcd_h: 272,
    usb: true,
    tcp: false,
};

const GEN2_5X3: ModelInfo = ModelInfo {
    id: ModelId::OriginalV2,
    name: "Stream Deck Classic (2019)",
    vid: 0x0fd9,
    pid: 0x006d,
    protocol: ProtocolFamily::Gen2,
    cols: 5,
    rows: 3,
    left_to_right: true,
    key_w: 72,
    key_h: 72,
    image_format: ImageFormat::Jpeg,
    flip_h: true,
    flip_v: true,
    rotate: false,
    key_data_offset: 3,
    rgb_keys: false,
    nfc: false,
    fullscreen_panels: 0,
    child_devices: false,
    encoder_count: 0,
    encoder_ring_steps: 0,
    encoder_ring_offsets: NO_RING,
    lcd_w: 480,
    lcd_h: 272,
    usb: true,
    tcp: false,
};

pub const ORIGINAL_V2: ModelInfo = GEN2_5X3;

pub const MK2: ModelInfo = ModelInfo {
    id: ModelId::Mk2,
    pid: 0x0080,
    name: "Stream Deck Mk.2",
    ..GEN2_5X3
};

pub const MK2_SCISSOR: ModelInfo = ModelInfo {
    id: ModelId::Mk2ScissorKeys,
    pid: 0x00a5,
    name: "Stream Deck Mk.2 (Scissor)",
    ..GEN2_5X3
};

const GEN2_8X4: ModelInfo = ModelInfo {
    id: ModelId::Xl,
    name: "Stream Deck XL",
    vid: 0x0fd9,
    pid: 0x006c,
    protocol: ProtocolFamily::Gen2,
    cols: 8,
    rows: 4,
    left_to_right: true,
    key_w: 96,
    key_h: 96,
    image_format: ImageFormat::Jpeg,
    flip_h: true,
    flip_v: true,
    rotate: false,
    key_data_offset: 3,
    rgb_keys: false,
    nfc: false,
    fullscreen_panels: 0,
    child_devices: false,
    encoder_count: 0,
    encoder_ring_steps: 0,
    encoder_ring_offsets: NO_RING,
    lcd_w: 1024,
    lcd_h: 600,
    usb: true,
    tcp: false,
};

pub const XL: ModelInfo = GEN2_8X4;

pub const XL_2022: ModelInfo = ModelInfo {
    id: ModelId::Xl2022,
    pid: 0x008f,
    name: "Stream Deck XL 2022",
    ..GEN2_8X4
};

pub const PLUS: ModelInfo = ModelInfo {
    id: ModelId::Plus,
    name: "Stream Deck +",
    vid: 0x0fd9,
    pid: 0x0084,
    protocol: ProtocolFamily::Gen2,
    cols: 4,
    rows: 2,
    left_to_right: true,
    key_w: 120,
    key_h: 120,
    image_format: ImageFormat::Jpeg,
    flip_h: false,
    flip_v: false,
    rotate: false,
    key_data_offset: 3,
    rgb_keys: true,
    nfc: false,
    fullscreen_panels: 0,
    child_devices: false,
    encoder_count: 4,
    encoder_ring_steps: 24,
    encoder_ring_offsets: NO_RING,
    lcd_w: 800,
    lcd_h: 480,
    usb: true,
    tcp: false,
};

pub const PLUS_XL: ModelInfo = ModelInfo {
    id: ModelId::PlusXl,
    name: "Stream Deck + XL",
    vid: 0x0fd9,
    pid: 0x00c6,
    protocol: ProtocolFamily::Gen2,
    cols: 9,
    rows: 4,
    left_to_right: true,
    key_w: 112,
    key_h: 112,
    image_format: ImageFormat::Jpeg,
    flip_h: true,
    flip_v: true,
    rotate: false,
    key_data_offset: 3,
    rgb_keys: true,
    nfc: false,
    fullscreen_panels: 0,
    child_devices: false,
    encoder_count: 2,
    encoder_ring_steps: 24,
    encoder_ring_offsets: NO_RING,
    lcd_w: 1280,
    lcd_h: 800,
    usb: true,
    tcp: false,
};

pub const NEO: ModelInfo = ModelInfo {
    id: ModelId::Neo,
    name: "Stream Deck Neo",
    vid: 0x0fd9,
    pid: 0x009a,
    protocol: ProtocolFamily::Gen2,
    cols: 4,
    rows: 2,
    left_to_right: true,
    key_w: 96,
    key_h: 96,
    image_format: ImageFormat::Jpeg,
    flip_h: true,
    flip_v: true,
    rotate: false,
    key_data_offset: 3,
    rgb_keys: true,
    nfc: false,
    fullscreen_panels: 0,
    child_devices: false,
    encoder_count: 0,
    encoder_ring_steps: 0,
    encoder_ring_offsets: NO_RING,
    lcd_w: 480,
    lcd_h: 320,
    usb: true,
    tcp: false,
};

pub const MODULE6: ModelInfo = ModelInfo {
    id: ModelId::Module6Keys,
    name: "Stream Deck Module 6 Keys",
    vid: 0x0fd9,
    pid: 0x00b8,
    protocol: ProtocolFamily::Module6,
    cols: 3,
    rows: 2,
    left_to_right: true,
    key_w: 80,
    key_h: 80,
    image_format: ImageFormat::Bmp,
    flip_h: false,
    flip_v: false,
    rotate: true,
    key_data_offset: 1,
    rgb_keys: false,
    nfc: false,
    fullscreen_panels: 0,
    child_devices: false,
    encoder_count: 0,
    encoder_ring_steps: 0,
    encoder_ring_offsets: NO_RING,
    lcd_w: 320,
    lcd_h: 240,
    usb: true,
    tcp: false,
};

pub const MODULE15: ModelInfo = ModelInfo {
    id: ModelId::Module15Keys,
    name: "Stream Deck Module 15 Keys",
    pid: 0x00b9,
    ..GEN2_5X3
};

pub const MODULE32: ModelInfo = ModelInfo {
    id: ModelId::Module32Keys,
    name: "Stream Deck Module 32 Keys",
    pid: 0x00ba,
    ..GEN2_8X4
};

const _: () = {
    // Guard against accidental `..base` updates keeping the base id.
    assert!(REVISED_MINI.id as u8 == ModelId::RevisedMini as u8);
    assert!(MINI_DISCORD.id as u8 == ModelId::MiniDiscord as u8);
    assert!(MK2.id as u8 == ModelId::Mk2 as u8);
    assert!(MK2_SCISSOR.id as u8 == ModelId::Mk2ScissorKeys as u8);
    assert!(XL_2022.id as u8 == ModelId::Xl2022 as u8);
    assert!(MODULE15.id as u8 == ModelId::Module15Keys as u8);
    assert!(MODULE32.id as u8 == ModelId::Module32Keys as u8);
    assert!(STUDIO.id as u8 == ModelId::Studio as u8);
};

/// Stream Deck Studio: 32 keys (16x2), 144x112 keys, two 24-step encoders,
/// NFC, two fullscreen panels, USB HID and native Cora TCP.
///
/// `lcd_h` is provisional (2 rows x 112px) until the real 0x08 unit-info
/// response is captured; see `docs/studio-protocol.md`.
pub const STUDIO: ModelInfo = ModelInfo {
    id: ModelId::Studio,
    name: "Stream Deck Studio",
    vid: 0x0fd9,
    pid: 0x00aa,
    protocol: ProtocolFamily::Gen2,
    cols: 16,
    rows: 2,
    left_to_right: true,
    key_w: 144,
    key_h: 112,
    image_format: ImageFormat::Jpeg,
    flip_h: false,
    flip_v: false,
    rotate: false,
    key_data_offset: 3,
    rgb_keys: true,
    nfc: true,
    fullscreen_panels: 2,
    child_devices: true,
    encoder_count: 2,
    encoder_ring_steps: 24,
    encoder_ring_offsets: [0, 12, 0, 0],
    lcd_w: 1424,
    lcd_h: 280,
    usb: true,
    tcp: true,
};

/// Stream Deck Network Dock: Cora primary port with a child-device port, no
/// physical controls. Used by the emulator's `network-dock` profile and as the
/// reference for Studio/Dock classification.
pub const NETWORK_DOCK: ModelInfo = ModelInfo {
    id: ModelId::NetworkDock,
    name: "Stream Deck Network Dock",
    vid: 0x0fd9,
    pid: 0xffff,
    protocol: ProtocolFamily::Gen2,
    cols: 0,
    rows: 0,
    left_to_right: true,
    key_w: 0,
    key_h: 0,
    image_format: ImageFormat::Jpeg,
    flip_h: false,
    flip_v: false,
    rotate: false,
    key_data_offset: 3,
    rgb_keys: false,
    nfc: false,
    fullscreen_panels: 0,
    child_devices: true,
    encoder_count: 0,
    encoder_ring_steps: 0,
    encoder_ring_offsets: NO_RING,
    lcd_w: 0,
    lcd_h: 0,
    usb: false,
    tcp: true,
};

/// Every model known to the core.
pub const MODELS: &[ModelInfo] = &[
    MINI,
    REVISED_MINI,
    MINI_DISCORD,
    ORIGINAL,
    ORIGINAL_V2,
    MK2,
    MK2_SCISSOR,
    XL,
    XL_2022,
    PLUS,
    PLUS_XL,
    NEO,
    MODULE6,
    MODULE15,
    MODULE32,
    STUDIO,
    NETWORK_DOCK,
];

/// Look up a model by id.
pub fn model(id: ModelId) -> &'static ModelInfo {
    MODELS.iter().find(|m| m.id == id).unwrap_or(&MINI)
}

/// Look up a USB model by VID/PID.
pub fn find_usb(vid: u16, pid: u16) -> Option<&'static ModelInfo> {
    MODELS
        .iter()
        .find(|m| m.usb && m.vid == vid && m.pid == pid)
}

/// Look up a USB model by PID (Elgato VID assumed).
pub fn find_pid(pid: u16) -> Option<&'static ModelInfo> {
    find_usb(0x0fd9, pid)
}
