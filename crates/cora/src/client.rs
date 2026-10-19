//! Cora TCP client.
//!
//! Reads and writes are separated: the client owns a single socket and never
//! holds a lock across a read. This is the fix for the `streamdeck-cora-rs`
//! deadlock noted in Issue #23.

use std::collections::VecDeque;
use std::time::Duration;

use anyhow::{anyhow, bail, Context, Result};
use productiondeck_core::cora::{
    write_header, CoraFlags, CoraParser, FeatureStyle, HidOp, CORA_HEADER,
};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::time::timeout;

use crate::frame::OwnedFrame;

const READ_BUF: usize = 4096;
const DEFAULT_TIMEOUT: Duration = Duration::from_secs(5);

/// Cora TCP client.
pub struct CoraClient {
    stream: TcpStream,
    parser: CoraParser<8192>,
    queue: VecDeque<OwnedFrame>,
    next_id: u32,
}

impl CoraClient {
    /// Connect to a Cora device.
    pub async fn connect(addr: impl tokio::net::ToSocketAddrs) -> Result<Self> {
        let stream = TcpStream::connect(addr)
            .await
            .context("connect to Cora device")?;
        stream.set_nodelay(true).ok();
        Ok(Self {
            stream,
            parser: CoraParser::new(),
            queue: VecDeque::new(),
            next_id: 1,
        })
    }

    /// Allocate a message id.
    pub fn next_message_id(&mut self) -> u32 {
        let id = self.next_id;
        self.next_id = self.next_id.wrapping_add(1);
        id
    }

    /// Send one frame.
    pub async fn send_frame(
        &mut self,
        flags: CoraFlags,
        hid_op: HidOp,
        message_id: u32,
        payload: &[u8],
    ) -> Result<()> {
        let mut out = vec![0u8; CORA_HEADER + payload.len()];
        write_header(&mut out, flags, hid_op, message_id, payload.len())
            .ok_or_else(|| anyhow!("frame too large"))?;
        out[CORA_HEADER..].copy_from_slice(payload);
        self.stream.write_all(&out).await?;
        Ok(())
    }

    /// Read one frame, waiting at most `dur`.
    pub async fn recv_frame(&mut self, dur: Duration) -> Result<Option<OwnedFrame>> {
        if let Some(frame) = self.queue.pop_front() {
            return Ok(Some(frame));
        }
        let mut buf = [0u8; READ_BUF];
        loop {
            let n = match timeout(dur, self.stream.read(&mut buf)).await {
                Ok(Ok(n)) => n,
                Ok(Err(e)) => return Err(e.into()),
                Err(_) => return Ok(None),
            };
            if n == 0 {
                bail!("connection closed");
            }
            self.parser
                .push(&buf[..n])
                .map_err(|e| anyhow!("Cora parse error: {e:?}"))?;
            let queue = &mut self.queue;
            self.parser
                .drain(|frame| {
                    queue.push_back(frame.into());
                    Ok(())
                })
                .map_err(|e| anyhow!("Cora frame error: {e:?}"))?;
            if let Some(frame) = self.queue.pop_front() {
                return Ok(Some(frame));
            }
        }
    }

    /// Acknowledge a keep-alive frame.
    pub async fn ack_keep_alive(&mut self, frame: &OwnedFrame) -> Result<()> {
        let conn = frame.connection_no().unwrap_or(0);
        let mut payload = [0u8; 32];
        payload[0] = 0x03;
        payload[1] = 0x1a;
        payload[2] = conn;
        self.send_frame(CoraFlags::ACK_NAK, frame.hid_op, frame.message_id, &payload)
            .await
    }

    /// Send a full HID feature report (`hid_send_feature_report`).
    pub async fn send_feature_report(&mut self, report: &[u8]) -> Result<()> {
        self.send_frame(CoraFlags::VERBATIM, HidOp::SendReport, 0, report)
            .await
    }

    /// Send a full HID output report (`hid_write`).
    pub async fn send_output_report(&mut self, report: &[u8]) -> Result<()> {
        self.send_frame(CoraFlags::VERBATIM, HidOp::Write, 0, report)
            .await
    }

    /// Send an output report requesting an ACK.
    pub async fn send_output_report_ack(&mut self, report: &[u8]) -> Result<()> {
        let id = self.next_message_id();
        self.send_frame(
            CoraFlags::VERBATIM | CoraFlags::REQ_ACK,
            HidOp::Write,
            id,
            report,
        )
        .await?;
        self.wait_ack(id, DEFAULT_TIMEOUT).await
    }

    /// Wait for an ACK matching `message_id`.
    pub async fn wait_ack(&mut self, message_id: u32, dur: Duration) -> Result<()> {
        let deadline = tokio::time::Instant::now() + dur;
        loop {
            let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
            if remaining.is_zero() {
                bail!("timed out waiting for ACK {message_id:#x}");
            }
            let Some(frame) = self.recv_frame(remaining).await? else {
                bail!("timed out waiting for ACK {message_id:#x}");
            };
            if frame.is_keep_alive() {
                self.ack_keep_alive(&frame).await?;
                continue;
            }
            if frame.flags.contains(CoraFlags::ACK_NAK) && frame.message_id == message_id {
                return Ok(());
            }
        }
    }

    /// Request a feature report using the node-style `GET_REPORT` convention.
    pub async fn get_feature(&mut self, report_id: u8, style: FeatureStyle) -> Result<Vec<u8>> {
        let id = self.next_message_id();
        let (flags, payload) = match style {
            FeatureStyle::CoraChild => (CoraFlags::VERBATIM, vec![report_id]),
            _ => (CoraFlags::empty(), vec![0x03, report_id]),
        };
        self.send_frame(flags, HidOp::GetReport, id, &payload)
            .await?;
        self.await_feature(report_id, DEFAULT_TIMEOUT).await
    }

    /// Request a feature report using the official-app `WRITE` convention.
    pub async fn get_feature_write_style(&mut self, report_id: u8) -> Result<Vec<u8>> {
        let id = self.next_message_id();
        let mut payload = vec![0u8; 1024];
        payload[0] = 0x03;
        payload[1] = report_id;
        self.send_frame(CoraFlags::empty(), HidOp::Write, id, &payload)
            .await?;
        self.await_feature(report_id, DEFAULT_TIMEOUT).await
    }

    async fn await_feature(&mut self, report_id: u8, dur: Duration) -> Result<Vec<u8>> {
        let deadline = tokio::time::Instant::now() + dur;
        loop {
            let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
            if remaining.is_zero() {
                bail!("timed out waiting for feature {report_id:#04x}");
            }
            let Some(frame) = self.recv_frame(remaining).await? else {
                bail!("timed out waiting for feature {report_id:#04x}");
            };
            if frame.is_keep_alive() {
                self.ack_keep_alive(&frame).await?;
                continue;
            }
            if frame.flags.contains(CoraFlags::ACK_NAK) {
                continue;
            }
            if frame.feature_report_id() == Some(report_id) {
                return Ok(frame.payload);
            }
        }
    }

    /// Send a brightness feature report.
    pub async fn set_brightness(&mut self, percent: u8) -> Result<()> {
        self.send_feature_report(&[0x03, 0x08, percent]).await
    }

    /// Send a reset feature report.
    pub async fn reset(&mut self) -> Result<()> {
        self.send_feature_report(&[0x03, 0x02]).await
    }
}
