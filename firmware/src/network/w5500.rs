//! W5500 board backend.
//!
//! Driver initialization (SPI + DMA, DHCP) is board-specific; once a `Stack`
//! exists the Cora server is shared with the CYW43 backend.
//!
//! See `docs/network.md` for the `embassy-net-wiznet` API notes.

#[cfg(feature = "cora-net")]
use embassy_net::Stack;
#[cfg(feature = "cora-net")]
use embassy_sync::blocking_mutex::raw::ThreadModeRawMutex;
#[cfg(feature = "cora-net")]
use embassy_sync::channel::Receiver;

#[cfg(feature = "cora-net")]
use productiondeck_core::feature::Identity;
#[cfg(feature = "cora-net")]
use productiondeck_core::input::InputEvent;
#[cfg(feature = "cora-net")]
use productiondeck_core::model::ModelInfo;

/// Serve Cora over an already-configured W5500 stack.
#[cfg(feature = "cora-net")]
pub async fn serve<const RX: usize, const IMG: usize>(
    stack: Stack<'static>,
    model: &'static ModelInfo,
    identity: Identity<'static>,
    input: Receiver<'static, ThreadModeRawMutex, InputEvent, 8>,
) -> ! {
    crate::network::tcp::serve::<RX, IMG>(stack, model, identity, input).await
}
