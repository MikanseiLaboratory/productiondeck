#![allow(unreachable_code)]
//! ProductionDeck — Stream Deck Module 6 (PID `0x00B8`), 6 keys.

#![no_std]
#![no_main]

use cortex_m_rt::entry;
use defmt_rtt as _;
use panic_halt as _;
use productiondeck::device::Device;
use productiondeck::entry::{run_multicore, MulticoreCore0Layout, MulticoreCore1Buffer};

const DEVICE: Device = Device::Module6Keys;

#[entry]
fn main() -> ! {
    run_multicore(
        DEVICE,
        MulticoreCore0Layout::MiniOrModule6Direct,
        MulticoreCore1Buffer::B8192,
    )
}
