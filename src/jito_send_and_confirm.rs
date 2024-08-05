use crate::send_and_confirm::ComputeBudget;

use crate::Miner;
use colored::Colorize;
use rand::Rng;
use serde::{de, Deserialize};
use serde_json::{json, Value};
use solana_program::pubkey::Pubkey;
use solana_program::{
    instruction::Instruction,
    native_token::{lamports_to_sol, sol_to_lamports},
};

use solana_sdk::{
    compute_budget::ComputeBudgetInstruction,
    pubkey,
    signature::{Signature, Signer},
    transaction::Transaction,
};
use solana_transaction_status::{Encodable, EncodedTransaction, UiTransactionEncoding};

pub const JITO_RECIPIENTS: [Pubkey; 8] = [
    pubkey!("96gYZGLnJYVFmbjzopPSU6QiEV5fGqZNyN9nmNhvrZU5"),
    pubkey!("HFqU5x63VTqvQss8hp11i4wVV8bD44PvwucfZ2bU7gRe"),
    pubkey!("Cw8CFyM9FkoMi7K7Crf6HNQqf4uEMzpKw6QNghXLvLkY"),
    pubkey!("ADaUMid9yfUytqMBgopwjb2DTLSokTSzL1zt6iGPaS49"),
    pubkey!("DfXygSm4jCyNCybVYYK6DwvWqjKee8pbDmJGcLWNDXjh"),
    pubkey!("ADuUkR4vqLUMWXxW9gh6D6L8pMSawimctcNZ5pGwDcEt"),
    pubkey!("DttWaMuVvTiduZRnguLF7jNxTgiMBZ1hyAumKUiL2KRL"),
    pubkey!("3AVi9Tg9Uo68tJfuvoKvqKNWKkC5wPdSSdeBnizKZ6jT"),
    //devnet
    // pubkey!("4xgEmT58RwTNsF5xm2RMYCnR1EVukdK8a1i2qFjnJFu3"),
    // pubkey!("EoW3SUQap7ZeynXQ2QJ847aerhxbPVr843uMeTfc9dxM"),
    // pubkey!("9n3d1K5YD2vECAbRFhFFGYNNjiXtHXJWn9F31t89vsAV"),
    // pubkey!("B1mrQSpdeMU9gCvkJ6VsXVVoYjRGkNA7TtjMyqxrhecH"),
    // pubkey!("ARTtviJkLLt6cHGQDydfo1Wyk6M4VGZdKZ2ZhdnJL336"),
    // pubkey!("9ttgPBBhRYFuQccdR1DSnb7hydsWANoDsV3P9kaGMCEh"),
    // pubkey!("E2eSqe33tuhAHKTrwky5uEjaVqnb2T9ns6nHHUrN8588"),
    // pubkey!("aTtUk2DHgLhKZRDjePq6eiHRKC1XXFMBiSUfQ2JNDbN"),
];

#[derive(Debug, Deserialize)]
pub struct JitoResponse<T> {
    pub result: T,
}

impl Miner {
    pub async fn jito_send_and_confirm(
        &self,
        ixs: &[Instruction],
        compute_budget: ComputeBudget,
        _skip_confirm: bool,
    ) -> eyre::Result<Signature> {
        let signer = self.signer();
        let client = self.rpc_client.clone();

        // Return error, if balance is zero
        if let Ok(balance) = client.get_balance(&signer.pubkey()).await {
            if balance <= sol_to_lamports(crate::send_and_confirm::MIN_SOL_BALANCE) {
                panic!(
                    "{} Insufficient balance: {} SOL\nPlease top up with at least {} SOL",
                    "ERROR".bold().red(),
                    lamports_to_sol(balance),
                    crate::send_and_confirm::MIN_SOL_BALANCE
                );
            }
        }

        // Set compute units
        let mut final_ixs = vec![];
        match compute_budget {
            ComputeBudget::Dynamic => {
                // TODO simulate
                final_ixs.push(ComputeBudgetInstruction::set_compute_unit_limit(1_400_000))
            }
            ComputeBudget::Fixed(cus) => {
                final_ixs.push(ComputeBudgetInstruction::set_compute_unit_limit(cus))
            }
        }
        final_ixs.push(ComputeBudgetInstruction::set_compute_unit_price(
            self.priority_fee,
        ));
        final_ixs.extend_from_slice(ixs);
        final_ixs.push(build_bribe_ix(&signer.pubkey(), 1_000_000));

        // Build tx
        let mut tx = Transaction::new_with_payer(&final_ixs, Some(&signer.pubkey()));

        // Sign tx
        let (hash, _slot) = client
            .get_latest_blockhash_with_commitment(self.rpc_client.commitment())
            .await
            .unwrap();

        tx.sign(&[&signer], hash);

        let mut bundle = Vec::with_capacity(5);
        bundle.push(tx);

        let signature = *bundle
            .first()
            .expect("empty bundle")
            .signatures
            .first()
            .expect("empty transaction");

        let bundle = bundle
            .into_iter()
            .map(|tx| match tx.encode(UiTransactionEncoding::Binary) {
                EncodedTransaction::LegacyBinary(b) => b,
                _ => panic!("impossible"),
            })
            .collect::<Vec<_>>();

        make_jito_request("sendBundle", json!([bundle])).await?;

        println!("signature:{:?}", signature);
        Ok(signature)
    }
}

async fn make_jito_request<T>(method: &'static str, params: Value) -> eyre::Result<T>
where
    T: de::DeserializeOwned,
{
    let response = reqwest::Client::new()
        .post("https://ny.mainnet.block-engine.jito.wtf/api/v1/bundle")
        .header("Content-Type", "application/json")
        .json(&json!({"jsonrpc": "2.0", "id": 1, "method": method, "params": params}))
        .send()
        .await;

    let response = match response {
        Ok(response) => response,
        Err(err) => eyre::bail!("fail to send request: {err}"),
    };

    let status = response.status();
    let text = match response.text().await {
        Ok(text) => text,
        Err(err) => eyre::bail!("fail to read response content: {err:#}"),
    };

    if !status.is_success() {
        eyre::bail!("status code: {status}, response: {text}");
    }

    let response: T = match serde_json::from_str(&text) {
        Ok(response) => response,
        Err(err) => {
            eyre::bail!("fail to deserialize response: {err:#}, response: {text}, status: {status}")
        }
    };

    Ok(response)
}

pub fn build_bribe_ix(pubkey: &Pubkey, value: u64) -> solana_sdk::instruction::Instruction {
    solana_sdk::system_instruction::transfer(
        pubkey,
        &JITO_RECIPIENTS[rand::thread_rng().gen_range(0..JITO_RECIPIENTS.len())],
        value,
    )
}
