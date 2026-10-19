//! Shared firmware entry points for device-specific binaries.

#![allow(unreachable_code)]

use crate::buttons;
use crate::config::{self, MULTICORE_CHANNEL_SIZE};
use crate::device::{Device, DeviceConfig};
use crate::hardware;
use crate::supervisor::AppSupervisor;
use crate::types::DisplayCommand;
use crate::usb;
use defmt::{error, info, unwrap, warn};
use embassy_executor::{Executor, Spawner};
use embassy_rp::gpio::{Input, Level, Output, Pull};
use embassy_rp::multicore::{spawn_core1, Stack};
use embassy_rp::usb::Driver;
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::channel::Channel;
use static_cell::StaticCell;

// ---------------------------------------------------------------------------
// Single-core (`#[embassy_executor::main]`) entry
// ---------------------------------------------------------------------------

/// Initialize runtime device, hardware tasks, and run the supervisor loop.
pub async fn run_single_core(spawner: Spawner, device: Device) {
    config::init_runtime_device(device);
    let p = embassy_rp::init(Default::default());
    let mut supervisor = AppSupervisor::new_for_device(device);
    supervisor.print_startup_banner();
    match hardware::init_hardware_tasks_for_device(&spawner, p, device).await {
        Ok(()) => {
            info!("{} firmware initialized successfully", device.device_name());
            supervisor.print_init_success();
        }
        Err(e) => {
            error!("Failed to spawn hardware tasks: {:?}", e);
            core::panic!("Hardware initialization failed");
        }
    }
    supervisor.run().await;
}

/// Same as [`run_single_core`] without the extra `info!` after hardware init (minimal binaries).
pub async fn run_single_core_quiet(spawner: Spawner, device: Device) {
    config::init_runtime_device(device);
    let p = embassy_rp::init(Default::default());
    let mut supervisor = AppSupervisor::new_for_device(device);
    supervisor.print_startup_banner();
    match hardware::init_hardware_tasks_for_device(&spawner, p, device).await {
        Ok(()) => supervisor.print_init_success(),
        Err(_) => core::panic!("Hardware init failed"),
    }
    supervisor.run().await;
}

// ---------------------------------------------------------------------------
// Multicore (`cortex_m_rt::entry` + dual executors)
// ---------------------------------------------------------------------------

/// Cross-core display pipeline (not yet wired from core 0).
pub static MULTICORE_IMAGE_CHANNEL: Channel<
    CriticalSectionRawMutex,
    DisplayCommand,
    MULTICORE_CHANNEL_SIZE,
> = Channel::new();

static mut CORE1_STACK: Stack<4096> = Stack::new();
static EXECUTOR0: StaticCell<Executor> = StaticCell::new();
static EXECUTOR1: StaticCell<Executor> = StaticCell::new();

/// GPIO / button wiring for multicore Stream Deck–style builds.
#[derive(Clone, Copy)]
pub enum MulticoreCore0Layout {
    /// Mini / Module 6: USB LED `PIN_20`, heartbeat `PIN_25` + `PIN_21`, six direct inputs.
    MiniOrModule6Direct,
    /// Module 15: USB `PIN_20`, status `PIN_25` / `PIN_21`, 5×3 matrix.
    Module15Matrix,
    /// Module 32: USB LED `PIN_25`, status `PIN_20` / `PIN_21`, 8×4 matrix.
    Module32Matrix,
}

/// Core 1 image buffer size for the display stub loop.
#[derive(Clone, Copy)]
pub enum MulticoreCore1Buffer {
    /// 8 KiB (Mini, Module 6, Module 15).
    B8192,
    /// 16 KiB (Module 32).
    B16384,
}

