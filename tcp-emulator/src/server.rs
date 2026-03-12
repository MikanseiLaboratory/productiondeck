/// TCP server – accepts connections and spawns sessions.
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::net::TcpListener;
use tokio::sync::{mpsc, Mutex};
use tracing::info;

use crate::feature_reports::DeviceConfig;
use crate::session::{Session, SessionCommand};

pub type SessionHandle = mpsc::Sender<SessionCommand>;

/// Shared state: map from peer address to session command sender.
#[derive(Default)]
pub struct ServerState {
    pub sessions: HashMap<SocketAddr, SessionHandle>,
}

pub async fn run_server(
    port: u16,
    config: Arc<DeviceConfig>,
    state: Arc<Mutex<ServerState>>,
) -> anyhow::Result<()> {
    let addr = format!("0.0.0.0:{}", port);
    let listener = TcpListener::bind(&addr).await?;
    info!("TCP server listening on {}", addr);

    loop {
        let (stream, peer_addr) = listener.accept().await?;
        let config = config.clone();
        let state = state.clone();

        let (cmd_tx, cmd_rx) = mpsc::channel::<SessionCommand>(32);

        {
            let mut s = state.lock().await;
            s.sessions.insert(peer_addr, cmd_tx);
        }

        let state_clone = state.clone();
        tokio::spawn(async move {
            let session = Session::new(config, cmd_rx);
            session.run(stream).await;

            // Remove session after it ends
            let mut s = state_clone.lock().await;
            s.sessions.remove(&peer_addr);
            info!("Session cleaned up for {}", peer_addr);
        });
    }
}
