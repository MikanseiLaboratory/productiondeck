//! USB HID implementation for StreamDeck compatibility
//!
//! This module implements a flexible USB HID protocol that supports multiple
//! StreamDeck device types through device abstraction and protocol handlers.

use crate::channels::{BUTTON_CHANNEL, DISPLAY_CHANNEL, USB_COMMAND_CHANNEL};
use crate::config;
use crate::device::{Device, DeviceConfig};
use crate::protocol::module::ModuleSetCommand;
use crate::protocol::{OutputReportResult, ProtocolHandler};
use crate::types::{DisplayCommand, UsbCommand};
use defmt::*;
use embassy_rp::gpio::Output;
use embassy_rp::peripherals;
use embassy_rp::usb::Driver;
use embassy_time::{Duration, Timer};
use embassy_usb::class::hid::{
    Config as HidConfig, HidReaderWriter, ReportId, RequestHandler, State,
};
use embassy_usb::control::OutResponse;
use embassy_usb::{Builder, Config};

// ===================================================================
// USB Configuration
// ===================================================================

fn create_usb_config_for_device(device: Device) -> Config<'static> {
    let usb_config_data = device.usb_config();
    let mut usb_config = Config::new(usb_config_data.vid, usb_config_data.pid);
    usb_config.manufacturer = Some(usb_config_data.manufacturer);
    usb_config.product = Some(usb_config_data.product_name);
    usb_config.serial_number = Some(config::USB_SERIAL);
    usb_config.max_power = 100; // 200mA (matches real StreamDeck devices)
    usb_config.max_packet_size_0 = 64;
    usb_config.device_class = 0x00; // Interface-defined (HID class will be set in interface)
    usb_config.device_sub_class = 0x00;
    usb_config.device_protocol = 0x00;
    usb_config.composite_with_iads = false;

    // Set device version to match real StreamDeck devices
    usb_config.device_release = config::USB_BCD_DEVICE;

    usb_config
}

// ===================================================================
// HID Request Handler
// ===================================================================

struct StreamDeckHidHandler {
    protocol_handler: ProtocolHandler,
    usb_command_sender: embassy_sync::channel::Sender<
        'static,
        embassy_sync::blocking_mutex::raw::ThreadModeRawMutex,
        UsbCommand,
        4,
    >,
}

impl StreamDeckHidHandler {
    fn new_for_device(device: Device) -> Self {
        let protocol_version = device.usb_config().protocol;
        let protocol_handler = ProtocolHandler::create(protocol_version);

        Self {
            protocol_handler,
            usb_command_sender: USB_COMMAND_CHANNEL.sender(),
        }
    }
}

impl RequestHandler for StreamDeckHidHandler {
    fn get_report(&mut self, id: ReportId, buf: &mut [u8]) -> Option<usize> {
        info!("HID Get Report: ID={:?}, buf_len={}", id, buf.len());

        match id {
            ReportId::In(_) => {
                // Button state will be sent via separate input reports
                None
            }
            ReportId::Feature(report_id) => {
                // Delegate fully to protocol handler; no fallback here
                self.protocol_handler.get_feature_report(report_id, buf)
            }
            _ => None,
        }
    }

    fn set_report(&mut self, id: ReportId, data: &[u8]) -> OutResponse {
        info!("HID Set Report: ID={:?}, len={}", id, data.len());

        match id {
            ReportId::Feature(report_id) => {
                if let Some(command) = self.protocol_handler.handle_feature_report(report_id, data)
                {
                    match command {
                        ModuleSetCommand::Reset => {
                            info!("Processing reset command");
                            let _ = self.usb_command_sender.try_send(UsbCommand::Reset);
                        }
                        ModuleSetCommand::SetBrightness { value } => {
                            info!("Processing brightness command: {}%", value);
                            let _ = self
                                .usb_command_sender
                                .try_send(UsbCommand::SetBrightness(value));
                        }
                        ModuleSetCommand::SetIdleTime { seconds } => {
                            crate::config::set_idle_time_seconds(seconds);
                            info!("Set idle time to {} seconds", seconds);
                        }
                        _ => {}
                    }
                }
            }
            ReportId::Out(_) => {
                self.handle_output_report(data);
            }
            _ => {}
        }

        OutResponse::Accepted
    }
}

