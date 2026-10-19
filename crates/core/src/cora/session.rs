//! Cora session state machine.
//!
//! The session is I/O-free: `feed` consumes received bytes, `poll_transmit`
//! produces the next frame to send, `push_input` queues device input, and
//! `tick` drives keep-alive / timeout handling.
//!
//! It answers both known request styles:
//! - `node-elgato-stream-deck`: `GET_REPORT`, flags `NONE`, payload `[0x03, id]`
//! - official app: `WRITE`, flags `NONE`, payload `[0x03, id, ...]`
//!
//! and verbatim (child-device) requests with flags `VERBATIM`, payload `[id]`.

use crate::cora::frame::{write_header, CoraFlags, CoraFrame, CoraParser, HidOp, CORA_HEADER};
use crate::cora::CoraError;
use crate::feature::Identity;
use crate::input::InputEvent;
use crate::model::ModelInfo;
use crate::protocol::{Gen2Handler, OutputSink};

pub use crate::feature::FeatureStyle;

/// Maximum queued control responses.
pub const MAX_PENDING_TX: usize = 8;
/// Maximum queued input frames.
pub const MAX_INPUT_QUEUE: usize = 8;
/// Maximum encoded input report size.
pub const MAX_INPUT_FRAME: usize = 64;
/// Keep-alive payload length used by the reference Network Dock capture.
pub const KEEPALIVE_PAYLOAD: usize = 512;
/// Keep-alive ACK payload length.
pub const KEEPALIVE_ACK_PAYLOAD: usize = 32;
/// Default keep-alive interval in milliseconds.
pub const DEFAULT_KEEPALIVE_MS: u32 = 2000;
/// Default receive timeout in milliseconds.
pub const DEFAULT_TIMEOUT_MS: u32 = 5000;

/// An encoded input report waiting to be wrapped in a Cora frame.
#[derive(Debug, Clone, Copy)]
struct InputFrame {
    data: [u8; MAX_INPUT_FRAME],
    len: u8,
}

/// A queued control response.
#[derive(Debug, Clone, Copy)]
pub enum PendingTx {
    /// 4 zero bytes acknowledging a `REQ_ACK` message.
    Ack {
        message_id: u32,
        hid_op: HidOp,
        verbatim: bool,
    },
    /// `03 1A <conn>` acknowledging a keep-alive.
    KeepAliveAck {
        message_id: u32,
        hid_op: HidOp,
        connection_no: u8,
    },
    /// Unsolicited `01 0A ...` keep-alive.
    KeepAlive { message_id: u32, connection_no: u8 },
    /// Feature GET response.
    Feature {
        message_id: u32,
        hid_op: HidOp,
        flags: CoraFlags,
        report_id: u8,
        style: FeatureStyle,
    },
}

#[derive(Debug)]
struct Ring<T: Copy, const N: usize> {
    items: [Option<T>; N],
    head: usize,
    len: usize,
}

impl<T: Copy, const N: usize> Ring<T, N> {
    fn new() -> Self {
        Self {
            items: [None; N],
            head: 0,
            len: 0,
        }
    }

    fn clear(&mut self) {
        self.items = [None; N];
        self.head = 0;
        self.len = 0;
    }

    fn push(&mut self, item: T) -> bool {
        if self.len == N {
            return false;
        }
        let idx = (self.head + self.len) % N;
        self.items[idx] = Some(item);
        self.len += 1;
        true
    }

    /// Push, dropping the oldest item when full.
    fn push_overwrite(&mut self, item: T) {
        if !self.push(item) {
            let _ = self.pop();
            let _ = self.push(item);
        }
    }

    fn pop(&mut self) -> Option<T> {
        if self.len == 0 {
            return None;
        }
        let item = self.items[self.head].take();
        self.head = (self.head + 1) % N;
        self.len -= 1;
        item
    }
}

