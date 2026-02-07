use crate::storage::Storage;
use crate::{Block, Transaction};

#[derive(Debug, Clone)]
pub struct Blockchain {
    pub chain: Vec<Block>,
    pub difficulty: usize,
    pub pending_transactions: Vec<Transaction>,
    pub storage: Option<Storage>,
}

impl Blockchain {
    pub fn new(difficulty: usize, storage: Option<Storage>) -> Self {
        let mut chain = Blockchain {
            chain: Vec::new(),
            difficulty,
            pending_transactions: Vec::new(),
            storage,
        };

        if let Some(ref store) = chain.storage {
            if let Ok(Some(last_hash)) = store.get_last_hash() {
                println!("Found existing chain tip: {}", last_hash);
                if let Err(e) = chain.load_chain_from_db(last_hash) {
                    println!("❌ Failed to load chain: {}", e);
                    chain.chain.clear();
                    chain.create_genesis_block();
                }
            } else {
                chain.create_genesis_block();
            }
        } else {
            chain.create_genesis_block();
        }

        chain
    }

    fn load_chain_from_db(&mut self, last_hash: String) -> std::io::Result<()> {
        let mut current_hash = last_hash;
        let mut blocks = Vec::new();

        if let Some(ref store) = self.storage {
            while let Ok(Some(block)) = store.get_block(&current_hash) {
                blocks.push(block.clone());
                if block.previous_hash == "0".repeat(64) {
                    break;
                }
                current_hash = block.previous_hash;
            }
        }

        if blocks.is_empty() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::Other,
                "Chain broken or empty",
            ));
        }

        blocks.reverse();
        self.chain = blocks;
        println!("✅ Loaded {} blocks from disk", self.chain.len());
        Ok(())
    }

    fn create_genesis_block(&mut self) {
        let genesis_block = Block::genesis();
        self.chain.push(genesis_block.clone());
        if let Some(ref store) = self.storage {
            let _ = store.insert_block(&genesis_block);
            let _ = store.save_last_hash(&genesis_block.hash);
        }
    }

    pub fn last_block(&self) -> &Block {
        self.chain.last().expect("Chain should never be empty")
    }

    pub fn mine_pending_transactions(&mut self, _miner_address: String) {
        let index = self.chain.len() as u64;
        let previous_hash = self.chain.last().unwrap().hash.clone();

        let mut block = Block::new(index, previous_hash, self.pending_transactions.clone());

        println!("Mining block {}...", index);
        block.mine(self.difficulty);

        println!("✅ Block mined: {}", block.hash);

        if let Some(ref store) = self.storage {
            let _ = store.insert_block(&block);
            let _ = store.save_last_hash(&block.hash);
        }

        self.chain.push(block);
        self.pending_transactions = Vec::new();
    }

    pub fn add_transaction(&mut self, transaction: Transaction) {
        self.pending_transactions.push(transaction);
    }

    pub fn is_valid(&self) -> bool {
        for (i, block) in self.chain.iter().enumerate() {
            if i == 0 {
                if block.hash != block.calculate_hash() {
                    println!("❌ Genesis block hash invalid!");
                    return false;
                }
                continue;
            }
            let previous_block = &self.chain[i - 1];

            if block.previous_hash != previous_block.hash {
                println!("❌ Block {} previous hash invalid!", i);
                return false;
            }

            if block.hash != block.calculate_hash() {
                println!("❌ Block {} hash invalid!", i);
                return false;
            }

            let target = "0".repeat(self.difficulty);
            if !block.hash.starts_with(&target) {
                println!("❌ Block {} PoW invalid!", i);
                return false;
            }
        }
        true
    }

    pub fn is_valid_chain(&self, chain: &[Block]) -> bool {
        if chain.is_empty() {
            return false; // Boş zincir geçerli değil
        }

        if chain[0] != Block::genesis() {
            return false;
        }

        for i in 1..chain.len() {
            let current_block = &chain[i];
            let previous_block = &chain[i - 1];

            if current_block.previous_hash != previous_block.hash {
                return false;
            }

            if current_block.hash != current_block.calculate_hash() {
                return false;
            }

            let target = "0".repeat(self.difficulty);
            if !current_block.hash.starts_with(&target) {
                return false;
            }
        }
        true
    }

    pub fn print_info(&self) {
        println!("Blockchain Info:");
        println!("Length: {}", self.chain.len());
        println!("Difficulty: {}", self.difficulty);
        println!("Pending Tx: {}", self.pending_transactions.len());
        for block in &self.chain {
            println!(" - Block #{}: {}", block.index, block.hash);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_blockchain() {
        let mut blockchain = Blockchain::new(2, None);

        blockchain.add_transaction(Transaction::new(
            "alice".to_string(),
            "bob".to_string(),
            50,
            vec![],
        ));
        blockchain.mine_pending_transactions("miner1".to_string());

        assert!(blockchain.is_valid());
        assert_eq!(blockchain.chain.len(), 2);
    }
}