impl StreamDeckHidHandler {
    fn handle_output_report(&mut self, data: &[u8]) {
        debug!("USB Output Report: {} bytes received", data.len());
        if data.len() >= 8 {
            debug!(
                "Header: [{:02X}, {:02X}, {:02X}, {:02X}, {:02X}, {:02X}, {:02X}, {:02X}]",
                data[0], data[1], data[2], data[3], data[4], data[5], data[6], data[7]
            );
        }

        match self.protocol_handler.parse_output_report(data) {
            OutputReportResult::KeyImageComplete { key_id, image } => {
                info!("Image complete for key {} ({} bytes)", key_id, image.len());
                let _ = self.usb_command_sender.try_send(UsbCommand::ImageData {
                    key_id,
                    data: image,
                });
            }
            OutputReportResult::FullScreenImageChunk => {
                debug!("Full screen image chunk received (not assembled)");
            }
            OutputReportResult::BootLogoImageChunk => {
                debug!("Boot logo image chunk received (not assembled)");
            }
            OutputReportResult::Unhandled => {
                debug!("Unhandled output report");
            }
        }
    }
}

// ===================================================================
// USB Task Implementation
// ===================================================================

#[embassy_executor::task]
pub async fn usb_task(driver: Driver<'static, peripherals::USB>, usb_led: Output<'static>) {
    usb_task_impl(driver, usb_led, config::get_current_device()).await
}

#[embassy_executor::task]
pub async fn usb_task_for_device(
    driver: Driver<'static, peripherals::USB>,
    usb_led: Output<'static>,
    device: Device,
) {
    usb_task_impl(driver, usb_led, device).await
}

