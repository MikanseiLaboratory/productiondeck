//! HID report handling.
//!
//! [`OutputSink`] is the boundary between the protocol core and whatever
//! consumes device commands (a display pipeline in firmware, a renderer in the
//! host emulator, or a test recorder).

pub mod gen2;

pub use gen2::Gen2Handler;

use crate::feature::FeatureError;
use crate::image::ImageError;

/// Errors produced while parsing host reports.
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProtocolError {
    /// Report was shorter than its declared header.
    Short,
    /// Command byte did not match the expected layout.
    BadCommand,
    /// A caller-provided buffer was too small.
    BufferFull,
    /// Image assembly failed.
    Image(ImageError),
    /// Feature report could not be built.
    Feature(FeatureError),
    /// Key index outside the model's key count.
    BadKey(u8),
    /// Encoder index outside the model's encoder count.
    BadEncoder(u8),
}

impl From<ImageError> for ProtocolError {
    fn from(value: ImageError) -> Self {
        ProtocolError::Image(value)
    }
}

impl From<FeatureError> for ProtocolError {
    fn from(value: FeatureError) -> Self {
        ProtocolError::Feature(value)
    }
}

/// Device-side command sink.
///
/// All methods have no-op defaults so tests and transports that do not care
/// about a particular command can ignore it.
pub trait OutputSink {
    fn key_image(&mut self, _key: u8, _jpeg: &[u8]) {}
    fn panel_image(&mut self, _x: u16, _y: u16, _w: u16, _h: u16, _jpeg: &[u8]) {}
    fn full_screen(&mut self, _panel: u8, _jpeg: &[u8]) {}
    fn window(&mut self, _jpeg: &[u8]) {}
    fn background(&mut self, _index: u8, _jpeg: &[u8]) {}
    fn brightness(&mut self, _percent: u8) {}
    fn key_rgb(&mut self, _key: u8, _r: u8, _g: u8, _b: u8) {}
    fn fill_lcd(&mut self, _r: u8, _g: u8, _b: u8) {}
    fn clear_panel(&mut self, _panel: u8) {}
    fn encoder_ring(&mut self, _enc: u8, _rgb: &[u8]) {}
    fn encoder_led(&mut self, _enc: u8, _r: u8, _g: u8, _b: u8) {}
    fn reset(&mut self) {}
    fn sleep(&mut self, _seconds: i32) {}
}

/// Sink that discards every command.
pub struct NullSink;

impl OutputSink for NullSink {}
