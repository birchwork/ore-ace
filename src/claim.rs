use std::str::FromStr;

use colored::*;
use ore_api::consts::MINT_ADDRESS;
use solana_program::pubkey::Pubkey;
use solana_sdk::signature::Signer;
use spl_token::amount_to_ui_amount;

use crate::{
    args::ClaimArgs,
    cu_limits::CU_LIMIT_CLAIM,
    send_and_confirm::ComputeBudget,
    utils::{amount_f64_to_u64, ask_confirm, get_proof_with_authority},
    Miner,
};

impl Miner {
    pub async fn claim(&self, args: ClaimArgs) {
        let signer = self.signer();
        let pubkey = signer.pubkey();
        let proof = get_proof_with_authority(&self.rpc_client, pubkey).await;
        let mut ixs = vec![];
        let beneficiary = match args.to {
            Some(to) => {
                // Create beneficiary token account, if needed
                let wallet = Pubkey::from_str(&to).expect("Failed to parse wallet address");
                let benefiary_tokens = spl_associated_token_account::get_associated_token_address(
                    &wallet,
                    &MINT_ADDRESS,
                );
                if self
                    .rpc_client
                    .get_token_account(&benefiary_tokens)
                    .await
                    .is_err()
                {
                    ixs.push(
                        spl_associated_token_account::instruction::create_associated_token_account(
                            &signer.pubkey(),
                            &wallet,
                            &ore_api::consts::MINT_ADDRESS,
                            &spl_token::id(),
                        ),
                    );
                }
                benefiary_tokens
            }
            None => self.initialize_ata().await,
        };

        // Parse amount to claim
        let amount = if let Some(amount) = args.amount {
            amount_f64_to_u64(amount)
        } else {
            proof.balance
        };

        // Confirm user wants to claim
        if !ask_confirm(
            format!(
                "\nYou are about to claim {}.\n\nAre you sure you want to continue? [Y/n]",
                format!(
                    "{} ORE",
                    amount_to_ui_amount(amount, ore_api::consts::TOKEN_DECIMALS)
                )
                .bold(),
            )
            .as_str(),
        ) {
            return;
        }

        // Send and confirm
        ixs.push(ore_api::instruction::claim(pubkey, beneficiary, amount));
        self.send_and_confirm(&ixs, ComputeBudget::Fixed(CU_LIMIT_CLAIM), false)
            .await
            .ok();
    }

    async fn initialize_ata(&self) -> Pubkey {
        // Initialize client.
        let signer = self.signer();
        let client = self.rpc_client.clone();

        // Build instructions.
        let token_account_pubkey = spl_associated_token_account::get_associated_token_address(
            &signer.pubkey(),
            &ore_api::consts::MINT_ADDRESS,
        );

        // Check if ata already exists
        if let Ok(Some(_ata)) = client.get_token_account(&token_account_pubkey).await {
            return token_account_pubkey;
        }
        // Sign and send transaction.
        let ix = spl_associated_token_account::instruction::create_associated_token_account(
            &signer.pubkey(),
            &signer.pubkey(),
            &ore_api::consts::MINT_ADDRESS,
            &spl_token::id(),
        );
        self.send_and_confirm(&[ix], ComputeBudget::Dynamic, false)
            .await
            .ok();

        // Return token account address
        token_account_pubkey
    }
}