/// Device-side Cora session.
#[derive(Debug)]
pub struct CoraSession<'a, const RX: usize, const IMG: usize> {
    parser: CoraParser<RX>,
    handler: Gen2Handler<IMG>,
    identity: Identity<'a>,
    tx: Ring<PendingTx, MAX_PENDING_TX>,
    inputs: Ring<InputFrame, MAX_INPUT_QUEUE>,
    next_message_id: u32,
    connection_no: u8,
    connected: bool,
    now_ms: u32,
    last_rx_ms: u32,
    last_keepalive_ms: u32,
    keepalive_interval_ms: u32,
    timeout_ms: u32,
}

impl<'a, const RX: usize, const IMG: usize> CoraSession<'a, RX, IMG> {
    /// Create a session for `model` with the given identity.
    pub fn new(model: &'static ModelInfo, identity: Identity<'a>) -> Self {
        Self {
            parser: CoraParser::new(),
            handler: Gen2Handler::new(model),
            identity,
            tx: Ring::new(),
            inputs: Ring::new(),
            next_message_id: 1,
            connection_no: 0,
            connected: false,
            now_ms: 0,
            last_rx_ms: 0,
            last_keepalive_ms: 0,
            keepalive_interval_ms: DEFAULT_KEEPALIVE_MS,
            timeout_ms: DEFAULT_TIMEOUT_MS,
        }
    }

    pub fn set_connection_no(&mut self, connection_no: u8) {
        self.connection_no = connection_no;
    }

    pub fn set_keepalive_interval_ms(&mut self, ms: u32) {
        self.keepalive_interval_ms = ms;
    }

    pub fn set_timeout_ms(&mut self, ms: u32) {
        self.timeout_ms = ms;
    }

    pub fn is_connected(&self) -> bool {
        self.connected
    }

    /// Mark the session connected and queue the initial keep-alive.
    pub fn start(&mut self, now_ms: u32) {
        self.connected = true;
        self.now_ms = now_ms;
        self.last_rx_ms = now_ms;
        self.queue_keepalive(now_ms);
    }

    /// Forget all connection state.
    pub fn on_disconnect(&mut self) {
        self.parser.clear();
        self.tx.clear();
        self.inputs.clear();
        self.connected = false;
    }

    /// True once no data has been received for the configured timeout.
    pub fn timed_out(&self, now_ms: u32) -> bool {
        self.connected && now_ms.wrapping_sub(self.last_rx_ms) > self.timeout_ms
    }

    /// Queue a device input event.
    pub fn push_input(&mut self, event: InputEvent) {
        let mut frame = InputFrame {
            data: [0u8; MAX_INPUT_FRAME],
            len: 0,
        };
        if let Ok(len) = self.handler.encode_input(event, &mut frame.data) {
            if len <= MAX_INPUT_FRAME {
                frame.len = len as u8;
                self.inputs.push_overwrite(frame);
            }
        }
    }

    /// Queue a raw, already-encoded input report.
    pub fn push_input_report(&mut self, report: &[u8]) {
        let len = report.len().min(MAX_INPUT_FRAME);
        let mut frame = InputFrame {
            data: [0u8; MAX_INPUT_FRAME],
            len: len as u8,
        };
        frame.data[..len].copy_from_slice(&report[..len]);
        self.inputs.push_overwrite(frame);
    }

