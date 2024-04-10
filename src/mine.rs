use std::{
    io::{stdout, Write},
    str::FromStr,
    sync::{Arc, atomic::AtomicBool, Mutex},
};

use ore::{self, BUS_ADDRESSES, BUS_COUNT, EPOCH_DURATION, state::Bus};
use rand::Rng;
use solana_client::nonblocking::rpc_client::RpcClient;
use solana_program::pubkey;
use solana_sdk::{
    commitment_config::CommitmentConfig,
    compute_budget::ComputeBudgetInstruction,
    keccak::{Hash as KeccakHash, hashv},
    signature::Signer,
    system_instruction::transfer,
};


use crate::{
    cu_limits::{CU_LIMIT_MINE, CU_LIMIT_RESET},
    Miner,
    utils::{get_clock_account, get_proof, get_treasury},
};

// Odds of being selected to submit a reset tx
const RESET_ODDS: u64 = 20;

impl Miner {
    pub async fn mine(&self, threads: u64) {
        // Register, if needed.
        let signer = self.signer();
        self.register().await;
        let mut stdout = stdout();
        let mut rng = rand::thread_rng();


        stdout.write_all(b"\x1b[2J\x1b[3J\x1b[H").ok();


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

        println!("{}: Starting to Mine", signer.pubkey());
        // Start mining loop
        'mining_loop: loop {
            // Fetch account state
            let balance = self.get_ore_display_balance().await;
            let treasury = get_treasury(self.cluster.clone()).await;
            let proof = get_proof(self.cluster.clone(), signer.pubkey()).await;
            let rewards =
                (proof.claimable_rewards as f64) / (10f64.powf(ore::TOKEN_DECIMALS as f64));
            let reward_rate =
                (treasury.reward_rate as f64) / (10f64.powf(ore::TOKEN_DECIMALS as f64));
            println!("Wallet: {}", signer.pubkey());
            println!("Balance: {} ORE", balance);
            println!("Claimable: {} ORE", rewards);
            println!("Reward rate: {} ORE", reward_rate);
            println!("Fee: {}", self.priority_fee);
            println!("Enable JitoTip: {}", self.jito_enable);
            println!("JitoTip Fee: {}", self.jito_fee);

            // Escape sequence that clears the screen and the scrollback buffer
            println!("\nMining for a valid hash...");
            let (next_hash, nonce) =
                self.find_next_hash_par(proof.hash.into(), treasury.difficulty.into(), threads);

            // Submit mine tx.
            // Use busses randomly so on each epoch, transactions don't pile on the same busses
            println!("\n\nSubmitting hash for validation...");

            loop {

                // Reset epoch, if needed
                let treasury = get_treasury(self.cluster.clone()).await;
                let clock = get_clock_account(self.cluster.clone()).await;
                let threshold = treasury.last_reset_at.saturating_add(EPOCH_DURATION);
                if clock.unix_timestamp.ge(&threshold) {
                    // There are a lot of miners right now, so randomly select into submitting tx
                    if rng.gen_range(0..RESET_ODDS).eq(&0) {
                        println!("Sending epoch reset transaction...");
                        let cu_limit_ix =
                            ComputeBudgetInstruction::set_compute_unit_limit(CU_LIMIT_RESET);
                        let cu_price_ix =
                            ComputeBudgetInstruction::set_compute_unit_price(self.priority_fee);
                        let reset_ix = ore::instruction::reset(signer.pubkey());
                        self.send_and_confirm(&[cu_limit_ix, cu_price_ix, reset_ix], false, true)
                            .await
                            .ok();
                    }
                    continue 'mining_loop;
                }

                // Submit request.
                let bus = self.find_bus_id(treasury.reward_rate).await;
                // let bus_rewards = (bus.rewards as f64) / (10f64.powf(ore::TOKEN_DECIMALS as f64));
                // println!("Sending on bus {} ({} ORE)", bus.id, bus_rewards);
                let cu_limit_ix = ComputeBudgetInstruction::set_compute_unit_limit(CU_LIMIT_MINE);
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

                match self
                    .send_and_confirm(&ixs, false, false)
                    .await
                {
                    Ok(sig) => {
                        println!("Success: {}", sig);
                        break;
                    }
                    Err(_err) => {
                        println!("send_and_confirm Error: {}", _err.to_string());
                        if self.jito_enable || Miner::should_break_loop(&_err.to_string()) {
                            continue 'mining_loop;
                        }
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
                if bus.rewards.gt(&reward_rate.saturating_mul(4)) {
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
                                stdout
                                    .write_all(format!("\r{}", next_hash.to_string()).as_bytes())
                                    .ok();
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

    pub async fn get_ore_display_balance(&self) -> String {
        let client =
            RpcClient::new_with_commitment(self.cluster.clone(), CommitmentConfig::processed());
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
            Err(_) => "Err".to_string(),
        }
    }
    pub fn should_break_loop(err_msg: &str) -> bool {
        err_msg.contains("custom program error: 0x3")
    }
}
