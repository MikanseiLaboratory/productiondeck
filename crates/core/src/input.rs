//! Transport-independent input events.
//!
//! A device (firmware or emulator) produces [`InputEvent`]s from buttons,
//! encoders, touch strips and NFC. The Gen2 protocol encoder turns them into
//! HID input reports; the Cora session wraps those reports into frames.

/// Touch gesture kind (Stream Deck + / + XL; unused by Studio).
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TouchKind {
    Tap,
    Press,
    Swipe,
}

/// Maximum NFC UID length carried by the core.
pub const MAX_NFC_UID: usize = 32;

/// An input event produced by a device.
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputEvent {
    /// A single key changed state. The encoder expands this into a full
    /// button-state report for the model.
    Button { index: u8, pressed: bool },
    /// An encoder was pressed or released.
    EncoderPress { index: u8, pressed: bool },
    /// An encoder rotated by `delta` ticks (positive = clockwise).
    EncoderRotate { index: u8, delta: i8 },
    /// Touch strip event.
    Touch {
        kind: TouchKind,
        x: u16,
        y: u16,
        x2: u16,
        y2: u16,
    },
    /// NFC tag read.
    Nfc { uid: [u8; MAX_NFC_UID], len: u8 },
}

impl InputEvent {
    /// Convenience constructor for an NFC event from a slice.
    pub fn nfc(uid: &[u8]) -> Self {
        let mut buf = [0u8; MAX_NFC_UID];
        let len = uid.len().min(MAX_NFC_UID);
        buf[..len].copy_from_slice(&uid[..len]);
        InputEvent::Nfc {
            uid: buf,
            len: len as u8,
        }
    }
}