    /// Feed received bytes and dispatch complete frames.
    pub fn feed(&mut self, bytes: &[u8], sink: &mut dyn OutputSink) -> Result<(), CoraError> {
        self.parser.push(bytes)?;

        let now = self.now_ms;
        let connection_no = self.connection_no;
        let handler = &mut self.handler;
        let tx = &mut self.tx;
        let last_rx = &mut self.last_rx_ms;
        let mut saw_data = false;

        self.parser.drain(|frame: CoraFrame<'_>| {
            saw_data = true;
            *last_rx = now;
            let payload = frame.payload;
            if payload.is_empty() {
                return Ok(());
            }

            // ACK/NAK from the client (including a keep-alive ACK whose payload
            // starts with `03 1a`). Never interpret it as a feature query.
            if frame.flags.contains(CoraFlags::ACK_NAK) {
                return Ok(());
            }

            // Keep-alive: 01 0A ...
            if payload.len() >= 2 && payload[0] == 0x01 && payload[1] == 0x0a {
                let conn = payload.get(5).copied().unwrap_or(connection_no);
                let _ = tx.push(PendingTx::KeepAliveAck {
                    message_id: frame.message_id,
                    hid_op: frame.hid_op,
                    connection_no: conn,
                });
                return Ok(());
            }

            match frame.hid_op {
                HidOp::SendReport => {
                    // Full HID feature report, including report id.
                    let _ = handler.handle_set_feature(payload, sink);
                }
                HidOp::GetReport => {
                    let (style, report_id) = if frame.flags.contains(CoraFlags::VERBATIM) {
                        (FeatureStyle::CoraChild, payload[0])
                    } else if payload[0] == 0x03 && payload.len() > 1 {
                        (FeatureStyle::CoraPrimary, payload[1])
                    } else {
                        (FeatureStyle::CoraPrimary, payload[0])
                    };
                    let flags = if frame.flags.contains(CoraFlags::VERBATIM) {
                        CoraFlags::RESULT | CoraFlags::VERBATIM
                    } else {
                        CoraFlags::RESULT
                    };
                    let _ = tx.push(PendingTx::Feature {
                        message_id: frame.message_id,
                        hid_op: frame.hid_op,
                        flags,
                        report_id,
                        style,
                    });
                }
                HidOp::Write => {
                    if payload[0] == 0x02 || frame.flags.contains(CoraFlags::VERBATIM) {
                        // HID output report.
                        let _ = handler.handle_output(payload, sink);
                        if frame.flags.contains(CoraFlags::REQ_ACK) {
                            let _ = tx.push(PendingTx::Ack {
                                message_id: frame.message_id,
                                hid_op: frame.hid_op,
                                verbatim: frame.flags.contains(CoraFlags::VERBATIM),
                            });
                        }
                    } else if payload[0] == 0x03 && payload.len() > 1 {
                        // Official-app style feature query over WRITE.
                        let _ = tx.push(PendingTx::Feature {
                            message_id: frame.message_id,
                            hid_op: frame.hid_op,
                            flags: CoraFlags::empty(),
                            report_id: payload[1],
                            style: FeatureStyle::CoraPrimary,
                        });
                    }
                }
            }
            Ok(())
        })?;

