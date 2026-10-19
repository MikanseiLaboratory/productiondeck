//! Cora server glue for firmware.
//!
//! The protocol state machine lives in `productiondeck_core::cora`. This module
//! only moves bytes between a byte stream (embassy-net TCP, CYW43, or a test
//! pipe) and the session, and drives keep-alive/timeout ticks.
//!
//! The stream is intentionally generic over `embedded-io-async`, so the W5500
//! and CYW43 backends can share it without the core depending on networking.

use embassy_futures::select::{select, Either};
use embassy_sync::blocking_mutex::raw::ThreadModeRawMutex;
use embassy_sync::channel::Receiver;
use embassy_time::{with_timeout, Duration, Instant};
use embedded_io_async::{Read, Write};

use productiondeck_core::cora::{CoraError, CoraSession};
use productiondeck_core::feature::Identity;
use productiondeck_core::input::InputEvent;
use productiondeck_core::model::ModelInfo;
use productiondeck_core::protocol::NullSink;

#[cfg(feature = "cora-cyw43")]
pub mod cyw43;
pub mod tcp;
#[cfg(feature = "cora-w5500")]
pub mod w5500;

/// How often keep-alive / timeout state is advanced while idle.
pub const TICK: Duration = Duration::from_millis(100);

/// Run one Cora connection until EOF, timeout, or a transport error.
pub async fn run_cora<const RX: usize, const IMG: usize, S>(
    mut stream: S,
    model: &'static ModelInfo,
    identity: Identity<'static>,
    input: &mut Receiver<'static, ThreadModeRawMutex, InputEvent, 8>,
) -> Result<(), CoraError>
where
    S: Read + Write,
{
    let mut session = CoraSession::<RX, IMG>::new(model, identity);
    session.start(0);
    let mut sink = NullSink;
    let mut rx = [0u8; 4096];
    let mut tx = [0u8; 1040];

    loop {
        let now = Instant::now().as_millis() as u32;
        session.tick(now);
        while let Some(n) = session.poll_transmit(&mut tx) {
            stream
                .write_all(&tx[..n])
                .await
                .map_err(|_| CoraError::Malformed)?;
        }
        if session.timed_out(now) {
            return Ok(());
        }

        match with_timeout(TICK, select(stream.read(&mut rx), input.receive())).await {
            Ok(Either::First(Ok(n))) if n > 0 => {
                session.feed(&rx[..n], &mut sink)?;
            }
            Ok(Either::First(Ok(_))) => return Ok(()),
            Ok(Either::First(Err(_))) => return Ok(()),
            Ok(Either::Second(event)) => session.push_input(event),
            Err(_) => {}
        }
    }
}
