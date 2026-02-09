use super::{ConsensusEngine, ConsensusError};
use crate::account::AccountState;
use crate::Block;
#[derive(Debug, Clone)]
pub struct PoWConfig {
    pub difficulty: usize,
    pub target_block_time: u64,
    pub adjustment_interval: u64,
    pub block_reward: u64,
}
impl Default for PoWConfig {
    fn default() -> Self {
        PoWConfig {
            difficulty: 2,
            target_block_time: 10,
            adjustment_interval: 100,
            block_reward: 50,
        }
    }
}
pub struct PoWEngine {
    pub config: PoWConfig,
}
impl PoWEngine {
    pub fn new(difficulty: usize) -> Self {
        PoWEngine {
            config: PoWConfig {
                difficulty,
                ..Default::default()
            },
        }
    }
    pub fn with_config(config: PoWConfig) -> Self {
        PoWEngine { config }
    }
    fn target(&self) -> String {
        "0".repeat(self.config.difficulty)
    }
    fn meets_difficulty(&self, hash: &str) -> bool {
        hash.starts_with(&self.target())
    }
    fn mine(&self, block: &mut Block) {
        let target = self.target();
        let mut iterations: u64 = 0;
        println!(
            "⛏️  Mining started (difficulty: {}, target: {}...)",
            self.config.difficulty, target
        );
        while !block.hash.starts_with(&target) {
            block.nonce += 1;
            block.hash = block.calculate_hash();
            iterations += 1;
            if iterations % 100_000 == 0 {
                println!("   ... {} iterations, nonce: {}", iterations, block.nonce);
            }
        }
        println!(
            "⛏️  Mining complete! {} iterations, nonce: {}",
            iterations, block.nonce
        );
    }
    #[allow(dead_code)]
    pub fn calculate_new_difficulty(&self, chain: &[Block]) -> usize {
        if chain.len() < self.config.adjustment_interval as usize {
            return self.config.difficulty;
        }
        let interval = self.config.adjustment_interval as usize;
        let last_block = &chain[chain.len() - 1];
        let first_block = &chain[chain.len() - interval];
        let actual_time = (last_block.timestamp - first_block.timestamp) / 1000;
        let expected_time = self.config.target_block_time * self.config.adjustment_interval;
        let ratio = expected_time as f64 / actual_time.max(1) as f64;
        let new_diff = (self.config.difficulty as f64 * ratio) as usize;
        new_diff.clamp(1, 32)
    }
}
impl ConsensusEngine for PoWEngine {
    fn prepare_block(
        &self,
        block: &mut Block,
        _state: &AccountState,
    ) -> Result<(), ConsensusError> {
        block.hash = block.calculate_hash();
        self.mine(block);
        Ok(())
    }
    fn validate_block(
        &self,
        block: &Block,
        chain: &[Block],
        _state: &AccountState,
    ) -> Result<(), ConsensusError> {
        if block.index == 0 {
            if block.hash != block.calculate_hash() {
                return Err(ConsensusError("Invalid genesis block hash".into()));
            }
            return Ok(());
        }
        if let Some(prev_block) = chain.last() {
            if block.previous_hash != prev_block.hash {
                return Err(ConsensusError(format!(
                    "Previous hash mismatch. Expected: {}, Got: {}",
                    prev_block.hash, block.previous_hash
                )));
            }
        }
        let calculated_hash = block.calculate_hash();
        if block.hash != calculated_hash {
            return Err(ConsensusError(format!(
                "Invalid block hash. Calculated: {}, Existing: {}",
                calculated_hash, block.hash
            )));
        }
        if !self.meets_difficulty(&block.hash) {
            return Err(ConsensusError(format!(
                "Invalid PoW. {} leading zeros required, hash: {}",
                self.config.difficulty, block.hash
            )));
        }
        Ok(())
    }
    fn consensus_type(&self) -> &'static str {
        "PoW"
    }
    fn info(&self) -> String {
        format!(
            "PoW (difficulty: {}, target: {}..., reward: {} coins)",
            self.config.difficulty,
            self.target(),
            self.config.block_reward
        )
    }

    fn fork_choice_score(&self, chain: &[Block]) -> u128 {
        (chain.len() as u128) * (self.config.difficulty as u128)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_pow_mining() {
        let engine = PoWEngine::new(1);
        let mut block = Block::new(1, "0".repeat(64), vec![]);
        let state = AccountState::new();
        engine.prepare_block(&mut block, &state).unwrap();
        assert!(block.hash.starts_with("0"));
    }
    #[test]
    fn test_pow_validation() {
        let engine = PoWEngine::new(1);
        let mut block = Block::new(1, "0".repeat(64), vec![]);
        let state = AccountState::new();
        engine.prepare_block(&mut block, &state).unwrap();
        assert!(engine.validate_block(&block, &[], &state).is_ok());
        let mut tampered = block.clone();
        tampered.hash = "invalid_hash".to_string();
        assert!(engine.validate_block(&tampered, &[], &state).is_err());
    }
    #[test]
    fn test_difficulty_levels() {
        let easy = PoWEngine::new(1);
        let hard = PoWEngine::new(2);
        let mut block1 = Block::new(1, "0".repeat(64), vec![]);
        let mut block2 = Block::new(1, "0".repeat(64), vec![]);
        let state = AccountState::new();
        easy.prepare_block(&mut block1, &state).unwrap();
        hard.prepare_block(&mut block2, &state).unwrap();
        assert!(block1.hash.starts_with("0"));
        assert!(block2.hash.starts_with("00"));
    }
}
