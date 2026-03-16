/// Stream Deck Studio device constants.
///
/// Based on the node-elgato-stream-deck source code:
/// - VENDOR_ID / product ID from packages/core/src/index.ts
/// - Feature report layouts from packages/tcp/src/connectionManager.ts
///   and packages/core/src/services/properties/
/// - Input report layout from packages/core/src/services/input/gen2.ts
///   and packages/core/src/models/studio.ts

pub const VENDOR_ID: u16 = 0x0fd9;
pub const PRODUCT_ID: u16 = 0x00aa;
pub const DEFAULT_TCP_PORT: u16 = 5343;

/// Number of physical buttons on Stream Deck Studio
pub const BUTTON_COUNT: usize = 32;

/// Number of encoders (dials) on Stream Deck Studio
pub const ENCODER_COUNT: usize = 2;

#[allow(dead_code)]

/// Gen2 input: KEY_DATA_OFFSET = 3
/// payload[0] = 0x01 (report indicator)
/// payload[1] = input type (0x00=button, 0x02=lcd, 0x03=encoder, 0x04=nfc)
/// payload[2..3] = padding
/// payload[3..3+BUTTON_COUNT] = button states (1=pressed)
#[allow(dead_code)]
pub const KEY_DATA_OFFSET: usize = 3;

/// Total input payload length for button events
#[allow(dead_code)]
pub const BUTTON_PAYLOAD_LEN: usize = 1 + 1 + 1 + 1 + BUTTON_COUNT; // 36

pub const INPUT_TYPE_BUTTON: u8 = 0x00;
#[allow(dead_code)]
pub const INPUT_TYPE_ENCODER: u8 = 0x03;

/// mDNS service type
#[allow(dead_code)]
pub const MDNS_SERVICE_TYPE: &str = "_elg._tcp.local.";

/// TXT record: dt value for Studio (non-215, use vid/pid path)
/// This value identifies the device type; 215 is reserved for Network Dock.
pub const DT_VALUE: &str = "170"; // same as pid decimal (used as placeholder)
