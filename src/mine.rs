use std::{sync::Arc, time::Instant};

use colored::*;
use drillx::{
    equix::{self},
    Hash, Solution,
};
use ore_api::{
    consts::{BUS_ADDRESSES, BUS_COUNT, EPOCH_DURATION},
    state::{Config, Proof},
};
use rand::Rng;
use solana_program::pubkey::Pubkey;
use solana_rpc_client::spinner;
use solana_sdk::signer::Signer;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;


use crate::{
    args::MineArgs,
    send_and_confirm::ComputeBudget,
    utils::{amount_u64_to_string, get_clock, get_config, get_proof_with_authority, proof_pubkey},
    Miner,
};

impl Miner {
    pub async fn mine(&self, args: MineArgs) {
        // Register, if needed.
        let signer = self.signer();
        self.open().await;

        // Check num threads
        self.check_num_cores(args.threads);

        // Start mining loop
        loop {
            // Fetch proof
            let proof = get_proof_with_authority(&self.rpc_client, signer.pubkey()).await;
            println!(
                "\nStake balance: {} ORE",
                amount_u64_to_string(proof.balance)
            );

            // Calc cutoff time
            let cutoff_time = self.get_cutoff(proof, args.buffer_time).await;

            // Run drillx
            let config = get_config(&self.rpc_client).await;
            let solution = Self::find_hash_par(
                proof,
                cutoff_time,
                args.threads,
                config.min_difficulty as u32,
            )
            .await;

            // Submit most difficult hash
            let mut compute_budget = 500_000;
            let mut ixs = vec![ore_api::instruction::auth(proof_pubkey(signer.pubkey()))];
            if self.should_reset(config).await {
                compute_budget += 100_000;
                ixs.push(ore_api::instruction::reset(signer.pubkey()));
            }
            ixs.push(ore_api::instruction::mine(
                signer.pubkey(),
                signer.pubkey(),
                find_bus(),
                solution,
            ));
            self.send_and_confirm(&ixs, ComputeBudget::Fixed(compute_budget), false)
                .await
                .ok();
        }
    }

    async fn find_hash_par(
        proof: Proof,
        cutoff_time: u64,
        threads: u64,
        min_difficulty: u32,
    ) -> Solution {

        let progress_bar = Arc::new(spinner::new_progress_bar());
        progress_bar.set_message("Mining...");
        let found_solution = Arc::new(AtomicBool::new(false));
    
        let start_time = Instant::now();
    
        let handles: Vec<_> = (0..threads)
            .map(|i| {
                let proof = proof.clone();
                let progress_bar = progress_bar.clone();
                let found_solution = found_solution.clone();
                let start_time = start_time.clone();
                std::thread::spawn(move || {
                    let mut memory = equix::SolverMemory::new();
                    let mut nonce = u64::MAX.saturating_div(threads).saturating_mul(i);
                    let mut best_nonce = nonce;
                    let mut best_difficulty = 0;
                    let mut best_hash = Hash::default();
                    let mut increment: u64 = 1;
                    loop {
                        if found_solution.load(Ordering::Relaxed) {
                            break;
                        }
    
                        // Create hash
                        if let Ok(hx) = drillx::hash_with_memory(
                            &mut memory,
                            &proof.challenge,
                            &nonce.to_le_bytes(),
                        ) {
                            let difficulty = hx.difficulty();
                            let elapsed_time = start_time.elapsed();
                            if difficulty >= 20 {
                                found_solution.store(true, Ordering::Relaxed);
                                if elapsed_time < Duration::from_secs(60) {
                                    // 如果未满60秒，等待至60秒
                                    let wait_time = Duration::from_secs(60) - elapsed_time;
                                    std::thread::sleep(wait_time);
                                }
                                return Some((nonce, difficulty, hx));
                            }
                            if difficulty.gt(&best_difficulty) {
                                best_nonce = nonce;
                                best_difficulty = difficulty;
                                best_hash = hx;
                                // 如果找到更好的难度，减小增量以更仔细地探索这个区域
                                increment = increment.max(1) / 2;
                            } else {
                                // 如果没有改进，逐渐增加增量
                                increment = increment.saturating_add(1);
                            }
                        }
    
                        // Exit if time has elapsed
                        if nonce % 10 == 0 {
                            let elapsed_time = start_time.elapsed().as_secs();
                            if elapsed_time.ge(&cutoff_time) {
                                if best_difficulty.gt(&min_difficulty) {
                                    // Mine until min difficulty has been met
                                    break;
                                }
                            } else if i == 0 {
                                progress_bar.set_message(format!(
                                    "Mining... ({} sec remaining) (bestdiff:{})",
                                    cutoff_time.saturating_sub(elapsed_time),best_difficulty,
                                ));
                            }
                        }
    
                        // 智能增加 Nonce
                        nonce = nonce.wrapping_add(increment);
                    }
    
                    // Return the best nonce
                    Some((best_nonce, best_difficulty, best_hash))
                })
            })
            .collect();
    
        // Join handles and return best nonce
        let mut best_nonce = 0;
        let mut best_difficulty = 0;
        let mut best_hash = Hash::default();
        for h in handles {
            if let Ok(Some((nonce, difficulty, hash))) = h.join() {
                if difficulty >= 20 {
                    progress_bar.finish_with_message(format!(
                        "Found solution: {} (difficulty: {})",
                        bs58::encode(hash.h).into_string(),
                        difficulty
                    ));
                    return Solution::new(hash.d, nonce.to_le_bytes());
                }
                if difficulty > best_difficulty {
                    best_difficulty = difficulty;
                    best_nonce = nonce;
                    best_hash = hash;
                }
            }
        }
    
        // Update log
        progress_bar.set_message(format!(
            "Best hash: {} (difficulty: {})",
            bs58::encode(best_hash.h).into_string(),
            best_difficulty
        ));
    
        Solution::new(best_hash.d, best_nonce.to_le_bytes())
    }

    pub fn check_num_cores(&self, threads: u64) {
        // Check num threads
        let num_cores = num_cpus::get() as u64;
        if threads.gt(&num_cores) {
            println!(
                "{} Number of threads ({}) exceeds available cores ({})",
                "WARNING".bold().yellow(),
                threads,
                num_cores
            );
        }
    }

    async fn should_reset(&self, config: Config) -> bool {
        let clock = get_clock(&self.rpc_client).await;
        config
            .last_reset_at
            .saturating_add(EPOCH_DURATION)
            .saturating_sub(5) // Buffer
            .le(&clock.unix_timestamp)
    }

    async fn get_cutoff(&self, proof: Proof, buffer_time: u64) -> u64 {
        let clock = get_clock(&self.rpc_client).await;
        proof
            .last_hash_at
            .saturating_add(60)
            .saturating_sub(buffer_time as i64)
            .saturating_sub(clock.unix_timestamp)
            .max(0) as u64
    }
}

// TODO Pick a better strategy (avoid draining bus)
fn find_bus() -> Pubkey {
    let i = rand::thread_rng().gen_range(0..BUS_COUNT);
    BUS_ADDRESSES[i]
}
