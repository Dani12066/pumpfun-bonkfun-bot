use std::time::Duration;

use anyhow::Result;
use tokio::sync::mpsc::UnboundedSender;

use super::TokenEvent;

pub async fn run(endpoint: String, _tx: UnboundedSender<TokenEvent>) -> Result<()> {
    log::info!("Starting LaserStream listener at {endpoint}");
    let mut backoff = Duration::from_millis(250);
    loop {
        match tonic::transport::Channel::from_shared(endpoint.clone())?
            .connect()
            .await
        {
            Ok(_channel) => {
                log::info!("Connected to LaserStream (placeholder parser not wired)");
                tokio::time::sleep(Duration::from_secs(5)).await;
            }
            Err(err) => {
                log::warn!("LaserStream connection failed: {err}");
                tokio::time::sleep(backoff).await;
                backoff = (backoff + Duration::from_millis(250)).min(Duration::from_secs(5));
            }
        }
    }
}