/// Multicore bring-up: core 1 runs display stub; core 0 runs USB, buttons, supervisor.
pub fn run_multicore(
    device: Device,
    layout: MulticoreCore0Layout,
    core1_buf: MulticoreCore1Buffer,
) -> ! {
    let p = embassy_rp::init(Default::default());
    config::init_runtime_device(device);
    config::init_usb_serial_from_flash(p.FLASH);

    let supervisor = AppSupervisor::new_for_device(device);
    supervisor.print_startup_banner();

    spawn_core1(
        p.CORE1,
        unsafe { &mut *core::ptr::addr_of_mut!(CORE1_STACK) },
        move || {
            let executor1 = EXECUTOR1.init(Executor::new());
            executor1.run(|spawner| match core1_buf {
                MulticoreCore1Buffer::B8192 => {
                    unwrap!(multicore_core1_image_task_8192(device).map(|t| spawner.spawn(t)));
                }
                MulticoreCore1Buffer::B16384 => {
                    unwrap!(multicore_core1_image_task_16384(device).map(|t| spawner.spawn(t)));
                }
            });
        },
    );

    let executor0 = EXECUTOR0.init(Executor::new());
    executor0.run(|spawner| {
        unwrap!(multicore_core0_supervisor_task(supervisor).map(|t| spawner.spawn(t)));
        match layout {
            MulticoreCore0Layout::MiniOrModule6Direct => {
                unwrap!(usb::usb_task_for_device(
                    Driver::new(p.USB, crate::Irqs),
                    Output::new(p.PIN_20, Level::Low),
                    device,
                )
                .map(|t| spawner.spawn(t)));
                unwrap!(buttons::button_task_direct({
                    let mut inputs = heapless::Vec::new();
                    let _ = inputs.push(Input::new(p.PIN_4, Pull::Up));
                    let _ = inputs.push(Input::new(p.PIN_5, Pull::Up));
                    let _ = inputs.push(Input::new(p.PIN_6, Pull::Up));
                    let _ = inputs.push(Input::new(p.PIN_10, Pull::Up));
                    let _ = inputs.push(Input::new(p.PIN_11, Pull::Up));
                    let _ = inputs.push(Input::new(p.PIN_12, Pull::Up));
                    inputs
                })
                .map(|t| spawner.spawn(t)));
                unwrap!(hardware::status_task(
                    Output::new(p.PIN_25, Level::Low),
                    Output::new(p.PIN_21, Level::Low),
                )
                .map(|t| spawner.spawn(t)));
            }
            MulticoreCore0Layout::Module15Matrix => {
                unwrap!(usb::usb_task_for_device(
                    Driver::new(p.USB, crate::Irqs),
                    Output::new(p.PIN_20, Level::Low),
                    device,
                )
                .map(|t| spawner.spawn(t)));
                unwrap!(buttons::button_task_matrix_5x3(
                    Output::new(p.PIN_2, Level::High),
                    Output::new(p.PIN_3, Level::High),
                    Output::new(p.PIN_7, Level::High),
                    Input::new(p.PIN_4, Pull::Up),
                    Input::new(p.PIN_5, Pull::Up),
                    Input::new(p.PIN_6, Pull::Up),
                    Input::new(p.PIN_10, Pull::Up),
                    Input::new(p.PIN_11, Pull::Up),
                )
                .map(|t| spawner.spawn(t)));
                unwrap!(hardware::status_task(
                    Output::new(p.PIN_25, Level::Low),
                    Output::new(p.PIN_21, Level::Low),
                )
                .map(|t| spawner.spawn(t)));
            }
            MulticoreCore0Layout::Module32Matrix => {
                unwrap!(usb::usb_task_for_device(
                    Driver::new(p.USB, crate::Irqs),
                    Output::new(p.PIN_25, Level::Low),
                    device,
                )
                .map(|t| spawner.spawn(t)));
                unwrap!(buttons::button_task_matrix_8x4(
                    Output::new(p.PIN_2, Level::High),
                    Output::new(p.PIN_3, Level::High),
                    Output::new(p.PIN_7, Level::High),
                    Output::new(p.PIN_9, Level::High),
                    Input::new(p.PIN_4, Pull::Up),
                    Input::new(p.PIN_5, Pull::Up),
                    Input::new(p.PIN_6, Pull::Up),
                    Input::new(p.PIN_10, Pull::Up),
                    Input::new(p.PIN_11, Pull::Up),
                    Input::new(p.PIN_12, Pull::Up),
                    Input::new(p.PIN_13, Pull::Up),
                    Input::new(p.PIN_16, Pull::Up),
                )
                .map(|t| spawner.spawn(t)));
                unwrap!(hardware::status_task(
                    Output::new(p.PIN_20, Level::Low),
                    Output::new(p.PIN_21, Level::Low),
                )
                .map(|t| spawner.spawn(t)));
            }
        }
    });

    loop {
        cortex_m::asm::wfe();
    }
}

#[embassy_executor::task]
async fn multicore_core0_supervisor_task(mut supervisor: AppSupervisor) {
    let device = supervisor.device();
    info!(
        "Core 0: USB + buttons (multicore) — {}",
        device.device_name()
    );
    supervisor.print_init_success();
    supervisor.run().await;
}

async fn multicore_core1_image_loop(device: Device, buf: &mut [u8]) {
    info!("Core 1: image/display stub for {}", device.device_name());
    match hardware::init_hardware_tasks_core1(device).await {
        Ok(()) => info!("Core 1: image pipeline init OK"),
        Err(e) => {
            error!("Core 1: init failed: {:?}", e);
            core::panic!("Image processing initialization failed");
        }
    }
    let receiver = MULTICORE_IMAGE_CHANNEL.receiver();
    loop {
        match receiver.receive().await {
            DisplayCommand::DisplayImage { key_id, data } => {
                info!("Core 1: image key {} ({} bytes)", key_id, data.len());
                if data.len() <= buf.len() {
                    let copy_len = data.len().min(buf.len());
                    buf[..copy_len].copy_from_slice(&data[..copy_len]);
                } else {
                    warn!("Core 1: image too large ({} > {})", data.len(), buf.len());
                }
            }
            DisplayCommand::SetBrightness(brightness) => {
                info!("Core 1: brightness {}%", brightness);
            }
            DisplayCommand::ClearAll => info!("Core 1: clear all (stub)"),
            DisplayCommand::Clear(key_id) => info!("Core 1: clear key {} (stub)", key_id),
            _ => {}
        }
    }
}

#[embassy_executor::task]
async fn multicore_core1_image_task_8192(device: Device) {
    let mut image_processing_buffer = [0u8; 8192];
    multicore_core1_image_loop(device, &mut image_processing_buffer).await;
}

#[embassy_executor::task]
async fn multicore_core1_image_task_16384(device: Device) {
    let mut image_processing_buffer = [0u8; 16384];
    multicore_core1_image_loop(device, &mut image_processing_buffer).await;
}
