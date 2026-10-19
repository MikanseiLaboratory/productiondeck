//! Cora TCP server over an `embassy-net` stack.
//!
//! The W5500 and CYW43 backends only need to build a `Stack` and call this
//! function. Each accepted connection is handed to the transport-independent
//! [`crate::network::run_cora`] loop.

#[cfg(feature = "cora-net")]
use embassy_net::tcp::TcpSocket;
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

#[cfg(feature = "cora-net")]
use crate::network::run_cora;

/// Cora TCP port.
pub const CORA_PORT: u16 = 5343;

/// Accept Cora connections forever.
#[cfg(feature = "cora-net")]
pub async fn serve<const RX: usize, const IMG: usize>(
    stack: Stack<'static>,
    model: &'static ModelInfo,
    identity: Identity<'static>,
    mut input: Receiver<'static, ThreadModeRawMutex, InputEvent, 8>,
) -> ! {
    let mut rx_buf = [0u8; 1024];
    let mut tx_buf = [0u8; 1024];
    loop {
        let mut socket = TcpSocket::new(stack, &mut rx_buf, &mut tx_buf);
        if socket.accept(CORA_PORT).await.is_err() {
            continue;
        }
        let _ = run_cora::<RX, IMG, _>(&mut socket, model, identity, &mut input).await;
        socket.close();
    }
}
