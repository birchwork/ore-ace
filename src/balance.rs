use std::str::FromStr;

use solana_program::pubkey::Pubkey;
use solana_sdk::signature::Signer;

use crate::{
    args::BalanceArgs,
    utils::{amount_u64_to_string, get_proof_with_authority},
    Miner,
};

impl Miner {
    pub async fn balance(&self, args: BalanceArgs) {
        let signer = self.signer();
        let address = if let Some(address) = args.address {
            if let Ok(address) = Pubkey::from_str(&address) {
                address
            } else {
                println!("Invalid address: {:?}", address);
                return;
            }
        } else {
            signer.pubkey()
        };
        let proof = get_proof_with_authority(&self.rpc_client, address).await;
        let token_account_address = spl_associated_token_account::get_associated_token_address(
            &address,
            &ore_api::consts::MINT_ADDRESS,
        );
        let token_balance = if let Ok(Some(token_account)) = self
            .rpc_client
            .get_token_account(&token_account_address)
            .await
        {
            token_account.token_amount.ui_amount_string
        } else {
            "0".to_string()
        };
        println!(
            "Balance: {} ORE\nStake: {} ORE",
            token_balance,
            amount_u64_to_string(proof.balance)
        )
    }
}
