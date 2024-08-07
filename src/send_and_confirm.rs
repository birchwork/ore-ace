use std::time::Duration;

use colored::*;
use serde_json::json;
use solana_client::nonblocking::rpc_client::RpcClient;
use solana_client::{
    client_error::{ClientError, ClientErrorKind, Result as ClientResult},
    rpc_config::RpcSendTransactionConfig,
};
use solana_client::{rpc_request::RpcRequest, rpc_response::Response};
use solana_program::clock::Slot;
use solana_program::{
    instruction::Instruction,
    native_token::{lamports_to_sol, sol_to_lamports},
};
use solana_rpc_client::spinner;
use solana_sdk::{
    commitment_config::CommitmentLevel,
    compute_budget::ComputeBudgetInstruction,
    signature::{Signature, Signer},
    transaction::Transaction,
};
use solana_transaction_status::{
    TransactionConfirmationStatus, TransactionStatus, UiTransactionEncoding,
};

use tracing::{debug, error, info, warn};

use crate::{constant, jito, utils};

use crate::Miner;

const MIN_SOL_BALANCE: f64 = 0.005;

const RPC_RETRIES: usize = 0;
const _SIMULATION_RETRIES: usize = 4;
const GATEWAY_RETRIES: usize = 100;
const CONFIRM_RETRIES: usize = 1;

const CONFIRM_DELAY: u64 = 0;
const GATEWAY_DELAY: u64 = 500;

pub enum ComputeBudget {
    Dynamic,
    Fixed(u32),
}

