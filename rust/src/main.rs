mod config;
mod events;
mod filters;
mod state;
mod transactions;

use std::sync::Arc;

use anyhow::Result;
use config::Config;
use events::{EventSupervisor, TokenEvent};
use filters::{apply_filters, FilterDecision};
use reqwest::Client;
use solana_client::nonblocking::rpc_client::RpcClient;
use solana_sdk::signature::Signer;
use transactions::{dispatch_transaction, TransactionBuilder};

#[tokio::main]
async fn main() -> Result<()> {
    env_logger::init();
    let config_path =
        std::env::var("SNIPER_CONFIG").unwrap_or_else(|_| "rust/config.example.toml".to_string());
    let config = Arc::new(Config::from_file(config_path)?);

    let payer = Arc::new(config.load_keypair()?);
    let rpc_client = Arc::new(RpcClient::new(config.endpoints.rpc_http_url.clone()));
    let state = state::SniperState::new(&config, rpc_client.clone())?;

    let blockhash_interval = config.blockhash_refresh_interval();
    let _blockhash_task = state
        .blockhash_cache
        .spawn_updater(rpc_client.clone(), blockhash_interval);

    let balance_state = state.clone();
    let owner = payer.pubkey();
    let balance_interval = config.balance_refresh_interval();
    tokio::spawn(async move {
        loop {
            balance_state.refresh_balance(&owner).await;
            tokio::time::sleep(balance_interval).await;
        }
    });

    let event_supervisor = EventSupervisor::new(config.clone());
    let mut receiver = event_supervisor.start();
    let builder =
        TransactionBuilder::new(config.clone(), payer.clone(), state.blockhash_cache.clone())?;
    let http_client = Client::new();

    log::info!("Sniper bot initialized; waiting for events");

    while let Some(event) = receiver.recv().await {
        handle_event(
            &config,
            &state,
            &builder,
            &http_client,
            rpc_client.clone(),
            &event,
        )
        .await?;
    }

    Ok(())
}

async fn handle_event(
    config: &Config,
    state: &state::SniperState,
    builder: &TransactionBuilder,
    http_client: &Client,
    rpc_client: Arc<RpcClient>,
    event: &TokenEvent,
) -> Result<()> {
    match apply_filters(event, config, state) {
        FilterDecision::Allowed => {
            log::info!(
                "Event passed filters from {:?}: {}",
                event.source,
                event.mint
            );
            state.seen_mints.insert(event.mint);
        }
        FilterDecision::Blacklisted => {
            log::warn!("Developer {} is blacklisted", event.developer);
            return Ok(());
        }
        FilterDecision::NotWhitelisted => {
            log::info!("Developer {} not whitelisted", event.developer);
            return Ok(());
        }
        FilterDecision::RateLimited => {
            log::info!("Developer {} rate limited", event.developer);
            return Ok(());
        }
        FilterDecision::Duplicate => return Ok(()),
    }

    let spend_lamports = config.compute_buy_amount(state.balance_cache.current())?;
    if let Some(transaction) = builder.build_buy_transaction(event, spend_lamports)? {
        if config.dry_run() {
            log::info!(
                "DRY_RUN: Built buy transaction for mint {} spending {} lamports",
                event.mint,
                spend_lamports
            );
            return Ok(());
        }

        match dispatch_transaction(&transaction, config, rpc_client, http_client).await {
            Ok(signature) => {
                state.balance_cache.debit(spend_lamports);
                log::info!("Submitted transaction {signature} for mint {}", event.mint);
            }
            Err(err) => log::error!("Failed to dispatch transaction: {err}"),
        }
    }

    Ok(())
}
