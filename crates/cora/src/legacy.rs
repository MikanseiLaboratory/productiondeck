//! Legacy (headerless 512/1024) Cora-side client.
//!
//! Real Studio firmware 2.01.002 speaks this framing on TCP 5343:
//! 512-byte keep-alive, 1024-byte ACK/query, 512-byte response.

use anyhow::{bail, Result};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

const KA: usize = 512;
const CMD: usize = 1024;

/// Blocking-style legacy Cora client.
pub struct LegacyClient {
    stream: TcpStream,
}

impl LegacyClient {
    /// Connect, wait for the initial keep-alive and acknowledge it.
    pub async fn connect(addr: impl tokio::net::ToSocketAddrs) -> Result<Self> {
        let mut stream = TcpStream::connect(addr).await?;
        stream.set_nodelay(true).ok();
        let mut keepalive = [0u8; KA];
        stream.read_exact(&mut keepalive).await?;
        if keepalive[0] != 0x01 || keepalive[1] != 0x0a {
            bail!("not a legacy keep-alive: {:02x?}", &keepalive[..8]);
        }
        let mut ack = [0u8; CMD];
        ack[0] = 0x03;
        ack[1] = 0x1a;
        stream.write_all(&ack).await?;
        Ok(Self { stream })
    }

    /// Send `03 <id>` and read the 512-byte response.
    pub async fn query(&mut self, id: u8) -> Result<Vec<u8>> {
        let mut query = [0u8; CMD];
        query[0] = 0x03;
        query[1] = id;
        self.stream.write_all(&query).await?;
        let mut response = [0u8; KA];
        self.stream.read_exact(&mut response).await?;
        Ok(response.to_vec())
    }
}
