//! Display driver for shared ST7735 TFT display
//!
//! This module manages a single 216x144 display divided into 6 regions (72x72 each)
//! to simulate individual key displays like the StreamDeck Mini.

#![allow(dead_code)]

use defmt::*;
use embassy_rp::gpio::Output;
use embassy_rp::peripherals;
use embassy_rp::spi::Spi;
use embassy_time::{Duration, Timer};
use heapless::Vec;

use crate::channels::DISPLAY_CHANNEL;
use crate::config::*;
use crate::types::DisplayCommand;

// ===================================================================
// Display Controller Structure
// ===================================================================

struct DisplayController {
    spi: Spi<'static, peripherals::SPI0, embassy_rp::spi::Blocking>,
    cs: Output<'static>,
    dc: Output<'static>,
    rst: Output<'static>,
    // backlight: Pwm<'static, PWM0>,
    current_brightness: u8,
}

impl DisplayController {
    async fn new(
        spi: Spi<'static, peripherals::SPI0, embassy_rp::spi::Blocking>,
        cs: Output<'static>,
        dc: Output<'static>,
        rst: Output<'static>,
        _bl: Output<'static>,
    ) -> Self {
        info!("Initializing display controller");

        let mut controller = Self {
            spi,
            cs,
            dc,
            rst,
            current_brightness: crate::config::display_brightness(),
        };

        // Initialize the display
        controller.init_display().await;

        controller
    }

    async fn init_display(&mut self) {
        info!(
            "Initializing shared display ({}x{})",
            crate::config::display_total_width(),
            crate::config::display_total_height()
        );

        // Select the display
        self.cs.set_low();

        // Reset the display
        self.rst.set_low();
        Timer::after(Duration::from_millis(10)).await;
        self.rst.set_high();
        Timer::after(Duration::from_millis(120)).await;

        // Initialization sequence for ST7735
        self.send_command(ST7735_SWRESET).await; // Software reset
        Timer::after(Duration::from_millis(150)).await;

        self.send_command(ST7735_SLPOUT).await; // Sleep out
        Timer::after(Duration::from_millis(120)).await;

        // Color mode - 16 bit RGB565
        self.send_command(ST7735_COLMOD).await;
        self.send_data(&[ST7735_COLOR_MODE_16BIT]).await;

        // Column address set (0 to display_total_width-1)
        self.send_command(ST7735_CASET).await;
        let width_bytes = (crate::config::display_total_width() - 1) as u16;
        self.send_data(&[
            0x00,
            0x00, // Start column (0)
            (width_bytes >> 8) as u8,
            (width_bytes & 0xFF) as u8, // End column
        ])
        .await;

        // Row address set (0 to display_total_height-1)
        self.send_command(ST7735_RASET).await;
        let height_bytes = (crate::config::display_total_height() - 1) as u16;
        self.send_data(&[
            0x00,
            0x00, // Start row (0)
            (height_bytes >> 8) as u8,
            (height_bytes & 0xFF) as u8, // End row
        ])
        .await;

        // Display inversion off
        self.send_command(ST7735_INVOFF).await;

        // Normal display mode
        self.send_command(ST7735_NORON).await;

        // Display on
        self.send_command(ST7735_DISPON).await;
        Timer::after(Duration::from_millis(10)).await;

        // Deselect display
        self.cs.set_high();

        info!("Shared display initialization complete");

        // Clear the entire display
        self.clear_all().await;
    }

    async fn send_command(&mut self, command: u8) {
        // Set DC pin low for command mode
        self.dc.set_low();

        // Send command byte
        let _ = self.spi.blocking_write(&[command]);
    }

    async fn send_data(&mut self, data: &[u8]) {
        // Set DC pin high for data mode
        self.dc.set_high();

        // Send data
        let _ = self.spi.blocking_write(data);
    }

    async fn set_window(&mut self, x_start: u16, y_start: u16, x_end: u16, y_end: u16) {
        // Column address set
        self.send_command(ST7735_CASET).await;
        self.send_data(&[
            (x_start >> 8) as u8,
            (x_start & 0xFF) as u8,
            (x_end >> 8) as u8,
            (x_end & 0xFF) as u8,
        ])
        .await;

        // Row address set
        self.send_command(ST7735_RASET).await;
        self.send_data(&[
            (y_start >> 8) as u8,
            (y_start & 0xFF) as u8,
            (y_end >> 8) as u8,
            (y_end & 0xFF) as u8,
        ])
        .await;

        // Memory write
        self.send_command(ST7735_RAMWR).await;
    }

