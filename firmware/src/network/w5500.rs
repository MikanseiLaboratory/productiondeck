//! W5500 backend for the **W5500-EVB-Pico** style board.
//!
//! Wiring (WIZnet W5500-EVB-Pico, SPI0):
//! ```text
//! MISO GP16   CSn  GP17
//! SCK  GP18   MOSI GP19
//! RSTn GP20   INTn GP21
//! ```
//! All six pins are configurable here; the defaults match the EVB board.
//!
//! The driver builds an `embassy-net` stack, waits for DHCP and then hands the
//! stack to the shared Cora TCP server.

use embassy_executor::Spawner;
use embassy_net::{Config as NetConfig, StackResources};
use embassy_net_wiznet::chip::W5500;
use embassy_rp::gpio::{Input, Level, Output, Pull};
use embassy_rp::peripherals;
use embassy_rp::spi::{self, Spi};
use embedded_hal_bus::spi::{ExclusiveDevice, NoDelay};
use static_cell::StaticCell;

use productiondeck_core::feature::{FirmwareVersions, Identity};
use productiondeck_core::model::ModelInfo;

use crate::channels::SD_INPUT_CHANNEL;
use crate::network::tcp;

const W5500_MAC: [u8; 6] = [0x02, 0x00, 0x00, 0x00, 0x00, 0x01];

type W5500Spi = ExclusiveDevice<
    Spi<'static, peripherals::SPI0, embassy_rp::spi::Async>,
    Output<'static>,
    NoDelay,
>;
type W5500Runner = embassy_net_wiznet::Runner<'static, W5500, W5500Spi, Input<'static>, Output<'static>>;
type NetRunner = embassy_net::Runner<'static, embassy_net_wiznet::Device<'static>>;

static W5500_STATE: StaticCell<embassy_net_wiznet::State<2, 2>> = StaticCell::new();
static NET_RESOURCES: StaticCell<StackResources<3>> = StaticCell::new();

/// Bring up the W5500 and serve Cora. Never returns.
pub async fn run<const RX: usize, const IMG: usize>(
    spawner: Spawner,
    p: embassy_rp::Peripherals,
    model: &'static ModelInfo,
) -> ! {
    let embassy_rp::Peripherals {
        FLASH,
        SPI0,
        PIN_16,
        PIN_17,
        PIN_18,
        PIN_19,
        PIN_20,
        PIN_21,
        DMA_CH0,
        DMA_CH1,
        ..
    } = p;

    crate::config::init_usb_serial_from_flash(FLASH);
    let identity = Identity {
        model,
        serial: crate::config::usb_serial_str(),
        mac: W5500_MAC,
        versions: FirmwareVersions {
            ap2: "2.01.002",
            ap1: "1.00.000",
            ld: "1.00.000",
            encoder_ap2: "1.01.012",
            encoder_ld: "1.01.006",
        },
        child: None,
    };

    let mut spi_cfg = spi::Config::default();
    spi_cfg.frequency = 33_000_000;
    let spi = Spi::new(
        SPI0, PIN_18, PIN_19, PIN_16, DMA_CH0, DMA_CH1, crate::Irqs, spi_cfg,
    );
    let cs = Output::new(PIN_17, Level::High);
    let int = Input::new(PIN_21, Pull::Up);
    let rst = Output::new(PIN_20, Level::High);

    let spi_dev = ExclusiveDevice::new_no_delay(spi, cs).unwrap();
    let state = W5500_STATE.init(embassy_net_wiznet::State::new());
    let (device, runner) = embassy_net_wiznet::new::<2, 2, W5500, _, _, _>(
        W5500_MAC, state, spi_dev, int, rst,
    )
    .await
    .expect("W5500 init");

    let (stack, net_runner) = embassy_net::new(
        device,
        NetConfig::dhcpv4(Default::default()),
        NET_RESOURCES.init(StackResources::new()),
        0x1234_5678,
    );

    spawner.spawn(w5500_runner_task(runner).unwrap());
    spawner.spawn(net_task(net_runner).unwrap());

    stack.wait_config_up().await;
    tcp::serve::<RX, IMG>(stack, model, identity, SD_INPUT_CHANNEL.receiver()).await
}

#[embassy_executor::task]
async fn w5500_runner_task(runner: W5500Runner) {
    runner.run().await
}

#[embassy_executor::task]
async fn net_task(mut runner: NetRunner) {
    runner.run().await
}
