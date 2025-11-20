use std::{str::FromStr, sync::Arc};

use anyhow::Result;
use solana_sdk::{
    compute_budget::ComputeBudgetInstruction,
    instruction::{AccountMeta, Instruction},
    message::Message,
    pubkey::Pubkey,
    signature::{Keypair, Signer},
    system_program,
    transaction::Transaction,
};

use crate::{config::Config, events::TokenEvent, state::BlockhashCache};

pub struct TransactionBuilder {
    config: Arc<Config>,
    payer: Arc<Keypair>,
    blockhash_cache: BlockhashCache,
    program_id: Pubkey,
}

impl TransactionBuilder {
    pub fn new(
        config: Arc<Config>,
        payer: Arc<Keypair>,
        blockhash_cache: BlockhashCache,
    ) -> Result<Self> {
        let program_id = config.program_id()?;
        Ok(Self {
            config,
            payer,
            blockhash_cache,
            program_id,
        })
    }

    pub fn build_buy_transaction(
        &self,
        event: &TokenEvent,
        lamports: u64,
    ) -> Result<Option<Transaction>> {
        let Some(blockhash) = self.blockhash_cache.latest() else {
            log::warn!("Blockhash cache empty, skipping transaction");
            return Ok(None);
        };

        let mut instructions = Vec::new();

        if let Some(priority_fee) = self.config.fee_config.priority_fee_lamports {
            instructions.push(ComputeBudgetInstruction::set_compute_unit_price(
                priority_fee,
            ));
        }

        instructions.push(self.create_associated_token_account(&event.mint)?);
        instructions.push(self.pump_fun_buy_instruction(event, lamports)?);

        let message = Message::new(&instructions, Some(&self.payer.pubkey()));
        let transaction = Transaction::new(&[self.payer.as_ref()], message, blockhash);
        Ok(Some(transaction))
    }

    fn pump_fun_buy_instruction(&self, event: &TokenEvent, lamports: u64) -> Result<Instruction> {
        let accounts = vec![
            AccountMeta::new(event.mint, false),
            AccountMeta::new(self.payer.pubkey(), true),
            AccountMeta::new_readonly(system_program::ID, false),
        ];

        let mut data = lamports.to_le_bytes().to_vec();
        data.extend_from_slice(&event.developer.to_bytes());

        Ok(Instruction {
            program_id: self.program_id,
            accounts,
            data,
        })
    }

    fn create_associated_token_account(&self, mint: &Pubkey) -> Result<Instruction> {
        const TOKEN_PROGRAM_ID: &str = "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA";
        const ASSOCIATED_TOKEN_PROGRAM_ID: &str = "ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL";

        let token_program = Pubkey::from_str(TOKEN_PROGRAM_ID)?;
        let ata_program = Pubkey::from_str(ASSOCIATED_TOKEN_PROGRAM_ID)?;
        let (ata, _) = Pubkey::find_program_address(
            &[
                self.payer.pubkey().as_ref(),
                token_program.as_ref(),
                mint.as_ref(),
            ],
            &ata_program,
        );

        Ok(Instruction {
            program_id: ata_program,
            accounts: vec![
                AccountMeta::new(self.payer.pubkey(), true),
                AccountMeta::new(ata, false),
                AccountMeta::new_readonly(self.payer.pubkey(), false),
                AccountMeta::new_readonly(*mint, false),
                AccountMeta::new_readonly(system_program::ID, false),
                AccountMeta::new_readonly(token_program, false),
            ],
            data: vec![],
        })
    }
}
