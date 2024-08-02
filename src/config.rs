use colored::Colorize;

use crate::{
    utils::{amount_u64_to_string, get_config},
    Miner,
};

impl Miner {
    pub async fn config(&self) {
        let config = get_config(&self.rpc_client).await;
        println!("{}: {}", "Last reset at".bold(), config.last_reset_at);
        println!("{}: {}", "Min difficulty".bold(), config.min_difficulty);
        println!("{}: {}", "Base reward rate".bold(), config.base_reward_rate);
        println!(
            "{}: {} ORE",
            "Top stake".bold(),
            amount_u64_to_string(config.top_balance)
        );
    }
}
