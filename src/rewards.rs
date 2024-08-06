use crate::{
    utils::{amount_u64_to_string, get_config},
    Miner,
};

impl Miner {
    pub async fn rewards(&self) {
        let config = get_config(&self.rpc_client).await;
        let base_reward_rate = config.base_reward_rate;
        let base_difficulty = ore_api::consts::MIN_DIFFICULTY;

        let mut s = format!(
            "{}: {} ORE",
            base_difficulty,
            amount_u64_to_string(base_reward_rate)
        )
        .to_string();
        for i in 1..32 {
            let reward_rate = base_reward_rate.saturating_mul(2u64.saturating_pow(i));
            s = format!(
                "{}\n{}: {} ORE",
                s,
                base_difficulty + i,
                amount_u64_to_string(reward_rate)
            );
        }
        println!("{}", s);
    }
}
