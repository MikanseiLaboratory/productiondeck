//! Inter-task communication channels
//!
//! This module defines all the Embassy channels used for communication
//! between different tasks in the ProductionDeck application.

use crate::types::{ButtonState, DisplayCommand, UsbCommand};
use embassy_sync::blocking_mutex::raw::ThreadModeRawMutex;
use embassy_sync::channel::Channel;

/// Channel for button state communication from button task to USB task
/// Buffer size: 1 (latest state only)
pub static BUTTON_CHANNEL: Channel<ThreadModeRawMutex, ButtonState, 1> = Channel::new();

/// Channel for USB commands from HID handler to other tasks
/// Buffer size: 4 (allows some buffering of commands)
pub static USB_COMMAND_CHANNEL: Channel<ThreadModeRawMutex, UsbCommand, 4> = Channel::new();

/// Channel for display commands to the display task
/// Buffer size: 8 (allows buffering of multiple display operations)
pub static DISPLAY_CHANNEL: Channel<ThreadModeRawMutex, DisplayCommand, 8> = Channel::new();