        if saw_data {
            self.connected = true;
        }
        Ok(())
    }

    /// Advance time, queueing keep-alives as needed.
    pub fn tick(&mut self, now_ms: u32) {
        self.now_ms = now_ms;
        if !self.connected {
            return;
        }
        if now_ms.wrapping_sub(self.last_keepalive_ms) >= self.keepalive_interval_ms {
            self.queue_keepalive(now_ms);
        }
    }

    fn queue_keepalive(&mut self, now_ms: u32) {
        let message_id = self.next_message_id;
        self.next_message_id = self.next_message_id.wrapping_add(1);
        self.last_keepalive_ms = now_ms;
        let _ = self.tx.push(PendingTx::KeepAlive {
            message_id,
            connection_no: self.connection_no,
        });
    }

    /// Produce the next frame to send, or `None` when idle.
    pub fn poll_transmit(&mut self, out: &mut [u8]) -> Option<usize> {
        // Input has priority: it is small and latency sensitive.
        if let Some(frame) = self.inputs.pop() {
            let payload = &frame.data[..frame.len as usize];
            let header = write_header(out, CoraFlags::empty(), HidOp::Write, 0, payload.len())?;
            out[header..header + payload.len()].copy_from_slice(payload);
            return Some(header + payload.len());
        }

        loop {
            let item = self.tx.pop()?;
            if let Some(n) = self.encode_tx(item, out) {
                return Some(n);
            }
        }
    }

    fn encode_tx(&mut self, item: PendingTx, out: &mut [u8]) -> Option<usize> {
        match item {
            PendingTx::Ack {
                message_id,
                hid_op,
                verbatim,
            } => {
                let flags = if verbatim {
                    CoraFlags::ACK_NAK | CoraFlags::VERBATIM
                } else {
                    CoraFlags::ACK_NAK
                };
                let header = write_header(out, flags, hid_op, message_id, 4)?;
                out[header..header + 4].fill(0);
                Some(header + 4)
            }
            PendingTx::KeepAliveAck {
                message_id,
                hid_op,
                connection_no,
            } => {
                let header = write_header(
                    out,
                    CoraFlags::ACK_NAK,
                    hid_op,
                    message_id,
                    KEEPALIVE_ACK_PAYLOAD,
                )?;
                let payload = &mut out[header..header + KEEPALIVE_ACK_PAYLOAD];
                payload.fill(0);
                payload[0] = 0x03;
                payload[1] = 0x1a;
                payload[2] = connection_no;
                Some(header + KEEPALIVE_ACK_PAYLOAD)
            }
            PendingTx::KeepAlive {
                message_id,
                connection_no,
            } => {
                let header = write_header(
                    out,
                    CoraFlags::empty(),
                    HidOp::Write,
                    message_id,
                    KEEPALIVE_PAYLOAD,
                )?;
                let payload = &mut out[header..header + KEEPALIVE_PAYLOAD];
                payload.fill(0);
                payload[0] = 0x01;
                payload[1] = 0x0a;
                payload[2] = 0x02;
                payload[3] = 0x00;
                payload[4] = 0x01;
                payload[5] = connection_no;
                Some(header + KEEPALIVE_PAYLOAD)
            }
            PendingTx::Feature {
                message_id,
                hid_op,
                flags,
                report_id,
                style,
            } => {
                let payload_buf = out.get_mut(CORA_HEADER..)?;
                let len = self
                    .handler
                    .get_feature(report_id, style, &self.identity, payload_buf)
                    .ok()?;
                let header = write_header(out, flags, hid_op, message_id, len)?;
                Some(header + len)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model;

    fn frame_bytes(
        flags: CoraFlags,
        hid_op: HidOp,
        id: u32,
        payload: &[u8],
    ) -> ([u8; 1200], usize) {
        let mut out = [0u8; 1200];
        let header = write_header(&mut out, flags, hid_op, id, payload.len()).unwrap();
        out[header..header + payload.len()].copy_from_slice(payload);
        (out, header + payload.len())
    }

    fn session() -> CoraSession<'static, 4096, 64> {
        static SERIAL: &str = "TESTSERIAL0001";
        let identity = Identity::new(&model::STUDIO, SERIAL);
        CoraSession::new(&model::STUDIO, identity)
    }

    #[test]
    fn keepalive_is_acked_without_eating_the_next_frame() {
        let mut s = session();
        s.start(0);
        let mut out = [0u8; 1200];
        assert!(s.poll_transmit(&mut out).is_some()); // initial keepalive

        let (ka, ka_len) = frame_bytes(CoraFlags::empty(), HidOp::Write, 0x11, &{
            let mut p = [0u8; 512];
            p[0] = 0x01;
            p[1] = 0x0a;
            p[5] = 3;
            p
        });
        let (feat, feat_len) =
            frame_bytes(CoraFlags::empty(), HidOp::GetReport, 0x22, &[0x03, 0x80]);
        let mut buf = [0u8; 1200];
        buf[..ka_len].copy_from_slice(&ka[..ka_len]);
        buf[ka_len..ka_len + feat_len].copy_from_slice(&feat[..feat_len]);
        let mut sink = crate::protocol::NullSink;
        s.feed(&buf[..ka_len + feat_len], &mut sink).unwrap();

        // First queued response is the keep-alive ACK ...
        let n = s.poll_transmit(&mut out).unwrap();
        assert_eq!(
            u16::from_le_bytes([out[4], out[5]]),
            CoraFlags::ACK_NAK.bits()
        );
        assert_eq!(out[6], HidOp::Write as u8);
        assert_eq!(u32::from_le_bytes([out[8], out[9], out[10], out[11]]), 0x11);
        assert_eq!(out[16], 0x03);
        assert_eq!(out[17], 0x1a);
        assert_eq!(out[18], 3);
        assert_eq!(n, CORA_HEADER + KEEPALIVE_ACK_PAYLOAD);
        // ... and the feature response survived.
        let n = s.poll_transmit(&mut out).unwrap();
        assert_eq!(u32::from_le_bytes([out[8], out[9], out[10], out[11]]), 0x22);
        assert_eq!(out[16], 0x03);
        assert_eq!(out[17], 0x80);
        assert!(n >= 32);
    }

    #[test]
    fn official_write_style_feature_query_is_answered_in_kind() {
        let mut s = session();
        s.start(0);
        let mut out = [0u8; 1200];
        let _ = s.poll_transmit(&mut out);
        let payload = [0x03u8, 0x83, 0x00];
        let (msg, len) = frame_bytes(CoraFlags::empty(), HidOp::Write, 9, &payload);
        let mut sink = crate::protocol::NullSink;
        s.feed(&msg[..len], &mut sink).unwrap();
        let n = s.poll_transmit(&mut out).unwrap();
        assert_eq!(out[6], HidOp::Write as u8);
        assert_eq!(u16::from_le_bytes([out[4], out[5]]), 0);
        assert_eq!(out[16], 0x03);
        assert_eq!(out[17], 0x83);
        assert!(n > 16);
    }

    #[test]
    fn child_verbatim_feature_query() {
        let mut s = session();
        s.start(0);
        let mut out = [0u8; 1200];
        let _ = s.poll_transmit(&mut out);
        let payload = [0x05u8];
        let (msg, len) = frame_bytes(CoraFlags::VERBATIM, HidOp::GetReport, 4, &payload);
        let mut sink = crate::protocol::NullSink;
        s.feed(&msg[..len], &mut sink).unwrap();
        let n = s.poll_transmit(&mut out).unwrap();
        assert_eq!(u16::from_le_bytes([out[4], out[5]]), 0x8100);
        assert_eq!(out[6], HidOp::GetReport as u8);
        assert_eq!(out[16], 0x05);
        assert!(n > 16);
    }

    #[test]
    fn image_write_with_req_ack_is_acked() {
        let mut s = session();
        s.start(0);
        let mut out = [0u8; 1200];
        let _ = s.poll_transmit(&mut out);

        let mut payload = [0u8; 1024];
        payload[0] = 0x02;
        payload[1] = 0x07;
        payload[2] = 0;
        payload[3] = 1;
        payload[4..6].copy_from_slice(&2u16.to_le_bytes());
        payload[8..10].copy_from_slice(b"AB");
        let (msg, len) = frame_bytes(
            CoraFlags::VERBATIM | CoraFlags::REQ_ACK,
            HidOp::Write,
            5,
            &payload,
        );
        let mut sink = crate::protocol::NullSink;
        s.feed(&msg[..len], &mut sink).unwrap();
        let n = s.poll_transmit(&mut out).unwrap();
        assert_eq!(u16::from_le_bytes([out[4], out[5]]), 0x8200);
        assert_eq!(u32::from_le_bytes([out[8], out[9], out[10], out[11]]), 5);
        assert_eq!(n, CORA_HEADER + 4);
    }

    #[test]
    fn input_is_emitted_as_a_cora_frame() {
        let mut s = session();
        s.start(0);
        let mut out = [0u8; 1200];
        let _ = s.poll_transmit(&mut out); // keepalive
        s.push_input(InputEvent::Button {
            index: 31,
            pressed: true,
        });
        let n = s.poll_transmit(&mut out).unwrap();
        assert_eq!(out[6], HidOp::Write as u8);
        assert_eq!(out[16], 0x01);
        assert_eq!(out[17], 0x00);
        assert_eq!(out[20 + 31], 1);
        assert_eq!(n, CORA_HEADER + 4 + 32);
    }

    #[test]
    fn keepalive_ack_is_not_misread_as_a_feature_query() {
        // Regression: an ACK for a keep-alive starts `03 1a`; before the
        // ACK_NAK early return it was answered as a 0x1A feature request.
        let mut s = session();
        s.start(0);
        let mut out = [0u8; 1200];
        let _ = s.poll_transmit(&mut out); // initial keepalive

        let (msg, len) = frame_bytes(CoraFlags::ACK_NAK, HidOp::Write, 0x77, &[0x03, 0x1a, 0x00]);
        let mut sink = crate::protocol::NullSink;
        s.feed(&msg[..len], &mut sink).unwrap();
        assert!(
            s.poll_transmit(&mut out).is_none(),
            "ACK_NAK must not generate a feature response"
        );
    }
}
