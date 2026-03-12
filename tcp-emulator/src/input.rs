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
///    input_type,   ← 0x00 for button
///    0x00, 0x00,   ← padding up to KEY_DATA_OFFSET=3
///    key0..key31]  ← 32 button states (1=pressed, 0=released)
///
/// Studio has 32 buttons (indices 0-31).
use crate::studio::{BUTTON_COUNT, INPUT_TYPE_BUTTON};

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
