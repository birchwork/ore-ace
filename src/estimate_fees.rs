use solana_client::rpc_request::RpcRequest;
use solana_sdk::{
    instruction::Instruction, message::Message, pubkey::Pubkey, transaction::Transaction,
};

use crate::Miner;

impl Miner {
    pub async fn get_priority_fee_estimate(&self, ixs: &[Instruction], payer: &Pubkey) -> u64 {
        let client = self.rpc_client.clone();
        let tx = Transaction::new_unsigned(Message::new(ixs, Some(payer)));
        let param = serde_json::json!([GetPriorityFeeEstimateRequest {
            transaction: Some(bs58::encode(bincode::serialize(&tx).unwrap()).into_string()),
            options: Some(GetPriorityFeeEstimateOptions {
                priority_level: Some(self.priority_level),
                ..Default::default()
            }),
            ..Default::default()
        }]);

        let response: GetPriorityFeeEstimateResponse = client
            .send(
                RpcRequest::Custom {
                    method: "getPriorityFeeEstimate",
                },
                param,
            )
            .await
            .expect("Failed to get priority fee estimate");

        response
            .priority_fee_estimate
            .expect("Failed to get priority fee estimate")
            .round() as u64
    }
}

#[derive(
    serde::Deserialize,
    serde::Serialize,
    Debug,
    Copy,
    Clone,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    clap::ValueEnum,
)]
pub enum PriorityLevel {
    Min,      // 0th percentile
    Low,      // 25th percentile
    Medium,   // 50th percentile
    High,     // 75th percentile
    VeryHigh, // 95th percentile
    // labelled unsafe to prevent people using and draining their funds by accident
    UnsafeMax, // 100th percentile
    Default,   // 50th percentile
}

#[derive(serde::Deserialize, serde::Serialize, Default)]
#[serde(rename_all = "camelCase")]
struct GetPriorityFeeEstimateRequest {
    transaction: Option<String>,       // estimate fee for a serialized txn
    account_keys: Option<Vec<String>>, // estimate fee for a list of accounts
    options: Option<GetPriorityFeeEstimateOptions>,
}

#[derive(serde::Deserialize, serde::Serialize, Default)]
#[serde(rename_all = "camelCase")]
struct GetPriorityFeeEstimateOptions {
    priority_level: Option<PriorityLevel>, // Default to MEDIUM
    include_all_priority_fee_levels: Option<bool>, // Include all priority level estimates in the response
    transaction_encoding: Option<solana_transaction_status::UiTransactionEncoding>, // Default Base58
    lookback_slots: Option<u8>, // number of slots to look back to calculate estimate. Valid number are 1-150, defualt is 150
}

#[derive(serde::Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
struct GetPriorityFeeEstimateResponse {
    priority_fee_estimate: Option<MicroLamportPriorityFee>,
    // priority_fee_levels: Option<serde_json::Value>,
}

type MicroLamportPriorityFee = f64;
// #[derive(serde::Deserialize, Debug)]
// #[serde(rename_all = "camelCase")]
// struct MicroLamportPriorityFeeLevels {
//     none: f64,
//     low: f64,
//     medium: f64,
//     high: f64,
//     very_high: f64,
//     unsafe_max: f64,
// }
