use std::{sync::Arc, time::Duration};

use dashmap::{DashMap, DashSet};
use parking_lot::RwLock;
use solana_client::nonblocking::rpc_client::RpcClient;
use solana_sdk::{hash::Hash, pubkey::Pubkey};
use tokio::sync::watch;
use tokio::time::Instant;

use crate::config::Config;

#[derive(Clone, Debug)]
pub struct BlockhashCache {
    inner: Arc<RwLock<Option<Hash>>>,
    notifier: watch::Sender<Option<Hash>>,
}

impl BlockhashCache {
    pub fn new() -> Self {
        let (tx, _rx) = watch::channel(None);
        Self {
            inner: Arc::new(RwLock::new(None)),
            notifier: tx,
        }
    }

    pub fn latest(&self) -> Option<Hash> {
        self.inner.read().clone()
    }

    pub fn subscribe(&self) -> watch::Receiver<Option<Hash>> {
        self.notifier.subscribe()
    }

    pub fn update(&self, hash: Hash) {
        *self.inner.write() = Some(hash);
        let _ = self.notifier.send_replace(Some(hash));
    }

    pub fn spawn_updater(
        &self,
        rpc_client: Arc<RpcClient>,
        interval: Duration,
    ) -> tokio::task::JoinHandle<()> {
        let cache = self.clone();
        tokio::spawn(async move {
            loop {
                match rpc_client.get_latest_blockhash().await {
                    Ok(hash) => cache.update(hash),
                    Err(err) => log::warn!("Blockhash refresh failed: {err}"),
                }
                tokio::time::sleep(interval).await;
            }
        })
    }
}

#[derive(Clone, Debug, Default)]
pub struct DevRateLimiter {
    pub counts: DashMap<Pubkey, Vec<Instant>>,
}

impl DevRateLimiter {
    pub fn is_allowed(&self, developer: &Pubkey, limit: u32, window: Duration) -> bool {
        let mut entry = self.counts.entry(*developer).or_default();
        let now = Instant::now();
        entry.retain(|ts| now.duration_since(*ts) <= window);
        entry.push(now);
        entry.len() as u32 <= limit
    }
}

#[derive(Clone, Debug)]
pub struct BalanceCache {
    balance: Arc<RwLock<u64>>,
}

impl BalanceCache {
    pub fn new(initial: u64) -> Self {
        Self {
            balance: Arc::new(RwLock::new(initial)),
        }
    }

    pub fn current(&self) -> u64 {
        *self.balance.read()
    }

    pub fn set(&self, new_value: u64) {
        *self.balance.write() = new_value;
    }

    pub fn debit(&self, lamports: u64) {
        let mut balance = self.balance.write();
        if *balance >= lamports {
            *balance -= lamports;
        }
    }
}

#[derive(Clone, Debug)]
pub struct FilterState {
    pub whitelist: DashSet<Pubkey>,
    pub blacklist: DashSet<Pubkey>,
}

impl FilterState {
    pub fn new(config: &Config) -> anyhow::Result<Self> {
        let whitelist = config.whitelist()?.into_iter().collect();
        let blacklist = config.blacklist()?.into_iter().collect();
        Ok(Self {
            whitelist,
            blacklist,
        })
    }

    pub fn is_whitelisted(&self, developer: &Pubkey) -> bool {
        self.whitelist.is_empty() || self.whitelist.contains(developer)
    }

    pub fn is_blacklisted(&self, developer: &Pubkey) -> bool {
        self.blacklist.contains(developer)
    }
}

#[derive(Clone)]
pub struct SniperState {
    pub filters: FilterState,
    pub rate_limiter: DevRateLimiter,
    pub seen_mints: DashSet<Pubkey>,
    pub blockhash_cache: BlockhashCache,
    pub balance_cache: BalanceCache,
    pub rpc_client: Arc<RpcClient>,
}

impl SniperState {
    pub fn new(config: &Config, rpc_client: Arc<RpcClient>) -> anyhow::Result<Self> {
        Ok(Self {
            filters: FilterState::new(config)?,
            rate_limiter: DevRateLimiter::default(),
            seen_mints: DashSet::new(),
            blockhash_cache: BlockhashCache::new(),
            balance_cache: BalanceCache::new(0),
            rpc_client,
        })
    }

    pub async fn refresh_balance(&self, owner: &Pubkey) {
        match self.rpc_client.get_balance(owner).await {
            Ok(lamports) => self.balance_cache.set(lamports),
            Err(err) => log::warn!("Failed to refresh balance: {err}"),
        }
    }
}
