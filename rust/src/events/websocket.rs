use std::sync::Arc;
use std::time::Duration;

use anyhow::Result;
use futures::{SinkExt, StreamExt};
codex/convert-pump.fun-sniper-bot-to-rust
use serde_json::{json, Value};
=======
use serde_json::Value;
main
use solana_sdk::pubkey::Pubkey;
use std::str::FromStr;
use tokio::sync::mpsc::UnboundedSender;
use tokio_tungstenite::connect_async;
use tokio_tungstenite::tungstenite::Message;

use super::{EventSourceKind, TokenEvent};
use crate::config::Config;

pub async fn run(
    ws_endpoint: String,
codex/convert-pump.fun-sniper-bot-to-rust
    config: Arc<Config>,
=======
    _config: Arc<Config>,
main
    tx: UnboundedSender<TokenEvent>,
) -> Result<()> {
    log::info!("Starting websocket listener at {ws_endpoint}");
    let mut backoff = Duration::from_millis(500);
 codex/convert-pump.fun-sniper-bot-to-rust
    let program_id = config.program_id()?.to_string();
=======
 main

    loop {
        match connect_async(&ws_endpoint).await {
            Ok((mut socket, _)) => {
                log::info!("WebSocket connected");
 codex/convert-pump.fun-sniper-bot-to-rust
                let subscribe_message = json!({
                    "jsonrpc": "2.0",
                    "id": 1,
                    "method": "logsSubscribe",
                    "params": [
                        { "mentions": [program_id.clone()] },
                        { "commitment": "processed" }
                    ]
                })
                .to_string();

                let _ = socket
                    .send(Message::text(subscribe_message))
=======
                let _ = socket
                    .send(Message::text("{}"))
 main
                    .await
                    .map_err(|err| log::warn!("Failed to send subscribe message: {err}"));

                while let Some(message) = socket.next().await {
                    match message {
                        Ok(Message::Text(text)) => {
                            if let Some(event) = parse_event(&text) {
                                if tx.send(event).is_err() {
                                    log::warn!("Receiver dropped, closing websocket listener");
                                    return Ok(());
                                }
                            }
                        }
                        Ok(Message::Binary(_)) => {}
                        Ok(Message::Pong(_)) | Ok(Message::Frame(_)) => {}
                        Ok(Message::Ping(data)) => {
                            let _ = socket.send(Message::Pong(data)).await;
                        }
                        Ok(Message::Close(frame)) => {
                            log::warn!("WebSocket closed: {frame:?}");
                            break;
                        }
                        Err(err) => {
                            log::warn!("WebSocket error: {err}");
                            break;
                        }
                    }
                }
            }
            Err(err) => log::warn!("WebSocket connection failed: {err}"),
        }

        tokio::time::sleep(backoff).await;
        backoff = (backoff + Duration::from_millis(500)).min(Duration::from_secs(5));
    }
}

fn parse_event(raw: &str) -> Option<TokenEvent> {
    let json: Value = serde_json::from_str(raw).ok()?;
    let params = json.get("params")?.get("result")?.get("value")?;
    let dev_str = params.get("developer")?.as_str()?;
    let mint_str = params.get("mint")?.as_str()?;

    let developer = Pubkey::from_str(dev_str).ok()?;
    let mint = Pubkey::from_str(mint_str).ok()?;

    Some(TokenEvent {
        mint,
        developer,
        source: EventSourceKind::WebSocket,
    })
}
