use std::sync::Arc;

use anyhow::{anyhow, Result};
use base64::{engine::general_purpose::STANDARD, Engine};
use futures::future::{select_ok, BoxFuture};
use reqwest::Client;
use serde::Deserialize;
use serde_json::json;
use solana_client::nonblocking::rpc_client::RpcClient;
use solana_client::rpc_config::RpcSendTransactionConfig;
use solana_sdk::{signature::Signature, transaction::Transaction};

use crate::config::Config;

pub async fn dispatch_transaction(
    transaction: &Transaction,
    config: &Config,
    rpc_client: Arc<RpcClient>,
    http_client: &Client,
) -> Result<Signature> {
    let serialized = bincode::serialize(transaction)?;
    let encoded = STANDARD.encode(serialized);

    let mut futures: Vec<BoxFuture<'static, Result<Signature>>> = Vec::new();
    futures.push(Box::pin(send_via_rpc(rpc_client, transaction.clone())));

    if let Some(url) = config.endpoints.jito_api_url.clone() {
        futures.push(Box::pin(send_via_jito(
            url,
            encoded.clone(),
            http_client.clone(),
        )));
    }

    if let Some(url) = config.endpoints.nozomi_rpc_url.clone() {
        futures.push(Box::pin(send_via_http(
            url,
            encoded.clone(),
            http_client.clone(),
        )));
    }

    match select_ok(futures).await {
        Ok((sig, _)) => Ok(sig),
        Err(err) => Err(err),
    }
}

async fn send_via_rpc(rpc_client: Arc<RpcClient>, transaction: Transaction) -> Result<Signature> {
    let signature = rpc_client
        .send_transaction_with_config(
            &transaction,
            RpcSendTransactionConfig {
                skip_preflight: true,
                ..RpcSendTransactionConfig::default()
            },
        )
        .await?;
    Ok(signature)
}

#[derive(Debug, Deserialize)]
struct RpcResponse {
    result: Option<String>,
}

async fn send_via_jito(url: String, encoded: String, client: Client) -> Result<Signature> {
    // Jito's sendBundle API expects an array of base64-encoded transactions
    let payload = json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "sendBundle",
        "params": [[encoded]],
    });

    let resp = client
        .post(url)
        .json(&payload)
        .send()
        .await
        .map_err(|err| anyhow!("Jito HTTP send failed: {err}"))?;
    let status = resp.status();
    let body: serde_json::Value = resp.json().await?;

    // Jito returns bundle IDs, not transaction signatures
    // Extract the first bundle ID from the result array
    if let Some(result) = body.get("result") {
        if let Some(bundle_id) = result.as_array().and_then(|arr| arr.first()).and_then(|v| v.as_str()) {
            // Parse bundle ID as signature (they're both base58 strings)
            let signature: Signature = bundle_id.parse()?;
            return Ok(signature);
        }
    }

    Err(anyhow!("Jito HTTP send failed with status {status}: {body}"))
}

async fn send_via_http(url: String, encoded: String, client: Client) -> Result<Signature> {
    let payload = json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "sendTransaction",
        "params": [encoded, {"skipPreflight": true, "encoding": "base64"}],
    });

    let resp = client
        .post(url)
        .json(&payload)
        .send()
        .await
        .map_err(|err| anyhow!("HTTP send failed: {err}"))?;
    let status = resp.status();
    let body: serde_json::Value = resp.json().await?;

    if let Some(result) = body.get("result").and_then(|v| v.as_str()) {
        let signature: Signature = result.parse()?;
        return Ok(signature);
    }

    Err(anyhow!("HTTP send failed with status {status}: {body}"))
}