async fn usb_task_impl(
    driver: Driver<'static, peripherals::USB>,
    mut usb_led: Output<'static>,
    device: Device,
) {
    info!("USB task started");

    info!("USB HID device: {}", device.device_name());
    info!("Protocol: {:?}", device.usb_config().protocol);
    info!(
        "Button layout: {}x{} ({} keys)",
        device.button_layout().cols,
        device.button_layout().rows,
        device.button_layout().total_keys
    );

    // Create USB configuration for specific device
    let usb_config = create_usb_config_for_device(device);

    // Create USB builder
    static mut DEVICE_DESC_BUF: [u8; 256] = [0; 256];
    static mut CONFIG_DESC_BUF: [u8; 256] = [0; 256];
    static mut BOS_DESC_BUF: [u8; 256] = [0; 256];
    static mut CONTROL_BUF: [u8; 512] = [0; 512];
    let mut builder = unsafe {
        #[allow(static_mut_refs)]
        Builder::new(
            driver,
            usb_config,
            &mut DEVICE_DESC_BUF,
            &mut CONFIG_DESC_BUF,
            &mut BOS_DESC_BUF,
            &mut CONTROL_BUF,
        )
    };

    // Create HID request handler for specific device
    static mut REQUEST_HANDLER: Option<StreamDeckHidHandler> = None;
    unsafe {
        REQUEST_HANDLER = Some(StreamDeckHidHandler::new_for_device(device));
    }

    // Get HID descriptor from protocol handler
    let protocol_handler = ProtocolHandler::create(device.usb_config().protocol);
    let hid_descriptor = protocol_handler.hid_descriptor();

    let hid_config = HidConfig {
        report_descriptor: hid_descriptor,
        #[allow(static_mut_refs)]
        request_handler: unsafe { REQUEST_HANDLER.as_mut().map(|h| h as _) },
        poll_ms: config::USB_POLL_RATE_MS as u8,
        max_packet_size: 64, // RP2040 USB hardware limitation
    };

    info!(
        "HID configuration created with report descriptor size: {} bytes",
        hid_descriptor.len()
    );

    static mut HID_STATE: State = State::new();
    #[allow(static_mut_refs)]
    let hid =
        unsafe { HidReaderWriter::<_, 64, 4096>::new(&mut builder, &mut HID_STATE, hid_config) };

    // Build USB device
    let mut usb = builder.build();

    // Split HID into reader and writer
    let (mut reader, mut writer) = hid.split();

    // Spawn USB device task
    let usb_fut = usb.run();

    // Spawn USB command processor
    let command_fut = async {
        info!("USB command processor started");
        let receiver = USB_COMMAND_CHANNEL.receiver();
        loop {
            match receiver.receive().await {
                UsbCommand::Reset => {
                    info!("Processing reset command");
                    let _ = DISPLAY_CHANNEL
                        .sender()
                        .send(DisplayCommand::ClearAll)
                        .await;
                }
                UsbCommand::SetBrightness(brightness) => {
                    info!("Processing brightness command: {}%", brightness);
                    let _ = DISPLAY_CHANNEL
                        .sender()
                        .send(DisplayCommand::SetBrightness(brightness))
                        .await;
                }
                UsbCommand::ImageData { key_id, data } => {
                    debug!(
                        "Processing image data for key {} ({} bytes)",
                        key_id,
                        data.len()
                    );
                    // Send to core 1 for processing via inter-core channel
                    // TODO: Replace with actual inter-core channel when implemented
                    let _ = DISPLAY_CHANNEL
                        .sender()
                        .send(DisplayCommand::DisplayImage { key_id, data })
                        .await;
                }
            }
        }
    };

    // Spawn combined IO future: send button reports and read OUT image packets
    let io_fut = async {
        let receiver = BUTTON_CHANNEL.receiver();
        let protocol_handler = ProtocolHandler::create(device.usb_config().protocol);

        // OUT image reader protocol state
        let mut out_protocol = ProtocolHandler::create(device.usb_config().protocol);
        let mut out_buf = [0u8; 4096];

        // Button sender loop
        let button_loop = async {
            loop {
                let button_state = receiver.receive().await;

                if button_state.changed {
                    let layout = device.button_layout();
                    let button_mapping = protocol_handler.map_buttons(
                        &button_state.buttons,
                        layout.cols,
                        layout.rows,
                        layout.left_to_right,
                    );

                    let mut report = [0u8; 64]; // RP2040 USB hardware limitation
                    let report_len =
                        protocol_handler.format_button_report(&button_mapping, &mut report);

                    if report_len > 0 {
                        match writer.write(&report[..report_len]).await {
                            Ok(()) => {
                                debug!("Button report sent ({} bytes)", report_len);
                            }
                            Err(e) => {
                                warn!("Failed to send button report: {:?}", e);
                            }
                        }
                    }
                }
            }
        };

        // OUT endpoint reader loop
        let out_loop = async {
            loop {
                match reader.read(&mut out_buf).await {
                    Ok(n) => {
                        let data = &out_buf[..n];
                        if !data.is_empty() {
                            match out_protocol.parse_output_report(data) {
                                OutputReportResult::KeyImageComplete { key_id, image } => {
                                    let img_len = image.len();
                                    let _ = USB_COMMAND_CHANNEL.sender().try_send(
                                        UsbCommand::ImageData {
                                            key_id,
                                            data: image,
                                        },
                                    );
                                    info!("Image complete for key {} ({} bytes)", key_id, img_len);
                                }
                                OutputReportResult::FullScreenImageChunk => {}
                                OutputReportResult::BootLogoImageChunk => {}
                                OutputReportResult::Unhandled => {}
                            }
                        }
                    }
                    Err(e) => {
                        warn!("HID OUT read error: {:?}", e);
                    }
                }
            }
        };

        embassy_futures::join::join(button_loop, out_loop).await;
    };

    // USB status LED control
    let led_fut = async {
        info!("USB LED task started");
        usb_led.set_high();
        loop {
            Timer::after(Duration::from_secs(1)).await;
        }
    };

    // Run all futures concurrently
    embassy_futures::join::join4(usb_fut, command_fut, io_fut, led_fut).await;
}
