//! Commands decoded from host output reports.

use productiondeck_core::protocol::OutputSink;
use tokio::sync::mpsc;

/// A command the host asked the device to perform.
#[derive(Debug, Clone)]
pub enum DeckCommand {
    KeyImage {
        key: u8,
        jpeg: Vec<u8>,
    },
    PanelImage {
        x: u16,
        y: u16,
        w: u16,
        h: u16,
        jpeg: Vec<u8>,
    },
    FullScreen {
        panel: u8,
        jpeg: Vec<u8>,
    },
    Window {
        jpeg: Vec<u8>,
    },
    Background {
        index: u8,
        jpeg: Vec<u8>,
    },
    Brightness(u8),
    KeyRgb {
        key: u8,
        r: u8,
        g: u8,
        b: u8,
    },
    FillLcd {
        r: u8,
        g: u8,
        b: u8,
    },
    ClearPanel(u8),
    EncoderRing {
        enc: u8,
        rgb: Vec<u8>,
    },
    EncoderLed {
        enc: u8,
        r: u8,
        g: u8,
        b: u8,
    },
    Reset,
    Sleep(i32),
}

/// [`OutputSink`] that forwards decoded commands to an async channel.
pub struct CommandSink {
    tx: mpsc::UnboundedSender<DeckCommand>,
}

impl CommandSink {
    pub fn new(tx: mpsc::UnboundedSender<DeckCommand>) -> Self {
        Self { tx }
    }
}

impl OutputSink for CommandSink {
    fn key_image(&mut self, key: u8, jpeg: &[u8]) {
        let _ = self.tx.send(DeckCommand::KeyImage {
            key,
            jpeg: jpeg.to_vec(),
        });
    }

    fn panel_image(&mut self, x: u16, y: u16, w: u16, h: u16, jpeg: &[u8]) {
        let _ = self.tx.send(DeckCommand::PanelImage {
            x,
            y,
            w,
            h,
            jpeg: jpeg.to_vec(),
        });
    }

    fn full_screen(&mut self, panel: u8, jpeg: &[u8]) {
        let _ = self.tx.send(DeckCommand::FullScreen {
            panel,
            jpeg: jpeg.to_vec(),
        });
    }

    fn window(&mut self, jpeg: &[u8]) {
        let _ = self.tx.send(DeckCommand::Window {
            jpeg: jpeg.to_vec(),
        });
    }

    fn background(&mut self, index: u8, jpeg: &[u8]) {
        let _ = self.tx.send(DeckCommand::Background {
            index,
            jpeg: jpeg.to_vec(),
        });
    }

    fn brightness(&mut self, percent: u8) {
        let _ = self.tx.send(DeckCommand::Brightness(percent));
    }

    fn key_rgb(&mut self, key: u8, r: u8, g: u8, b: u8) {
        let _ = self.tx.send(DeckCommand::KeyRgb { key, r, g, b });
    }

    fn fill_lcd(&mut self, r: u8, g: u8, b: u8) {
        let _ = self.tx.send(DeckCommand::FillLcd { r, g, b });
    }

    fn clear_panel(&mut self, panel: u8) {
        let _ = self.tx.send(DeckCommand::ClearPanel(panel));
    }

    fn encoder_ring(&mut self, enc: u8, rgb: &[u8]) {
        let _ = self.tx.send(DeckCommand::EncoderRing {
            enc,
            rgb: rgb.to_vec(),
        });
    }

    fn encoder_led(&mut self, enc: u8, r: u8, g: u8, b: u8) {
        let _ = self.tx.send(DeckCommand::EncoderLed { enc, r, g, b });
    }

    fn reset(&mut self) {
        let _ = self.tx.send(DeckCommand::Reset);
    }

    fn sleep(&mut self, seconds: i32) {
        let _ = self.tx.send(DeckCommand::Sleep(seconds));
    }
}