impl Miner {
    pub async fn send_and_confirm(
        &self,
        ixs: &[Instruction],
        compute_budget: ComputeBudget,
        skip_confirm: bool,
    ) -> ClientResult<Signature> {
        let progress_bar = spinner::new_progress_bar();
        let signer = self.signer();
        let fee_payer = self.fee_payer();
        let client = self.rpc_client.clone();

        // Return error, if balance is zero
        if let Ok(balance) = client.get_balance(&fee_payer.pubkey()).await {
            if balance <= sol_to_lamports(MIN_SOL_BALANCE) {
                panic!(
                    "{} Insufficient balance: {} SOL\nPlease top up with at least {} SOL",
                    "ERROR".bold().red(),
                    lamports_to_sol(balance),
                    MIN_SOL_BALANCE
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

        // Build tx
        let send_cfg = RpcSendTransactionConfig {
            skip_preflight: true,
            preflight_commitment: Some(CommitmentLevel::Confirmed),
            encoding: Some(UiTransactionEncoding::Base64),
            max_retries: Some(RPC_RETRIES),
            min_context_slot: None,
        };
        let mut tx = Transaction::new_with_payer(&final_ixs, Some(&fee_payer.pubkey()));

        // Sign tx
        let (hash, _slot) = client
            .get_latest_blockhash_with_commitment(self.rpc_client.commitment())
            .await
            .unwrap();
        tx.sign(&[&signer, &fee_payer], hash);

        // Submit tx
        let mut attempts = 0;
        loop {
            progress_bar.set_message(format!("Submitting transaction... (attempt {})", attempts));
            match client.send_transaction_with_config(&tx, send_cfg).await {
                Ok(sig) => {
                    // Skip confirmation
                    if skip_confirm {
                        progress_bar.finish_with_message(format!("Sent: {}", sig));
                        return Ok(sig);
                    }

                    // Confirm the tx landed
                    for _ in 0..CONFIRM_RETRIES {
                        std::thread::sleep(Duration::from_millis(CONFIRM_DELAY));
                        match client.get_signature_statuses(&[sig]).await {
                            Ok(signature_statuses) => {
                                for status in signature_statuses.value {
                                    if let Some(status) = status {
                                        if let Some(err) = status.err {
                                            progress_bar.finish_with_message(format!(
                                                "{}: {}",
                                                "ERROR".bold().red(),
                                                err
                                            ));
                                            return Err(ClientError {
                                                request: None,
                                                kind: ClientErrorKind::Custom(err.to_string()),
                                            });
                                        }
                                        if let Some(confirmation) = status.confirmation_status {
                                            match confirmation {
                                                TransactionConfirmationStatus::Processed => {}
                                                TransactionConfirmationStatus::Confirmed
                                                | TransactionConfirmationStatus::Finalized => {
                                                    progress_bar.finish_with_message(format!(
                                                        "{} {}",
                                                        "OK".bold().green(),
                                                        sig
                                                    ));
                                                    return Ok(sig);
                                                }
                                            }
                                        }
                                    }
                                }
                            }

                            // Handle confirmation errors
                            Err(err) => {
                                progress_bar.set_message(format!(
                                    "{}: {}",
                                    "ERROR".bold().red(),
                                    err.kind().to_string()
                                ));
                            }
                        }
                    }
                }

                // Handle submit errors
                Err(err) => {
                    progress_bar.set_message(format!(
                        "{}: {}",
                        "ERROR".bold().red(),
                        err.kind().to_string()
                    ));
                }
            }

            // Retry
            std::thread::sleep(Duration::from_millis(GATEWAY_DELAY));
            attempts += 1;
            if attempts > GATEWAY_RETRIES {
                progress_bar.finish_with_message(format!("{}: Max retries", "ERROR".bold().red()));
                return Err(ClientError {
                    request: None,
                    kind: ClientErrorKind::Custom("Max retries".into()),
                });
            }
        }
    }

    pub async fn send_and_confirm_by_jito(
        &self,
        ixs: &[Instruction],
        compute_budget: ComputeBudget,
        tip: u64,
    ) {
        // if tips.p50() > 0 {
        //
        // }

        let fee_payer = self.fee_payer();
        let signer = self.signer();
        let client = self.rpc_client.clone();

        // Return error, if balance is zero
        if let Ok(balance) = client.get_balance(&fee_payer.pubkey()).await {
            if balance <= sol_to_lamports(MIN_SOL_BALANCE) {
                panic!(
                    "{} Insufficient balance: {} SOL\nPlease top up with at least {} SOL",
                    "ERROR".bold().red(),
                    lamports_to_sol(balance),
                    MIN_SOL_BALANCE
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

        let miner = signer.pubkey().to_string();
        let bundle_tipper = signer.pubkey();
        final_ixs.push(jito::build_bribe_ix(&bundle_tipper, tip));

        let mut tx = Transaction::new_with_payer(&final_ixs, Some(&fee_payer.pubkey()));
        // Sign tx
        let (hash, _slot) = client
            .get_latest_blockhash_with_commitment(self.rpc_client.commitment())
            .await
            .unwrap();
        let send_at_slot = _slot;
        let mut latest_slot = _slot;
        let mut landed_tx = vec![];

        tx.sign(&[&signer, &fee_payer], hash);
        let mut bundle = Vec::with_capacity(5);
        bundle.push(tx);
        let task = tokio::spawn(async move { jito::send_bundle(bundle).await });

        let (signature, bundle_id) = match task.await.unwrap() {
            Ok(r) => r,
            Err(err) => {
                error!(miner, "fail to send bundle: {err:#}");
                return;
            }
        };

        let mut signatures = vec![];
        signatures.push(signature);

        while landed_tx.is_empty() && latest_slot < send_at_slot + constant::SLOT_EXPIRATION {
            tokio::time::sleep(Duration::from_secs(2)).await;
            debug!(miner, latest_slot, send_at_slot, "checking bundle status");
            let (statuses, slot) = match Self::get_signature_statuses(&client, &signatures).await {
                Ok(value) => value,
                Err(err) => {
                    error!(
                        miner,
                        latest_slot, send_at_slot, "fail to get bundle status: {err:#}"
                    );
                    tokio::time::sleep(Duration::from_secs(2)).await;
                    continue;
                }
            };

            latest_slot = slot;
            landed_tx = utils::find_landed_txs(&signatures, statuses);
        }

        if !landed_tx.is_empty() {
            info!(
                miner,
                first_tx = ?landed_tx.first().unwrap(),
                "bundle mined",
            );
        } else {
            println!(
                "Bundle dropped: {} {} {}",
                bundle_tipper.to_string(),
                bundle_id,
                tip
            );
            warn!(
                miner,
                tip,
                %tip,
                "bundle dropped"
            );
        }
    }

    pub async fn get_signature_statuses(
        client: &RpcClient,
        signatures: &[Signature],
    ) -> eyre::Result<(Vec<Option<TransactionStatus>>, Slot)> {
        let signatures_params = signatures.iter().map(|s| s.to_string()).collect::<Vec<_>>();

        let (statuses, slot) = match client
            .send::<Response<Vec<Option<TransactionStatus>>>>(
                RpcRequest::GetSignatureStatuses,
                json!([signatures_params]),
            )
            .await
        {
            Ok(result) => (result.value, result.context.slot),
            Err(err) => eyre::bail!("fail to get bundle status: {err}"),
        };

        Ok((statuses, slot))
    }

    // TODO
    fn _simulate(&self) {

        // Simulate tx
        // let mut sim_attempts = 0;
        // 'simulate: loop {
        //     let sim_res = client
        //         .simulate_transaction_with_config(
        //             &tx,
        //             RpcSimulateTransactionConfig {
        //                 sig_verify: false,
        //                 replace_recent_blockhash: true,
        //                 commitment: Some(self.rpc_client.commitment()),
        //                 encoding: Some(UiTransactionEncoding::Base64),
        //                 accounts: None,
        //                 min_context_slot: Some(slot),
        //                 inner_instructions: false,
        //             },
        //         )
        //         .await;
        //     match sim_res {
        //         Ok(sim_res) => {
        //             if let Some(err) = sim_res.value.err {
        //                 println!("Simulaton error: {:?}", err);
        //                 sim_attempts += 1;
        //             } else if let Some(units_consumed) = sim_res.value.units_consumed {
        //                 if dynamic_cus {
        //                     println!("Dynamic CUs: {:?}", units_consumed);
        //                     let cu_budget_ix = ComputeBudgetInstruction::set_compute_unit_limit(
        //                         units_consumed as u32 + 1000,
        //                     );
        //                     let cu_price_ix =
        //                         ComputeBudgetInstruction::set_compute_unit_price(self.priority_fee);
        //                     let mut final_ixs = vec![];
        //                     final_ixs.extend_from_slice(&[cu_budget_ix, cu_price_ix]);
        //                     final_ixs.extend_from_slice(ixs);
        //                     tx = Transaction::new_with_payer(&final_ixs, Some(&signer.pubkey()));
        //                 }
        //                 break 'simulate;
        //             }
        //         }
        //         Err(err) => {
        //             println!("Simulaton error: {:?}", err);
        //             sim_attempts += 1;
        //         }
        //     }

        //     // Abort if sim fails
        //     if sim_attempts.gt(&SIMULATION_RETRIES) {
        //         return Err(ClientError {
        //             request: None,
        //             kind: ClientErrorKind::Custom("Simulation failed".into()),
        //         });
        //     }
        // }
    }
}
