use std::{
    io::{stdout, Write},
    str::FromStr,
    sync::{atomic::AtomicBool, Arc, Mutex},
};

use ore::{
    self,
    state::{Bus, Proof},
    utils::AccountDeserialize,
    BUS_ADDRESSES, BUS_COUNT, EPOCH_DURATION,
};
use rand::Rng;
use solana_program::{
    keccak::HASH_BYTES,
    program_memory::sol_memcmp,
    pubkey::{self, Pubkey},
};
use solana_sdk::{
    compute_budget::ComputeBudgetInstruction,
    keccak::{hashv, Hash as KeccakHash},
    signature::Signer,
    system_instruction::transfer,
};

use crate::{
    cu_limits::{CU_LIMIT_MINE, CU_LIMIT_RESET},
    utils::{get_clock_account, get_proof, get_treasury, proof_pubkey},
    Miner,
};

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
// Odds of being selected to submit a reset tx
const RESET_ODDS: u64 = 20;
const CLAIM_PERIOD: u16 = 100;

impl Miner {
    pub async fn mine(&self, threads: u64, auto_claim: bool) {
        // Register, if needed.
        let signer = self.signer();
        self.register().await;
        let mut count = 0_u16;
        let mut rng = rand::thread_rng();

        let jito_addresses = vec![
            "96gYZGLnJYVFmbjzopPSU6QiEV5fGqZNyN9nmNhvrZU5",
            "HFqU5x63VTqvQss8hp11i4wVV8bD44PvwucfZ2bU7gRe",
            "Cw8CFyM9FkoMi7K7Crf6HNQqf4uEMzpKw6QNghXLvLkY",
            "ADaUMid9yfUytqMBgopwjb2DTLSokTSzL1zt6iGPaS49",
            "DfXygSm4jCyNCybVYYK6DwvWqjKee8pbDmJGcLWNDXjh",
            "ADuUkR4vqLUMWXxW9gh6D6L8pMSawimctcNZ5pGwDcEt",
            "DttWaMuVvTiduZRnguLF7jNxTgiMBZ1hyAumKUiL2KRL",
            "3AVi9Tg9Uo68tJfuvoKvqKNWKkC5wPdSSdeBnizKZ6jT",
        ];

        // Start mining loop
        loop {
            // Fetch account state
            let balance = self.get_ore_display_balance().await;
            println!("Balance:{}", balance);
            let treasury = get_treasury(&self.rpc_client).await;
            let proof = get_proof(&self.rpc_client, signer.pubkey()).await;

            if auto_claim
                && count % CLAIM_PERIOD == 0
                && balance.parse::<f64>().unwrap_or(0.0) > 0.0
            {
                println!("Auto-claiming rewards...");
                let pubkey = signer.pubkey();
                let client = self.rpc_client.clone();
                let amount = match client.get_account(&proof_pubkey(pubkey)).await {
                    Ok(proof_account) => {
                        let proof = Proof::try_from_bytes(&proof_account.data).unwrap();
                        proof.claimable_rewards
                    }
                    Err(err) => {
                        println!("Error looking up claimable rewards: {:?}", err);
                        return;
                    }
                };

                self.claim(None, Some(amount as f64)).await;
            }

            count += 1;

            // Escape sequence that clears the screen and the scrollback buffer
            let (next_hash, nonce) =
                self.find_next_hash_par(proof.hash.into(), treasury.difficulty.into(), threads);

            // Submit mine tx.
            // Use busses randomly so on each epoch, transactions don't pile on the same busses
            'submit: loop {
                println!("\n\nSubmitting hash for validation...");
                // Double check we're submitting for the right challenge
                let proof_ = get_proof(&self.rpc_client, signer.pubkey()).await;
                if !self.validate_hash(
                    next_hash,
                    proof_.hash.into(),
                    signer.pubkey(),
                    nonce,
                    treasury.difficulty.into(),
                ) {
                    println!("Hash already validated! An earlier transaction must have landed.");
                    break 'submit;
                }

                // Reset epoch, if needed
                let treasury = get_treasury(&self.rpc_client).await;
                let clock = get_clock_account(&self.rpc_client).await;
                let threshold = treasury.last_reset_at.saturating_add(EPOCH_DURATION);
                if clock.unix_timestamp.ge(&threshold) {
                    // There are a lot of miners right now, so randomly select into submitting tx
                    if rng.gen_range(0..RESET_ODDS).eq(&0) {
                        let cu_limit_ix =
                            ComputeBudgetInstruction::set_compute_unit_limit(CU_LIMIT_RESET);
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
                        let fuckmesilly =
                            rand::thread_rng().gen_range(((p50 / 2) + p50)..(p25 / 5 + p75));
                        let cu_price_ix =
                            ComputeBudgetInstruction::set_compute_unit_price(self.priority_fee);
                        let reset_ix = ore::instruction::reset(signer.pubkey());
                        self.send_and_confirm(
                            &[cu_limit_ix, cu_price_ix, reset_ix],
                            false,
                            true,
                            fuckmesilly,
                        )
                        .await
                        .ok();
                    }
                }

                // Submit request.
                let bus = self.find_bus_id(treasury.reward_rate).await;
                let bus_rewards = (bus.rewards as f64) / (10f64.powf(ore::TOKEN_DECIMALS as f64));
                println!("Sending on bus {} ({} ORE)", bus.id, bus_rewards);
                let cu_limit_ix = ComputeBudgetInstruction::set_compute_unit_limit(CU_LIMIT_MINE);
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
                let cu_price_ix =
                    ComputeBudgetInstruction::set_compute_unit_price(self.priority_fee);

                // jito Tips
                let mut rng = rand::thread_rng();
                let random_index = rng.gen_range(0..jito_addresses.len());
                let selected_address = jito_addresses[random_index];

                let jito_tips = transfer(
                    &signer.pubkey(),
                    &pubkey::Pubkey::from_str(selected_address).unwrap(),
                    self.jito_fee,
                );

                let ix_mine = ore::instruction::mine(
                    signer.pubkey(),
                    BUS_ADDRESSES[bus.id as usize],
                    next_hash.into(),
                    nonce,
                );

                let mut ixs: Vec<_> = vec![cu_limit_ix, cu_price_ix, ix_mine];

                if self.jito_enable {
                    ixs.insert(0, jito_tips);
                }

                match self.send_and_confirm(&ixs, false, false, fuckmesilly).await {
                    Ok(sig) => {
                        println!("Success: {}", sig);
                        break;
                    }
                    Err(_err) => {
                        // TODO
                    }
                }
            }
        }
    }

    async fn find_bus_id(&self, reward_rate: u64) -> Bus {
        let mut rng = rand::thread_rng();
        loop {
            let bus_id = rng.gen_range(0..BUS_COUNT);
            if let Ok(bus) = self.get_bus(bus_id).await {
                if bus.rewards.gt(&reward_rate.saturating_mul(20)) {
                    return bus;
                }
            }
        }
    }

    fn _find_next_hash(&self, hash: KeccakHash, difficulty: KeccakHash) -> (KeccakHash, u64) {
        let signer = self.signer();
        let mut next_hash: KeccakHash;
        let mut nonce = 0u64;
        loop {
            next_hash = hashv(&[
                hash.to_bytes().as_slice(),
                signer.pubkey().to_bytes().as_slice(),
                nonce.to_le_bytes().as_slice(),
            ]);
            if next_hash.le(&difficulty) {
                break;
            } else {
                println!("Invalid hash: {} Nonce: {:?}", next_hash.to_string(), nonce);
            }
            nonce += 1;
        }
        (next_hash, nonce)
    }

    fn find_next_hash_par(
        &self,
        hash: KeccakHash,
        difficulty: KeccakHash,
        threads: u64,
    ) -> (KeccakHash, u64) {
        let found_solution = Arc::new(AtomicBool::new(false));
        let solution = Arc::new(Mutex::<(KeccakHash, u64)>::new((
            KeccakHash::new_from_array([0; 32]),
            0,
        )));
        let signer = self.signer();
        let pubkey = signer.pubkey();
        let thread_handles: Vec<_> = (0..threads)
            .map(|i| {
                std::thread::spawn({
                    let found_solution = found_solution.clone();
                    let solution = solution.clone();
                    let mut stdout = stdout();
                    move || {
                        let n = u64::MAX.saturating_div(threads).saturating_mul(i);
                        let mut next_hash: KeccakHash;
                        let mut nonce: u64 = n;
                        loop {
                            next_hash = hashv(&[
                                hash.to_bytes().as_slice(),
                                pubkey.to_bytes().as_slice(),
                                nonce.to_le_bytes().as_slice(),
                            ]);
                            if nonce % 10_000 == 0 {
                                if found_solution.load(std::sync::atomic::Ordering::Relaxed) {
                                    return;
                                }
                                if n == 0 {
                                    stdout
                                        .write_all(
                                            format!("\r{}", next_hash.to_string()).as_bytes(),
                                        )
                                        .ok();
                                }
                            }
                            if next_hash.le(&difficulty) {
                                found_solution.store(true, std::sync::atomic::Ordering::Relaxed);
                                let mut w_solution = solution.lock().expect("failed to lock mutex");
                                *w_solution = (next_hash, nonce);
                                return;
                            }
                            nonce += 1;
                        }
                    }
                })
            })
            .collect();

        for thread_handle in thread_handles {
            thread_handle.join().unwrap();
        }

        let r_solution = solution.lock().expect("Failed to get lock");
        *r_solution
    }

    pub fn validate_hash(
        &self,
        hash: KeccakHash,
        current_hash: KeccakHash,
        signer: Pubkey,
        nonce: u64,
        difficulty: KeccakHash,
    ) -> bool {
        // Validate hash correctness
        let hash_ = hashv(&[
            current_hash.as_ref(),
            signer.as_ref(),
            nonce.to_le_bytes().as_slice(),
        ]);
        if sol_memcmp(hash.as_ref(), hash_.as_ref(), HASH_BYTES) != 0 {
            return false;
        }

        // Validate hash difficulty
        if hash.gt(&difficulty) {
            return false;
        }

        true
    }

    pub async fn get_ore_display_balance(&self) -> String {
        let client = self.rpc_client.clone();
        let signer = self.signer();
        let token_account_address = spl_associated_token_account::get_associated_token_address(
            &signer.pubkey(),
            &ore::MINT_ADDRESS,
        );
        match client.get_token_account(&token_account_address).await {
            Ok(token_account) => {
                if let Some(token_account) = token_account {
                    token_account.token_amount.ui_amount_string
                } else {
                    "0.00".to_string()
                }
            }
            Err(_) => "0.00".to_string(),
        }
    }
}
