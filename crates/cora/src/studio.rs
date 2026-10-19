//! High-level Studio device facade for integrationdeck.
//!
//! Tries the legacy 512/1024 framing first (real Studio fw 2.01.002), then
//! falls back to Cora. The caller gets a small brightness/feature API without
//! caring which framing the device uses.

use anyhow::{Context, Result};

use crate::client::CoraClient;
use crate::legacy::LegacyClient;
use crate::FeatureStyle;

/// Detected framing.
pub enum Transport {
    Legacy(LegacyClient),
    Cora(CoraClient),
}

/// Connected Studio.
pub struct StudioDevice {
    transport: Transport,
}

impl StudioDevice {
    /// Connect to `addr`, auto-detecting legacy vs Cora.
    pub async fn connect(addr: impl tokio::net::ToSocketAddrs + Clone) -> Result<Self> {
        if let Ok(legacy) = LegacyClient::connect(addr.clone()).await {
            return Ok(Self {
                transport: Transport::Legacy(legacy),
            });
        }
        let mut cora = CoraClient::connect(addr).await.context("connect Cora")?;
        cora.handshake().await.context("Cora handshake")?;
        Ok(Self {
            transport: Transport::Cora(cora),
        })
    }

    /// Set backlight brightness (0..100).
    pub async fn set_brightness(&mut self, value: u8) -> Result<()> {
        match &mut self.transport {
            Transport::Legacy(client) => client.set_brightness(value).await,
            Transport::Cora(client) => client.set_brightness(value).await,
        }
    }

    /// Query a feature report and return the raw payload.
    pub async fn feature(&mut self, id: u8) -> Result<Vec<u8>> {
        match &mut self.transport {
            Transport::Legacy(client) => client.query(id).await,
            Transport::Cora(client) => client.get_feature(id, FeatureStyle::CoraPrimary).await,
        }
    }

    /// True when the device uses legacy framing.
    pub fn is_legacy(&self) -> bool {
        matches!(self.transport, Transport::Legacy(_))
    }
}
