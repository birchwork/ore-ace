use solana_sdk::signature::Signer;

use crate::{send_and_confirm::ComputeBudget, utils::proof_pubkey, Miner};

impl Miner {
    pub async fn open(&self) {
        // Return early if miner is already registered
        let signer = self.signer();
        let proof_address = proof_pubkey(signer.pubkey());
        if self.rpc_client.get_account(&proof_address).await.is_ok() {
            return;
        }

        // Sign and send transaction.
        println!("Generating challenge...");
        let ix = ore_api::instruction::open(signer.pubkey(), signer.pubkey(), signer.pubkey());
        self.send_and_confirm(&[ix], ComputeBudget::Dynamic, false)
            .await
            .ok();
    }
}
