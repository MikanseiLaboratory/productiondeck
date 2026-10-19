#![allow(unreachable_code)]
//! ProductionDeck — StreamDeck Mini (PID `0x0063`), V1 BMP, 3×2 keys.

#![no_std]
#![no_main]

use cortex_m_rt::entry;
use defmt_rtt as _;
use panic_halt as _;
use productiondeck::device::Device;
use productiondeck::entry::{run_multicore, MulticoreCore0Layout, MulticoreCore1Buffer};

const DEVICE: Device = Device::Mini;

#[entry]
fn main() -> ! {
    run_multicore(
        DEVICE,
        MulticoreCore0Layout::MiniOrModule6Direct,
        MulticoreCore1Buffer::B8192,
    )
}
