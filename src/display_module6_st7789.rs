//! Core 1 ST7789 pipeline for Module 6 + TC-00 wiring (`--features display`).

use crate::channels::MULTICORE_IMAGE_CHANNEL;
use crate::device::{Device, DeviceConfig};
use crate::display_spi_dma::DisplaySpiBus;
use crate::protocol::image::rotate_270;
use crate::types::DisplayCommand;
use defmt::{info, warn};
use display_interface_spi::SPIInterface;
use embassy_rp::gpio::Output;
use embassy_rp::peripherals::SPI1;
use embassy_rp::spi::{Async, Spi};
use embassy_time::Delay;
use embedded_graphics::primitives::{Primitive, PrimitiveStyleBuilder};
use embedded_graphics_core::geometry::{Point, Size};
use embedded_graphics_core::pixelcolor::{Rgb565, Rgb888, RgbColor};
use embedded_graphics_core::prelude::*;
use embedded_graphics_core::primitives::Rectangle;
use embedded_hal_bus_sync::spi::ExclusiveDevice;
use mipidsi::models::ST7789;
use mipidsi::options::{ColorInversion, ColorOrder, Orientation};
use mipidsi::Builder;

type Module6Display = mipidsi::Display<
    SPIInterface<
        ExclusiveDevice<DisplaySpiBus<'static, SPI1>, Output<'static>, Delay>,
        Output<'static>,
    >,
    ST7789,
    Output<'static>,
>;

fn bmp_rgb888_80(buf: &[u8], dst: &mut [u8; 80 * 80 * 3]) -> Result<(), ()> {
    if buf.len() < 54 {
        return Err(());
    }
    if buf[0] != b'B' || buf[1] != b'M' {
        return Err(());
    }
    let offset = u32::from_le_bytes(buf[10..14].try_into().map_err(|_| ())?) as usize;
    let w = i32::from_le_bytes(buf[18..22].try_into().map_err(|_| ())?).unsigned_abs();
    let h_raw = i32::from_le_bytes(buf[22..26].try_into().map_err(|_| ())?);
    let h = h_raw.unsigned_abs();
    let bpp = u16::from_le_bytes(buf[28..30].try_into().map_err(|_| ())?) as usize;
    if w != 80 || h != 80 || bpp != 24 {
        return Err(());
    }
    let row_stride = ((w as usize * bpp / 8) + 3) & !3;
    let top_down = h_raw < 0;
    for row in 0..80usize {
        let src_row = if top_down { row } else { 79 - row };
        let src_off = offset + src_row * row_stride;
        if src_off + 240 > buf.len() {
            return Err(());
        }
        let dst_off = row * 240;
        for col in 0..80usize {
            let s = src_off + col * 3;
            let d = dst_off + col * 3;
            let b = buf[s];
            let g = buf[s + 1];
            let r = buf[s + 2];
            dst[d] = r;
            dst[d + 1] = g;
            dst[d + 2] = b;
        }
    }
    Ok(())
}

fn key_origin_px(key_id: u8) -> (i32, i32) {
    let k = (key_id as usize).min(5);
    let col = k % 3;
    let row = k / 3;
    ((col * 80) as i32, (row * 80) as i32)
}

fn fill_rect(display: &mut Module6Display, x: i32, y: i32, w: u32, h: u32, c: Rgb565) {
    let rect = Rectangle::new(Point::new(x, y), Size::new(w, h))
        .into_styled(PrimitiveStyleBuilder::new().fill_color(c).build());
    let _ = rect.draw(display);
}

fn draw_px_grid(display: &mut Module6Display, ox: i32, oy: i32, px: &[u8]) {
    let it = (0..80i32).flat_map(|yy| {
        (0..80i32).filter_map(move |xx| {
            let i = ((yy as usize) * 80 + (xx as usize)) * 3;
            if i + 2 >= px.len() {
                return None;
            }
            let r = px[i];
            let g = px[i + 1];
            let b = px[i + 2];
            let c = Rgb565::from(Rgb888::new(r, g, b));
            Some(Pixel(Point::new(ox + xx, oy + yy), c))
        })
    });
    let _ = display.draw_iter(it);
}

fn draw_key_bmp(display: &mut Module6Display, device: Device, key_id: u8, bmp: &[u8]) {
    let dc = device.display_config();
    let mut rgb = [0u8; 80 * 80 * 3];
    if bmp_rgb888_80(bmp, &mut rgb).is_err() {
        warn!("module6 display: invalid BMP for key {}", key_id);
        return;
    }

    let (ox, oy) = key_origin_px(key_id);
    if dc.needs_rotation {
        let rotated = rotate_270(&rgb, 80, 80);
        draw_px_grid(display, ox, oy, rotated.as_slice());
    } else {
        draw_px_grid(display, ox, oy, rgb.as_slice());
    }
}

fn handle_display_cmd(
    display: &mut Module6Display,
    device: Device,
    backlight: &mut Output<'static>,
    cmd: DisplayCommand,
) {
    match cmd {
        DisplayCommand::ClearAll => {
            for kid in 0u8..6 {
                let (x, y) = key_origin_px(kid);
                fill_rect(display, x, y, 80, 80, Rgb565::BLACK);
            }
        }
        DisplayCommand::Clear(key_id) => {
            let (x, y) = key_origin_px(key_id);
            fill_rect(display, x, y, 80, 80, Rgb565::BLACK);
        }
        DisplayCommand::SetBrightness(pct) => {
            info!("backlight {}%", pct);
            if pct > 4 {
                backlight.set_high();
            } else {
                backlight.set_low();
            }
        }
        DisplayCommand::DisplayImage { key_id, data } => {
            draw_key_bmp(display, device, key_id, data.as_slice());
        }
        DisplayCommand::FillLcd { r, g, b } => {
            let c = Rgb565::from(Rgb888::new(r, g, b));
            fill_rect(display, 0, 0, 240, 320, c);
        }
        DisplayCommand::FillKey { key_index, r, g, b } => {
            let c = Rgb565::from(Rgb888::new(r, g, b));
            let (x, y) = key_origin_px(key_index);
            fill_rect(display, x, y, 80, 80, c);
        }
        DisplayCommand::DisplayFullScreen { .. } | DisplayCommand::DisplayWindow { .. } => {}
    }
}

#[embassy_executor::task]
pub async fn module6_st7789_core1_task(
    device: Device,
    spi: Spi<'static, SPI1, Async>,
    cs: Output<'static>,
    dc: Output<'static>,
    rst: Output<'static>,
    mut backlight: Output<'static>,
) {
    backlight.set_high();

    let spi_bus = DisplaySpiBus(spi);
    let spi_dev = ExclusiveDevice::new(spi_bus, cs, Delay);
    let di = SPIInterface::new(spi_dev, dc);
    let mut display: Module6Display = Builder::new(ST7789, di)
        .display_size(240, 320)
        .orientation(Orientation::new())
        .reset_pin(rst)
        .color_order(ColorOrder::Rgb)
        .invert_colors(ColorInversion::Inverted)
        .init(&mut Delay)
        .expect("ST7789 init");

    display.clear(Rgb565::BLACK).expect("clear");

    let recv = MULTICORE_IMAGE_CHANNEL.receiver();
    loop {
        let cmd = recv.receive().await;
        handle_display_cmd(&mut display, device, &mut backlight, cmd);
    }
}
