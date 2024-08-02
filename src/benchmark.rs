use std::{sync::Arc, time::Instant};

use solana_rpc_client::spinner;

use crate::{args::BenchmarkArgs, Miner};

const TEST_DURATION: i64 = 30;

impl Miner {
    pub async fn benchmark(&self, args: BenchmarkArgs) {
        // Check num threads
        self.check_num_cores(args.threads);

        // Dispatch job to each thread
        let challenge = [0; 32];
        let progress_bar = Arc::new(spinner::new_progress_bar());
        progress_bar.set_message(format!(
            "Benchmarking. This will take {} sec...",
            TEST_DURATION
        ));
        let handles: Vec<_> = (0..args.threads)
            .map(|i| {
                std::thread::spawn({
                    move || {
                        let timer = Instant::now();
                        let first_nonce = u64::MAX.saturating_div(args.threads).saturating_mul(i);
                        let mut nonce = first_nonce;
                        loop {
                            // Create hash
                            let _hx = drillx::hash(&challenge, &nonce.to_le_bytes());

                            // Increment nonce
                            nonce += 1;

                            // Exit if time has elapsed
                            if (timer.elapsed().as_secs() as i64).ge(&TEST_DURATION) {
                                break;
                            }
                        }

                        // Return hash count
                        nonce - first_nonce
                    }
                })
            })
            .collect();

        // Join handles and return best nonce
        let mut total_nonces = 0;
        for h in handles {
            if let Ok(count) = h.join() {
                total_nonces += count;
            }
        }

        // Update log
        progress_bar.finish_with_message(format!(
            "Hashpower: {} H/sec",
            total_nonces.saturating_div(TEST_DURATION as u64),
        ));
    }
}
