use colored::Colorize;

use crate::{
    utils::{amount_u64_to_string, get_config},
    Miner,
};

impl Miner {
    pub async fn config(&self) {
        let config = get_config(&self.rpc_client).await;
        println!("{}: {}", "Last reset".bold(), config.last_reset_at);
        println!("{}: {}", "Top staker".bold(), config.top_staker);
        println!(
            "{}: {} ORE",
            "Top stake".bold(),
            amount_u64_to_string(config.max_stake)
        );
    }
}
