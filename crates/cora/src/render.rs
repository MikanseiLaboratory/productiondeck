//! Virtual Studio panel renderer used by the emulator and tests.
//!
//! The real Studio has two 2304x112 LCD strips behind 16x2 keys. This renderer
//! composites key images and panel images into a single 2304x224 RGB buffer so
//! geometry, flips and chunk assembly can be checked without hardware.

use std::path::Path;

use crate::command::DeckCommand;

pub const PANEL_W: usize = 2304;
pub const PANEL_H: usize = 224;
pub const KEY_W: usize = 144;
pub const KEY_H: usize = 112;
pub const COLS: usize = 16;
pub const ROWS: usize = 2;

/// A 2304x224 RGB888 virtual panel.
pub struct VirtualStudio {
    rgb: Vec<u8>,
}

impl Default for VirtualStudio {
    fn default() -> Self {
        Self::new()
    }
}

impl VirtualStudio {
    pub fn new() -> Self {
        Self {
            rgb: vec![0; PANEL_W * PANEL_H * 3],
        }
    }

    pub fn as_rgb(&self) -> &[u8] {
        &self.rgb
    }

    pub fn apply(&mut self, command: &DeckCommand) {
        match command {
            DeckCommand::KeyImage { key, jpeg } => {
                if let Some((px, pw, ph)) = decode(jpeg) {
                    let (x, y) = key_origin(*key);
                    blit_scaled(&mut self.rgb, &px, pw, ph, x, y, KEY_W, KEY_H);
                }
            }
            DeckCommand::PanelImage { x, y, w, h, jpeg } => {
                if let Some((px, pw, ph)) = decode(jpeg) {
                    blit_scaled(
                        &mut self.rgb,
                        &px,
                        pw,
                        ph,
                        *x as usize,
                        *y as usize,
                        *w as usize,
                        *h as usize,
                    );
                }
            }
            DeckCommand::FullScreen { panel, jpeg } => {
                if let Some((px, pw, ph)) = decode(jpeg) {
                    let y = (*panel as usize).min(1) * KEY_H;
                    blit_scaled(&mut self.rgb, &px, pw, ph, 0, y, PANEL_W, KEY_H);
                }
            }
            DeckCommand::FillLcd { r, g, b } => fill(&mut self.rgb, *r, *g, *b),
            DeckCommand::KeyRgb { key, r, g, b } => {
                let (x, y) = key_origin(*key);
                fill_rect(&mut self.rgb, x, y, KEY_W, KEY_H, *r, *g, *b);
            }
            DeckCommand::ClearPanel(panel) => {
                let y = (*panel as usize).min(1) * KEY_H;
                fill_rect(&mut self.rgb, 0, y, PANEL_W, KEY_H, 0, 0, 0);
            }
            DeckCommand::Reset => self.rgb.fill(0),
            _ => {}
        }
    }

    /// Write the current image as a binary PPM (P6) file.
    pub fn write_ppm(&self, path: impl AsRef<Path>) -> std::io::Result<()> {
        let mut out = Vec::with_capacity(32 + self.rgb.len());
        out.extend_from_slice(format!("P6\n{PANEL_W} {PANEL_H}\n255\n").as_bytes());
        out.extend_from_slice(&self.rgb);
        std::fs::write(path, out)
    }
}

fn key_origin(key: u8) -> (usize, usize) {
    let key = key as usize;
    let col = key % COLS;
    let row = (key / COLS).min(ROWS - 1);
    (col * KEY_W, row * KEY_H)
}

#[allow(clippy::chunks_exact_to_as_chunks)]
fn fill(dst: &mut [u8], r: u8, g: u8, b: u8) {
    for px in dst.chunks_exact_mut(3) {
        px.copy_from_slice(&[r, g, b]);
    }
}

#[allow(clippy::too_many_arguments)]
fn fill_rect(dst: &mut [u8], x: usize, y: usize, w: usize, h: usize, r: u8, g: u8, b: u8) {
    for yy in y..(y + h).min(PANEL_H) {
        for xx in x..(x + w).min(PANEL_W) {
            let i = (yy * PANEL_W + xx) * 3;
            dst[i..i + 3].copy_from_slice(&[r, g, b]);
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn blit_scaled(
    dst: &mut [u8],
    src: &[u8],
    sw: usize,
    sh: usize,
    x: usize,
    y: usize,
    dw: usize,
    dh: usize,
) {
    if sw == 0 || sh == 0 || dw == 0 || dh == 0 {
        return;
    }
    for dy in 0..dh {
        let ty = y + dy;
        if ty >= PANEL_H {
            break;
        }
        let sy = dy * sh / dh;
        for dx in 0..dw {
            let tx = x + dx;
            if tx >= PANEL_W {
                break;
            }
            let sx = dx * sw / dw;
            let si = (sy * sw + sx) * 3;
            let di = (ty * PANEL_W + tx) * 3;
            if si + 2 < src.len() {
                dst[di..di + 3].copy_from_slice(&src[si..si + 3]);
            }
        }
    }
}

#[allow(clippy::chunks_exact_to_as_chunks)]
fn decode(jpeg: &[u8]) -> Option<(Vec<u8>, usize, usize)> {
    use jpeg_decoder::PixelFormat;

    let mut decoder = jpeg_decoder::Decoder::new(jpeg);
    let pixels = decoder.decode().ok()?;
    let info = decoder.info()?;
    let (w, h) = (info.width as usize, info.height as usize);
    let rgb = match info.pixel_format {
        PixelFormat::RGB24 => pixels,
        PixelFormat::L8 => {
            let mut out = Vec::with_capacity(pixels.len() * 3);
            for v in pixels {
                out.extend_from_slice(&[v, v, v]);
            }
            out
        }
        PixelFormat::L16 => {
            let mut out = Vec::with_capacity(pixels.len() / 2 * 3);
            for v in pixels.chunks_exact(2) {
                let g = v[1];
                out.extend_from_slice(&[g, g, g]);
            }
            out
        }
        PixelFormat::CMYK32 => {
            let mut out = Vec::with_capacity(pixels.len() / 4 * 3);
            for v in pixels.chunks_exact(4) {
                let c = v[0] as u16;
                let m = v[1] as u16;
                let y = v[2] as u16;
                let k = v[3] as u16;
                let r = 255u16.saturating_sub(c + k).min(255) as u8;
                let g = 255u16.saturating_sub(m + k).min(255) as u8;
                let b = 255u16.saturating_sub(y + k).min(255) as u8;
                out.extend_from_slice(&[r, g, b]);
            }
            out
        }
    };
    Some((rgb, w, h))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn key_rgb_lands_in_the_right_cell() {
        let mut panel = VirtualStudio::new();
        panel.apply(&DeckCommand::KeyRgb {
            key: 17,
            r: 255,
            g: 0,
            b: 0,
        });
        let (x, y) = key_origin(17);
        assert_eq!((x, y), (144, 112));
        let i = (y * PANEL_W + x) * 3;
        assert_eq!(&panel.as_rgb()[i..i + 3], &[255, 0, 0]);
    }

    #[test]
    fn reset_clears() {
        let mut panel = VirtualStudio::new();
        panel.apply(&DeckCommand::FillLcd { r: 1, g: 2, b: 3 });
        assert_eq!(&panel.as_rgb()[..3], &[1, 2, 3]);
        panel.apply(&DeckCommand::Reset);
        assert_eq!(&panel.as_rgb()[..3], &[0, 0, 0]);
    }
}
