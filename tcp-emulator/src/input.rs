/// Input event payload builder for Stream Deck Studio (Gen2).
///
/// From packages/core/src/services/input/gen2.ts:
///   data[0] = input type  (0x00=button, 0x02=lcd, 0x03=encoder, 0x04=nfc)
///
/// From packages/core/src/services/input/gen1.ts (base):
///   KEY_DATA_OFFSET = 3  (for Gen2 / Studio)
///   data[KEY_DATA_OFFSET + hidIndex] = pressed (1) or released (0)
///
/// The Cora payload sent to the client for an input event is:
///   [0x01,         ← TcpCoraHidDevice checks payload[0]==0x01 to detect input
///    input_type,   ← 0x00 for button, 0x03 for encoder
///    0x00, 0x00,   ← padding up to KEY_DATA_OFFSET=3
///    data...]      ← event data starting at KEY_DATA_OFFSET
///
/// Studio has 32 buttons (indices 0-31) and 2 encoders (indices 0-1).
use crate::studio::{BUTTON_COUNT, ENCODER_COUNT, INPUT_TYPE_BUTTON, INPUT_TYPE_ENCODER};

#[derive(Debug, Clone)]
pub struct ButtonState {
    states: [bool; BUTTON_COUNT],
}

impl Default for ButtonState {
    fn default() -> Self {
        Self {
            states: [false; BUTTON_COUNT],
        }
    }
}

impl ButtonState {
    pub fn press(&mut self, index: usize) -> bool {
        if index >= BUTTON_COUNT {
            return false;
        }
        self.states[index] = true;
        true
    }

    pub fn release(&mut self, index: usize) -> bool {
        if index >= BUTTON_COUNT {
            return false;
        }
        self.states[index] = false;
        true
    }

    #[allow(dead_code)]
    pub fn is_pressed(&self, index: usize) -> bool {
        self.states.get(index).copied().unwrap_or(false)
    }

    /// Build the Cora payload for the current button state.
    ///
    /// Layout (36 bytes total):
    ///   payload[0]    = 0x01              ← TcpCoraHidDevice strips this, emits input(payload[1:])
    ///   payload[1]    = 0x00              ← data[0] = Gen2 input type (0x00 = button)
    ///   payload[2]    = 0x00              ← data[1] padding
    ///   payload[3]    = 0x00              ← data[2] padding
    ///   payload[4..36]= key0..key31       ← data[KEY_DATA_OFFSET..] (KEY_DATA_OFFSET=3)
    pub fn to_payload(&self) -> Vec<u8> {
        let mut payload = Vec::with_capacity(4 + BUTTON_COUNT);
        payload.push(0x01);               // TCP input event marker (stripped by HID layer)
        payload.push(INPUT_TYPE_BUTTON);  // Gen2 input type: 0x00 = button
        payload.push(0x00);              // data[1] padding
        payload.push(0x00);              // data[2] padding — key data follows at data[3]=KEY_DATA_OFFSET
        for &pressed in &self.states {
            payload.push(if pressed { 1 } else { 0 });
        }
        payload
    }
}

/// Encoder input event payload builder for Stream Deck Studio (Gen2).
///
/// From packages/core/src/services/input/gen2.ts (encoder handling):
///   input type = 0x03
///   data[KEY_DATA_OFFSET + encoderIndex*2 + 0] = press state (1=pressed, 0=released)
///   data[KEY_DATA_OFFSET + encoderIndex*2 + 1] = rotation delta (i8, positive=CW)
///
/// Studio has 2 encoders (indices 0-1).
pub struct EncoderState {
    press: [bool; ENCODER_COUNT],
}

impl Default for EncoderState {
    fn default() -> Self {
        Self {
            press: [false; ENCODER_COUNT],
        }
    }
}

impl EncoderState {
    /// Set the press state of an encoder. Returns false if index is out of range.
    pub fn set_press(&mut self, index: usize, pressed: bool) -> bool {
        if index >= ENCODER_COUNT {
            return false;
        }
        self.press[index] = pressed;
        true
    }

    /// Build a Cora payload for the current encoder press states with zero rotation.
    ///
    /// Layout:
    ///   payload[0]   = 0x01                     ← TCP input event marker
    ///   payload[1]   = 0x03                     ← Gen2 input type: encoder
    ///   payload[2-3] = 0x00                     ← padding (KEY_DATA_OFFSET=3)
    ///   payload[4 + i*2 + 0] = press[i]         ← press state per encoder
    ///   payload[4 + i*2 + 1] = 0x00             ← rotation delta (none)
    pub fn to_payload(&self) -> Vec<u8> {
        let mut payload = Vec::with_capacity(4 + ENCODER_COUNT * 2);
        payload.push(0x01);
        payload.push(INPUT_TYPE_ENCODER);
        payload.push(0x00);
        payload.push(0x00);
        for &pressed in &self.press {
            payload.push(if pressed { 1 } else { 0 });
            payload.push(0x00); // rotation delta = 0
        }
        payload
    }

    /// Build a Cora payload for a single encoder rotation event.
    ///
    /// `delta` is a signed rotation tick count (positive = clockwise).
    /// Press states are included as-is; only the specified encoder's delta is set.
    pub fn to_rotation_payload(&self, index: usize, delta: i8) -> Vec<u8> {
        let mut payload = Vec::with_capacity(4 + ENCODER_COUNT * 2);
        payload.push(0x01);
        payload.push(INPUT_TYPE_ENCODER);
        payload.push(0x00);
        payload.push(0x00);
        for (i, &pressed) in self.press.iter().enumerate() {
            payload.push(if pressed { 1 } else { 0 });
            payload.push(if i == index { delta as u8 } else { 0 });
        }
        payload
    }
}
