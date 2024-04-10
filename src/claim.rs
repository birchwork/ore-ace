use std::str::FromStr;

use ore::{self, state::Proof, utils::AccountDeserialize};
use rand::Rng;
use solana_program::pubkey::Pubkey;
use solana_sdk::{compute_budget::ComputeBudgetInstruction, signature::Signer};

use crate::{cu_limits::CU_LIMIT_CLAIM, utils::proof_pubkey, Miner};

use serde::{Deserialize, Serialize};
#[derive(Serialize, Deserialize, Debug)]
struct GasTrackerResponse {
    sol: SolData,
}

#[derive(Serialize, Deserialize, Debug)]
struct SolData {
    per_transaction: PerTransaction,
}

#[derive(Serialize, Deserialize, Debug)]
struct PerTransaction {
    percentiles: Percentiles,
}

#[derive(Serialize, Deserialize, Debug)]
struct Percentiles {
    #[serde(rename = "25")]
    p25: u64,
    #[serde(rename = "50")]
    p50: u64,
    #[serde(rename = "75")]
    p75: u64,
}

impl Miner {
    pub async fn claim(&self, beneficiary: Option<String>, amount: Option<f64>) {
        let signer = self.signer();
        let pubkey = signer.pubkey();
        let client = self.rpc_client.clone();
        let beneficiary = match beneficiary {
            Some(beneficiary) => {
                Pubkey::from_str(&beneficiary).expect("Failed to parse beneficiary address")
            }
            None => self.initialize_ata().await,
        };
        let amount = if let Some(amount) = amount {
            amount as u64
        } else {
            match client.get_account(&proof_pubkey(pubkey)).await {
                Ok(proof_account) => {
                    let proof = Proof::try_from_bytes(&proof_account.data).unwrap();
                    proof.claimable_rewards
                }
                Err(err) => {
                    println!("Error looking up claimable rewards: {:?}", err);
                    return;
                }
            }
        };
        let amountf = (amount as f64) / (10f64.powf(ore::TOKEN_DECIMALS as f64));
        let cu_limit_ix = ComputeBudgetInstruction::set_compute_unit_limit(CU_LIMIT_CLAIM);
        let url = "https://quicknode.com/_gas-tracker?slug=solana";
        let client = reqwest::Client::new();
        let resp = match client
            .get(url)
            .header("Accept", "application/json")
            .send()
            .await
        {
            Ok(response) => response.json::<GasTrackerResponse>().await.ok(),
            Err(_) => None,
        };
        let p25 = resp.as_ref().unwrap().sol.per_transaction.percentiles.p25;
        let p50 = resp.as_ref().unwrap().sol.per_transaction.percentiles.p50;
        let p75 = resp.unwrap().sol.per_transaction.percentiles.p75;
        // Perform the calculation as peryour request
        let fuckmesilly = rand::thread_rng().gen_range(((p50 / 2) + p50)..(p25 / 5 + p75));
        let cu_price_ix = ComputeBudgetInstruction::set_compute_unit_price(fuckmesilly);
        let ix = ore::instruction::claim(pubkey, beneficiary, amount);
        println!("Submitting claim transaction...");
        match self
            .send_and_confirm(&[cu_limit_ix, cu_price_ix, ix], false, false, 0)
            .await
        {
            Ok(sig) => {
                println!("Claimed {:} ORE to account {:}", amountf, beneficiary);
                println!("{:?}", sig);
            }
            Err(err) => {
                println!("Error: {:?}", err);
            }
        }
    }

    async fn initialize_ata(&self) -> Pubkey {
        // Initialize client.
        let signer = self.signer();
        let client = self.rpc_client.clone();

        // Build instructions.
        let token_account_pubkey = spl_associated_token_account::get_associated_token_address(
            &signer.pubkey(),
            &ore::MINT_ADDRESS,
        );

        // Check if ata already exists
        if let Ok(Some(_ata)) = client.get_token_account(&token_account_pubkey).await {
            return token_account_pubkey;
        }

        // Sign and send transaction.
        let ix = spl_associated_token_account::instruction::create_associated_token_account(
            &signer.pubkey(),
            &signer.pubkey(),
            &ore::MINT_ADDRESS,
            &spl_token::id(),
        );
        match self.send_and_confirm(&[ix], true, false, 0).await {
            Ok(_sig) => println!("Created token account {:?}", token_account_pubkey),
            Err(e) => println!("Transaction failed: {:?}", e),
        }

        // Return token account address
        token_account_pubkey
    }
}