    async fn display_image(&mut self, key_id: u8, image_data: &[u8]) {
        if key_id >= crate::config::streamdeck_keys() as u8 {
            warn!("Invalid key_id: {}", key_id);
            return;
        }

        info!("Displaying image on key {} region", key_id);

        // Calculate position on shared display
        let cols = crate::config::streamdeck_cols();
        let col = (key_id as usize) % cols;
        let row = (key_id as usize) / cols;
        let image_size = crate::config::key_image_size();
        let x_start = (col * image_size) as u16;
        let y_start = (row * image_size) as u16;
        let x_end = x_start + image_size as u16 - 1;
        let y_end = y_start + image_size as u16 - 1;

        debug!(
            "Key {} maps to region: ({},{}) to ({},{})",
            key_id, x_start, y_start, x_end, y_end
        );

        // Select the display
        self.cs.set_low();

        // Set window to key region
        self.set_window(x_start, y_start, x_end, y_end).await;

        // Process image data - skip BMP header if present
        let mut data_offset = 0;
        if image_data.len() > 54 && image_data[0] == 0x42 && image_data[1] == 0x4D {
            data_offset = 54; // Skip BMP header
            debug!("Skipped BMP header");
        }

        let rgb_data = &image_data[data_offset..];
        let expected_size = image_size * image_size * 3;

        if rgb_data.len() < expected_size {
            warn!(
                "Image data too small: {} bytes, expected: {}",
                rgb_data.len(),
                expected_size
            );
            self.cs.set_high();
            return;
        }

        // Convert RGB888 to RGB565 and send to display
        let pixel_count = image_size * image_size;
        let mut buffer = [0u8; 2]; // Buffer for one RGB565 pixel

        for i in 0..pixel_count {
            let rgb_offset = i * 3;
            if rgb_offset + 2 < rgb_data.len() {
                let r = rgb_data[rgb_offset];
                let g = rgb_data[rgb_offset + 1];
                let b = rgb_data[rgb_offset + 2];

                // Convert to RGB565
                let rgb565 = ((r as u16 & RGB565_RED_MASK) << 8)
                    | ((g as u16 & RGB565_GREEN_MASK) << 3)
                    | (b as u16 >> RGB565_BLUE_SHIFT);

                // Send as big-endian
                buffer[0] = (rgb565 >> 8) as u8;
                buffer[1] = (rgb565 & 0xFF) as u8;
                let _ = self.spi.blocking_write(&buffer);
            }
        }

        // Deselect display
        self.cs.set_high();

        info!(
            "Image displayed on key {} region: {} pixels",
            key_id, pixel_count
        );
    }

    async fn clear_key(&mut self, key_id: u8) {
        if key_id >= crate::config::streamdeck_keys() as u8 {
            warn!("Invalid key_id: {}", key_id);
            return;
        }

        debug!("Clearing key {} region", key_id);

        // Calculate position on shared display
        let cols = crate::config::streamdeck_cols();
        let col = (key_id as usize) % cols;
        let row = (key_id as usize) / cols;
        let image_size = crate::config::key_image_size();
        let x_start = (col * image_size) as u16;
        let y_start = (row * image_size) as u16;
        let x_end = x_start + image_size as u16 - 1;
        let y_end = y_start + image_size as u16 - 1;

        // Select the display
        self.cs.set_low();

        // Set window to key region
        self.set_window(x_start, y_start, x_end, y_end).await;

        // Fill region with black (RGB565: 0x0000)
        let black_pixel = [0x00, 0x00];
        for _ in 0..(image_size * image_size) {
            let _ = self.spi.blocking_write(&black_pixel);
        }

        // Deselect display
        self.cs.set_high();

        debug!("Key {} region cleared", key_id);
    }

    async fn clear_all(&mut self) {
        info!("Clearing entire display");

        // Select the display
        self.cs.set_low();

        // Set window to entire display
        self.set_window(
            0,
            0,
            crate::config::display_total_width() as u16 - 1,
            crate::config::display_total_height() as u16 - 1,
        )
        .await;

        // Fill entire display with black
        let black_pixel = [0x00, 0x00];
        for _ in 0..(crate::config::display_total_width() * crate::config::display_total_height()) {
            let _ = self.spi.blocking_write(&black_pixel);
        }

        // Deselect display
        self.cs.set_high();

        info!("Display cleared");
    }

