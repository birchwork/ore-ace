use colored::*;
use solana_sdk::signature::Signer;
use spl_token::amount_to_ui_amount;

use crate::{
    args::ClaimArgs,
    send_and_confirm::ComputeBudget,
    utils::{ask_confirm, get_proof_with_authority},
    Miner,
};

impl Miner {
    pub async fn close(&self) {
        // Confirm proof exists
        let signer = self.signer();
        let proof = get_proof_with_authority(&self.rpc_client, signer.pubkey()).await;

        // Confirm the user wants to close.
        if !ask_confirm(
            format!("{} You have {} ORE staked in this account.\nAre you sure you want to {}close this account? [Y/n]", 
                "WARNING".yellow(),
                amount_to_ui_amount(proof.balance, ore_api::consts::TOKEN_DECIMALS),
                if proof.balance.gt(&0) { "claim your stake and "} else { "" }
            ).as_str()
        ) {
            return;
        }

        // Claim stake
        if proof.balance.gt(&0) {
            self.claim(ClaimArgs {
                amount: None,
                to: None,
            })
            .await;
        }

        // Submit close transaction
        let ix = ore_api::instruction::close(signer.pubkey());
        self.send_and_confirm(&[ix], ComputeBudget::Dynamic, false)
            .await
            .ok();
    }
}
