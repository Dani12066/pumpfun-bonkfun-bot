use std::{fs, path::Path, str::FromStr, time::Duration};

use anyhow::{anyhow, Context, Result};
use serde::Deserialize;
use solana_sdk::{
    native_token::sol_to_lamports,
    pubkey::Pubkey,
    signature::{read_keypair_file, Keypair},
};

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PurchaseStrategy {
    FixedSol(f64),
    PercentBalance(f64),
}

#[derive(Clone, Debug, Deserialize)]
pub struct FeeConfig {
    pub priority_fee_lamports: Option<u64>,
    pub use_jito_tip: Option<bool>,
    pub jito_tip_lamports: Option<u64>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct ProfitGuardConfig {
    pub take_profit_factor: Option<f64>,
    pub stop_loss_factor: Option<f64>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct DevFilterConfig {
    pub dev_whitelist: Option<Vec<String>>,
    pub dev_blacklist: Option<Vec<String>>,
    pub dev_max_tokens_per_min: Option<u32>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct EndpointsConfig {
    pub rpc_http_url: String,
    pub ws_url: Option<String>,
    pub laserstream_grpc_url: Option<String>,
    pub jito_api_url: Option<String>,
    pub nozomi_rpc_url: Option<String>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct Config {
    pub endpoints: EndpointsConfig,
    pub keypair_path: String,
    pub pump_fun_program: Option<String>,
    pub purchase_strategy: PurchaseStrategy,
    pub max_slippage_bps: Option<u64>,
    pub fee_config: FeeConfig,
    pub profit_guard: Option<ProfitGuardConfig>,
    pub dev_filters: DevFilterConfig,
    pub dry_run: Option<bool>,
    pub log_level: Option<String>,
    pub blockhash_refresh_ms: Option<u64>,
    pub balance_refresh_ms: Option<u64>,
}

impl Config {
    pub fn from_file(path: impl AsRef<Path>) -> Result<Self> {
        let raw = fs::read_to_string(&path).with_context(|| {
            format!("Failed to read config file at {}", path.as_ref().display())
        })?;
        let config: Config = toml::from_str(&raw).with_context(|| {
            format!("Failed to parse config file at {}", path.as_ref().display())
        })?;
        Ok(config)
    }

    pub fn load_keypair(&self) -> Result<Keypair> {
        read_keypair_file(&self.keypair_path)
            .map_err(|err| anyhow!("Failed to read keypair {}: {err}", self.keypair_path))
    }

    pub fn whitelist(&self) -> Result<Vec<Pubkey>> {
        parse_pubkeys(self.dev_filters.dev_whitelist.clone())
    }

    pub fn blacklist(&self) -> Result<Vec<Pubkey>> {
        parse_pubkeys(self.dev_filters.dev_blacklist.clone())
    }

    pub fn blockhash_refresh_interval(&self) -> Duration {
        Duration::from_millis(self.blockhash_refresh_ms.unwrap_or(350))
    }

    pub fn balance_refresh_interval(&self) -> Duration {
        Duration::from_millis(self.balance_refresh_ms.unwrap_or(1500))
    }

    pub fn dry_run(&self) -> bool {
        self.dry_run.unwrap_or(false)
    }

    pub fn compute_buy_amount(&self, cached_balance: u64) -> Result<u64> {
        match self.purchase_strategy {
            PurchaseStrategy::FixedSol(amount) => {
                if amount <= 0.0 {
                    return Err(anyhow!("Fixed SOL amount must be positive"));
                }
                Ok(sol_to_lamports(amount))
            }
            PurchaseStrategy::PercentBalance(fraction) => {
                if !(0.0..=1.0).contains(&fraction) {
                    return Err(anyhow!("purchase_percentage must be between 0 and 1"));
                }
                Ok((cached_balance as f64 * fraction) as u64)
            }
        }
    }

    pub fn program_id(&self) -> Result<Pubkey> {
        let id = self
            .pump_fun_program
            .clone()
            .unwrap_or_else(|| "5eykt4UsFv8P8NJdTREpY1vzqKqZKvdp".to_string());
        Pubkey::from_str(&id).map_err(|err| anyhow!("Invalid pump.fun program id: {err}"))
    }
}

fn parse_pubkeys(values: Option<Vec<String>>) -> Result<Vec<Pubkey>> {
    let Some(list) = values else {
        return Ok(vec![]);
    };
    list.into_iter()
        .map(|value| {
            Pubkey::from_str(&value).map_err(|err| anyhow!("Invalid pubkey {}: {err}", value))
        })
        .collect()
}
