//! Cora TCP server used by the emulator.
//!
//! One task per connection. Each connection owns a [`CoraSession`], so read and
//! write never block each other and a slow client cannot stall another.

use std::net::SocketAddr;
use std::sync::OnceLock;
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use productiondeck_core::cora::{CoraError, CoraSession};
use productiondeck_core::feature::Identity;
use productiondeck_core::input::InputEvent;
use productiondeck_core::model::ModelInfo;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{broadcast, mpsc};
use tracing::{debug, info, warn};

use crate::command::{CommandSink, DeckCommand};

/// Maximum Cora frame we expect to send (16-byte header + 1024-byte payload).
const TX_BUF: usize = 16 + 1024;

/// A running Cora server.
pub struct CoraServer {
    input: broadcast::Sender<InputEvent>,
    local_addr: SocketAddr,
}

impl CoraServer {
    /// Bind `addr` and start accepting connections.
    pub async fn bind(
        addr: impl tokio::net::ToSocketAddrs,
        model: &'static ModelInfo,
        identity: Identity<'static>,
        command_tx: mpsc::UnboundedSender<DeckCommand>,
    ) -> Result<Self> {
        let listener = TcpListener::bind(addr)
            .await
            .context("bind Cora TCP listener")?;
        let local_addr = listener.local_addr()?;
        let (input, _) = broadcast::channel(64);
        let input_tx = input.clone();

        tokio::spawn(async move {
            loop {
                match listener.accept().await {
                    Ok((stream, peer)) => {
                        debug!("Cora client connected: {peer}");
                        let rx = input_tx.subscribe();
                        let commands = command_tx.clone();
                        tokio::spawn(async move {
                            if let Err(e) = connection(stream, model, identity, commands, rx).await
                            {
                                warn!("Cora connection ended: {e:#}");
                            }
                        });
                    }
                    Err(e) => {
                        warn!("accept failed: {e}");
                        tokio::time::sleep(Duration::from_millis(100)).await;
                    }
                }
            }
        });

        Ok(Self { input, local_addr })
    }

    /// Local address the server is listening on.
    pub fn local_addr(&self) -> SocketAddr {
        self.local_addr
    }

    /// Broadcast an input event to every connected client.
    pub fn input(&self, event: InputEvent) {
        let _ = self.input.send(event);
    }
}

async fn connection(
    stream: TcpStream,
    model: &'static ModelInfo,
    identity: Identity<'static>,
    command_tx: mpsc::UnboundedSender<DeckCommand>,
    mut input_rx: broadcast::Receiver<InputEvent>,
) -> Result<()> {
    stream.set_nodelay(true).ok();
    let (mut reader, mut writer) = stream.into_split();
    let mut session = CoraSession::<8192, 16384>::new(model, identity);
    session.start(now_ms());
    let mut sink = CommandSink::new(command_tx);
    let mut buf = [0u8; 4096];
    let mut out = [0u8; TX_BUF];
    let mut tick = tokio::time::interval(Duration::from_millis(100));
    tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

    flush(&mut session, &mut writer, &mut out).await?;

    loop {
        tokio::select! {
            read = reader.read(&mut buf) => {
                let n = read?;
                if n == 0 {
                    info!("Cora client disconnected");
                    break;
                }
                session
                    .feed(&buf[..n], &mut sink)
                    .map_err(|e| anyhow::anyhow!("feed: {e:?}"))?;
                flush(&mut session, &mut writer, &mut out).await?;
            }
            event = input_rx.recv() => {
                match event {
                    Ok(event) => {
                        session.push_input(event);
                        flush(&mut session, &mut writer, &mut out).await?;
                    }
                    Err(broadcast::error::RecvError::Lagged(_)) => continue,
                    Err(broadcast::error::RecvError::Closed) => break,
                }
            }
            _ = tick.tick() => {
                let now = now_ms();
                session.tick(now);
                if session.timed_out(now) {
                    info!("Cora client timed out");
                    break;
                }
                flush(&mut session, &mut writer, &mut out).await?;
            }
        }
    }

    session.on_disconnect();
    Ok(())
}

async fn flush<W: tokio::io::AsyncWrite + Unpin>(
    session: &mut CoraSession<'_, 8192, 16384>,
    writer: &mut W,
    out: &mut [u8; TX_BUF],
) -> Result<()> {
    while let Some(n) = session.poll_transmit(out) {
        writer.write_all(&out[..n]).await?;
    }
    Ok(())
}

fn now_ms() -> u32 {
    static START: OnceLock<Instant> = OnceLock::new();
    START.get_or_init(Instant::now).elapsed().as_millis() as u32
}

/// Convert a core Cora error into a readable string (used in logs/tests).
pub fn describe_error(error: CoraError) -> &'static str {
    match error {
        CoraError::BufferFull => "buffer full",
        CoraError::UnknownHidOp => "unknown hidOp",
        CoraError::Malformed => "malformed frame",
        CoraError::Protocol(_) => "protocol error",
    }
}
