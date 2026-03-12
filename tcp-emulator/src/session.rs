/// Single TCP session – manages one connected client.
///
/// Responsibilities:
/// - Send initial Cora keepalive to establish connection
/// - Send periodic keepalives (every 2 seconds)
/// - Handle incoming Cora messages:
///   - ACK_NAK: acknowledge our keepalives
///   - GET_REPORT: respond with feature report data
///   - SEND_REPORT / WRITE: log received commands (brightness, image, etc.)
/// - Send button input events on demand via a channel

use std::sync::Arc;
use std::sync::atomic::{AtomicU8, Ordering};
use bytes::Bytes;
use tokio::net::TcpStream;
use tokio::sync::mpsc;
use tokio::time::{self, Duration};
use tokio_util::codec::Framed;
use futures_util::{SinkExt, StreamExt};
use tracing::{debug, info, warn};

use crate::cora::{CoraCodec, CoraFlags, CoraMessage, HidOp};
use crate::feature_reports::{build_response, DeviceConfig};

static GLOBAL_CONN_NO: AtomicU8 = AtomicU8::new(0);

/// Commands sent from the CLI to a live session.
#[derive(Debug, Clone)]
pub enum SessionCommand {
    /// Send a button-state input event. Payload = full Cora input payload.
    SendInput(Vec<u8>),
    /// Disconnect this session.
    Disconnect,
}

pub struct Session {
    config: Arc<DeviceConfig>,
    cmd_rx: mpsc::Receiver<SessionCommand>,
}

impl Session {
    pub fn new(config: Arc<DeviceConfig>, cmd_rx: mpsc::Receiver<SessionCommand>) -> Self {
        Self { config, cmd_rx }
    }

    pub async fn run(mut self, stream: TcpStream) {
        let peer = stream.peer_addr().map(|a| a.to_string()).unwrap_or_default();
        info!("Client connected: {}", peer);

        let conn_no = GLOBAL_CONN_NO.fetch_add(1, Ordering::Relaxed);
        let mut framed = Framed::new(stream, CoraCodec);

        // --- Send initial keepalive ---
        if let Err(e) = framed.send(build_keepalive(conn_no, 0)).await {
            warn!("Failed to send initial keepalive to {}: {}", peer, e);
            return;
        }

        let keepalive_interval = Duration::from_millis(2000);
        let mut keepalive_timer = time::interval(keepalive_interval);
        keepalive_timer.tick().await; // consume the immediate first tick

        loop {
            tokio::select! {
                // Periodic keepalive
                _ = keepalive_timer.tick() => {
                    debug!("Sending keepalive to {}", peer);
                    if framed.send(build_keepalive(conn_no, 0)).await.is_err() {
                        break;
                    }
                }

                // Incoming message from client
                maybe_msg = framed.next() => {
                    match maybe_msg {
                        None => {
                            info!("Client disconnected: {}", peer);
                            break;
                        }
                        Some(Err(e)) => {
                            warn!("Read error from {}: {}", peer, e);
                            break;
                        }
                        Some(Ok(msg)) => {
                            if let Err(e) = self.handle_message(&mut framed, msg, &peer).await {
                                warn!("Error handling message from {}: {}", peer, e);
                                break;
                            }
                        }
                    }
                }

                // Command from CLI
                maybe_cmd = self.cmd_rx.recv() => {
                    match maybe_cmd {
                        None |                         Some(SessionCommand::Disconnect) => {
                            info!("Session {} disconnected by command", peer);
                            break;
                        }
                        Some(SessionCommand::SendInput(payload)) => {
                            debug!("Sending input event to {}", peer);
                            let msg = CoraMessage::new(
                                CoraFlags::NONE,
                                HidOp::Write,
                                0,
                                Bytes::from(payload),
                            );
                            if framed.send(msg).await.is_err() {
                                break;
                            }
                        }
                    }
                }
            }
        }

        info!("Session ended: {}", peer);
    }

    async fn handle_message(
        &self,
        framed: &mut Framed<TcpStream, CoraCodec>,
        msg: CoraMessage,
        peer: &str,
    ) -> Result<(), std::io::Error> {
        // Keepalive ACK from client – nothing to do
        if msg.flags.contains(CoraFlags::ACK_NAK) {
            debug!("ACK from {}", peer);
            return Ok(());
        }

        match msg.hid_op {
            HidOp::GetReport => {
                self.handle_get_report(framed, msg, peer).await?;
            }
            HidOp::SendReport => {
                // Client sends a feature report (e.g. setBrightness)
                self.handle_send_report(&msg, peer);
            }
            HidOp::Write => {
                // Image data or other writes
                debug!(
                    "WRITE from {} flags={:#06x} len={}",
                    peer,
                    msg.flags.0,
                    msg.payload.len()
                );
            }
        }
        Ok(())
    }

    async fn handle_get_report(
        &self,
        framed: &mut Framed<TcpStream, CoraCodec>,
        msg: CoraMessage,
        peer: &str,
    ) -> Result<(), std::io::Error> {
        // Primary port request: payload = [0x03, report_id]
        // Secondary port request (VERBATIM): payload = [report_id]
        let report_id = if msg.flags.contains(CoraFlags::VERBATIM) {
            msg.payload.first().copied()
        } else {
            msg.payload.get(1).copied()
        };

        let report_id = match report_id {
            Some(id) => id,
            None => {
                warn!("GET_REPORT with empty payload from {}", peer);
                return Ok(());
            }
        };

        debug!("GET_REPORT {:#04x} from {}", report_id, peer);

        match build_response(report_id, &self.config) {
            Some(payload) => {
                let response = CoraMessage::new(
                    CoraFlags::RESULT,
                    HidOp::GetReport,
                    msg.message_id,
                    Bytes::from(payload),
                );
                framed.send(response).await?;
            }
            None => {
                // Report 0x08 is for the secondary port – we are primary, ignore
                // (client will timeout on 0x08 and see 0x80 response first, setting isPrimary=true)
                debug!("No response for GET_REPORT {:#04x}", report_id);
            }
        }

        Ok(())
    }

    fn handle_send_report(&self, msg: &CoraMessage, peer: &str) {
        if msg.payload.len() >= 3 && msg.payload[0] == 0x03 && msg.payload[1] == 0x08 {
            let brightness = msg.payload[2];
            info!("SET_BRIGHTNESS {} from {}", brightness, peer);
        } else {
            debug!(
                "SEND_REPORT from {} len={} data={:02x?}",
                peer,
                msg.payload.len(),
                &msg.payload[..msg.payload.len().min(8)]
            );
        }
    }
}

/// Build a Cora keepalive message.
///
/// From socketWrapper.ts #handleCoraDataPacket (server-side / client-side mirror):
///   payload[0] == 0x01 && payload[1] == 0x0a  → keepalive
///   payload[5] = connection_no
/// We send at least 6 bytes.
pub fn build_keepalive(conn_no: u8, message_id: u32) -> CoraMessage {
    let mut payload = vec![0u8; 32];
    payload[0] = 0x01;
    payload[1] = 0x0a;
    payload[5] = conn_no;

    CoraMessage::new(CoraFlags::NONE, HidOp::Write, message_id, Bytes::from(payload))
}