    async fn set_brightness(&mut self, brightness: u8) {
        let brightness = brightness.min(100);
        self.current_brightness = brightness;

        // TODO: Implement PWM brightness control
        info!(
            "Brightness set to {}% (PWM not implemented yet)",
            brightness
        );
    }
}

// ===================================================================
// Image Buffer Management
// ===================================================================

struct ImageBuffer {
    data: Vec<u8, IMAGE_BUFFER_SIZE>,
    receiving: bool,
    complete: bool,
    expected_sequence: u16,
}

impl ImageBuffer {
    fn new() -> Self {
        Self {
            data: Vec::new(),
            receiving: false,
            complete: false,
            expected_sequence: 0,
        }
    }

    fn reset(&mut self) {
        self.data.clear();
        self.receiving = false;
        self.complete = false;
        self.expected_sequence = 0;
    }

    fn add_packet(&mut self, packet_data: &[u8]) -> Result<bool, &'static str> {
        if packet_data.len() < 8 {
            return Err("Packet too small");
        }

        let key_id = packet_data[2];
        let is_last = packet_data[3] != 0;
        let payload_len = u16::from_le_bytes([packet_data[4], packet_data[5]]);
        let sequence = u16::from_le_bytes([packet_data[6], packet_data[7]]);

        // Reset buffer on first packet
        if sequence == 0 {
            self.reset();
            self.receiving = true;
            debug!("Starting image reception for key {}", key_id);
        }

        // Validate sequence
        if !self.receiving || sequence != self.expected_sequence {
            error!(
                "Image packet sequence error: expected {}, got {}",
                self.expected_sequence, sequence
            );
            self.reset();
            return Err("Sequence error");
        }

        // Copy payload data
        let data_offset = 8;
        let copy_len = (payload_len as usize).min(packet_data.len() - data_offset);

        if self
            .data
            .extend_from_slice(&packet_data[data_offset..data_offset + copy_len])
            .is_err()
        {
            error!("Image buffer overflow");
            self.reset();
            return Err("Buffer overflow");
        }

        self.expected_sequence += 1;

        debug!(
            "Image packet key={} seq={} len={} total={}",
            key_id,
            sequence,
            copy_len,
            self.data.len()
        );

        if is_last {
            self.complete = true;
            self.receiving = false;
            info!(
                "Image complete for key {} ({} bytes)",
                key_id,
                self.data.len()
            );
            return Ok(true);
        }

        Ok(false)
    }
}

// ===================================================================
// Display Task Implementation
// ===================================================================

#[embassy_executor::task]
pub async fn display_task(
    spi: embassy_rp::spi::Spi<'static, peripherals::SPI0, embassy_rp::spi::Blocking>,
    cs: Output<'static>,
    dc: Output<'static>,
    rst: Output<'static>,
    bl: Output<'static>,
) {
    info!("Display task started");

    let mut controller = DisplayController::new(spi, cs, dc, rst, bl).await;

    let mut image_buffers: [ImageBuffer; 32] = Default::default(); // Max keys for any device

    // Initialize image buffers
    for buffer in &mut image_buffers {
        *buffer = ImageBuffer::new();
    }

    let receiver = DISPLAY_CHANNEL.receiver();

    info!("Display controller ready");

    loop {
        match receiver.receive().await {
            DisplayCommand::Clear(key_id) => {
                controller.clear_key(key_id).await;
            }
            DisplayCommand::ClearAll => {
                controller.clear_all().await;
            }
            DisplayCommand::SetBrightness(brightness) => {
                controller.set_brightness(brightness).await;
            }
            DisplayCommand::DisplayImage { key_id, data } => {
                if key_id < 32 {
                    // Max keys for any device
                    let buffer = &mut image_buffers[key_id as usize];

                    match buffer.add_packet(&data) {
                        Ok(true) => {
                            // Image complete, display it
                            controller.display_image(key_id, &buffer.data).await;
                            buffer.reset();
                        }
                        Ok(false) => {
                            // More packets expected
                            debug!("Partial image data received for key {}", key_id);
                        }
                        Err(e) => {
                            error!("Image processing error for key {}: {}", key_id, e);
                        }
                    }
                } else {
                    error!("Invalid key_id: {}", key_id);
                }
            }
        }
    }
}

// ===================================================================
// Default trait implementation for ImageBuffer array
// ===================================================================

impl Default for ImageBuffer {
    fn default() -> Self {
        Self::new()
    }
}
