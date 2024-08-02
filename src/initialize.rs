use ore_api::consts::TREASURY_ADDRESS;
use solana_sdk::{signature::Signer, transaction::Transaction};

use crate::Miner;

impl Miner {
    pub async fn initialize(&self) {
        // Return early if program is already initialized
        if self.rpc_client.get_account(&TREASURY_ADDRESS).await.is_ok() {
            return;
        }

        // Submit initialize tx
        let blockhash = self.rpc_client.get_latest_blockhash().await.unwrap();
        let ix = ore_api::instruction::initialize(self.signer().pubkey());
        let tx = Transaction::new_signed_with_payer(
            &[ix],
            Some(&self.signer().pubkey()),
            &[&self.signer()],
            blockhash,
        );
        let res = self.rpc_client.send_and_confirm_transaction(&tx).await;
        println!("{:?}", res);
    }
}
