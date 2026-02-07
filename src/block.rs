use crate::hash::hash_fields;
use crate::transaction::Transaction;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Block {
    pub index: u64,
    pub timestamp: u128,
    pub previous_hash: String,
    pub hash: String,
    pub transactions: Vec<Transaction>,
    pub nonce: u64,
}

impl Block {
    pub fn new(index: u64, previous_hash: String, transactions: Vec<Transaction>) -> Self {
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis();

        let mut block = Block {
            index,
            timestamp,
            previous_hash,
            hash: String::new(),
            transactions,
            nonce: 0,
        };
        block.hash = block.calculate_hash();
        block
    }

    pub fn genesis() -> Self {
        Block::new(0, "0".repeat(64), vec![Transaction::genesis()])
    }

    pub fn calculate_hash(&self) -> String {
        let tx_data: Vec<u8> = self
            .transactions
            .iter()
            .flat_map(|tx| tx.to_bytes())
            .collect();

        hash_fields(&[
            &self.index.to_le_bytes(),
            &self.timestamp.to_le_bytes(),
            self.previous_hash.as_bytes(),
            &tx_data,
            &self.nonce.to_le_bytes(),
        ])
    }

    pub fn mine(&mut self, difficulty: usize) {
        let target = "0".repeat(difficulty);
        while !self.hash.starts_with(&target) {
            self.nonce += 1;
            self.hash = self.calculate_hash();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_genesis_block() {
        let genesis = Block::genesis();
        assert_eq!(genesis.index, 0);
        assert_eq!(genesis.previous_hash, "0".repeat(64));
        assert!(!genesis.hash.is_empty());
    }

    #[test]
    fn test_mining() {
        let mut block = Block::genesis();
        block.mine(1); // 1 leading zero
        assert!(block.hash.starts_with("0"));
    }
}
