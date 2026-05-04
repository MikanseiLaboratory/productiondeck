//! Inter-task communication channels
//!
//! This module defines all the Embassy channels used for communication
//! between different tasks in the ProductionDeck application.

use crate::config::{DISPLAY_CHANNEL_CAPACITY, MULTICORE_CHANNEL_SIZE, USB_COMMAND_CHANNEL_SIZE};
use crate::types::{ButtonState, DisplayCommand, UsbCommand};
use embassy_sync::blocking_mutex::raw::{CriticalSectionRawMutex, ThreadModeRawMutex};
use embassy_sync::channel::Channel;

/// Channel for button state communication from button task to USB task
/// Buffer size: 1 (latest state only)
pub static BUTTON_CHANNEL: Channel<ThreadModeRawMutex, ButtonState, 1> = Channel::new();

/// Channel for USB commands from HID handler to other tasks
/// Buffer size: 4 (allows some buffering of commands)
pub static USB_COMMAND_CHANNEL: Channel<ThreadModeRawMutex, UsbCommand, USB_COMMAND_CHANNEL_SIZE> =
    Channel::new();

/// Channel for display commands to the display task
pub static DISPLAY_CHANNEL: Channel<
    ThreadModeRawMutex,
    DisplayCommand,
    DISPLAY_CHANNEL_CAPACITY,
> = Channel::new();

/// Core 0 → Core 1 display commands (multicore builds). Uses [`MULTICORE_CHANNEL_SIZE`] slots.
pub static MULTICORE_IMAGE_CHANNEL: Channel<
    CriticalSectionRawMutex,
    DisplayCommand,
    MULTICORE_CHANNEL_SIZE,
> = Channel::new();
