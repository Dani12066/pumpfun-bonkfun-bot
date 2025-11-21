pub mod laserstream;
pub mod websocket;

use std::sync::Arc;

use solana_sdk::pubkey::Pubkey;
use tokio::sync::mpsc::UnboundedReceiver;

use crate::config::Config;

#[derive(Clone, Debug)]
pub enum EventSourceKind {
    LaserStream,
    WebSocket,
}

#[derive(Clone, Debug)]
pub struct TokenEvent {
    pub mint: Pubkey,
    pub developer: Pubkey,
    pub source: EventSourceKind,
}

#[derive(Clone)]
pub struct EventSupervisor {
    config: Arc<Config>,
}

impl EventSupervisor {
    pub fn new(config: Arc<Config>) -> Self {
        Self { config }
    }

    pub fn start(&self) -> UnboundedReceiver<TokenEvent> {
        let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
        let laserstream = self.config.endpoints.laserstream_grpc_url.clone();
        let ws = self.config.endpoints.ws_url.clone();
        let config = self.config.clone();

        if let Some(endpoint) = laserstream {
            let tx_clone = tx.clone();
            tokio::spawn(async move {
                if let Err(err) = laserstream::run(endpoint, tx_clone).await {
                    log::warn!("LaserStream listener exited: {err}");
                }
            });
        }

        if let Some(ws_endpoint) = ws {
            tokio::spawn(async move {
                if let Err(err) = websocket::run(ws_endpoint, config, tx).await {
                    log::warn!("WebSocket listener exited: {err}");
                }
            });
        }

        rx
    }
}
