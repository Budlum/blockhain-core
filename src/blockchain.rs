use crate::account::AccountState;
use crate::consensus::ConsensusEngine;
use crate::genesis::{GenesisConfig, GENESIS_TIMESTAMP};
use crate::mempool::{Mempool, MempoolConfig};
use crate::snapshot::PruningManager;
use crate::storage::Storage;
use crate::{Block, Transaction};
use std::sync::Arc;

pub const MAX_REORG_DEPTH: usize = 100;
pub const FINALITY_DEPTH: usize = 50;
pub const EPOCH_LENGTH: u64 = 32;

pub struct Blockchain {
    pub chain: Vec<Block>,
    pub consensus: Arc<dyn ConsensusEngine>,
    pub mempool: Mempool,
    pub storage: Option<Storage>,
    pub state: AccountState,
    pub chain_id: u64,
    pub pruning_manager: Option<PruningManager>,
}
impl Blockchain {
    pub fn new(
        consensus: Arc<dyn ConsensusEngine>,
        storage: Option<Storage>,
        chain_id: u64,
        pruning_manager: Option<PruningManager>,
    ) -> Self {
        println!("Consensus: {}", consensus.info());
        let mut chain_vec = Vec::new();
        let mut state = AccountState::new();

        let mut loaded_chain = false;
        if let Some(ref store) = storage {
            if let Ok(c) = store.load_chain() {
                if !c.is_empty() {
                    chain_vec = c;
                    loaded_chain = true;
                    println!("📚 Loaded chain from DB: {} blocks", chain_vec.len());
                }
            }
        }

        if !loaded_chain {
            let genesis = GenesisConfig::new(chain_id).build_genesis_block();
            chain_vec.push(genesis);
        }

        let mut snapshot_height = 0;
        if let Some(ref pm) = pruning_manager {
            if let Ok(Some(snapshot)) = pm.load_latest_snapshot() {
                if snapshot.chain_id == chain_id {
                    for (addr, balance) in &snapshot.balances {
                        let acc = state.get_or_create(addr);
                        acc.balance = *balance;
                    }
                    for (addr, nonce) in &snapshot.nonces {
                        let acc = state.get_or_create(addr);
                        acc.nonce = *nonce;
                    }
                    snapshot_height = snapshot.height;
                    println!(
                        "✅ Restored state from snapshot at height {}",
                        snapshot_height
                    );
                } else {
                    println!(
                        "⚠️  Snapshot chain_id mismatch (expected {}, got {}). Ignoring.",
                        chain_id, snapshot.chain_id
                    );
                }
            }
        }

        let chain_len = chain_vec.len();
        let start_index = if snapshot_height > 0 && snapshot_height < chain_len as u64 {
            (snapshot_height + 1) as usize
        } else {
            if snapshot_height >= chain_len as u64 {
                println!("⚠️  Chain shorter than snapshot height! Replaying from Genesis.");
                0
            } else {
                0
            }
        };

        println!(
            "🔄 Replaying blocks from index {} to {}...",
            start_index,
            chain_len - 1
        );

        for (i, block) in chain_vec.iter().enumerate().skip(start_index) {
            if i == 0 {}

            for tx in &block.transactions {
                if let Err(e) = state.apply_transaction(tx) {
                    println!("❌ Error applying block {} tx: {}", block.index, e);
                }
            }

            let total_fees: u64 = block.transactions.iter().map(|t| t.fee).sum();
            if let Some(ref producer) = block.producer {
                if total_fees > 0 {
                    let acc = state.get_or_create(producer);
                    acc.balance += total_fees;
                }
            }
        }

        Blockchain {
            chain: chain_vec,
            consensus,
            mempool: Mempool::new(MempoolConfig::default()),
            storage,
            state,
            chain_id,
            pruning_manager,
        }
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
        println!("Loaded {} blocks from disk", self.chain.len());
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
    pub fn produce_block(&mut self, producer_address: String) {
        let index = self.chain.len() as u64;
        let previous_hash = self.chain.last().unwrap().hash.clone();

        let mut valid_txs = Vec::new();
        let mut temp_state = self.state.clone();

        let pending_txs = self.mempool.get_sorted_transactions(1000);
        for tx in &pending_txs {
            if let Ok(_) = temp_state.validate_transaction(tx) {
                if let Ok(_) = temp_state.apply_transaction(tx) {
                    valid_txs.push(tx.clone());
                }
            } else {
                println!("Discarding invalid transaction: {}", tx.hash);
            }
        }

        let mut block = Block::new(index, previous_hash, valid_txs);
        println!(
            "Producing block {} with {} ({} txs)...",
            index,
            self.consensus.consensus_type(),
            block.transactions.len()
        );

        block.producer = Some(producer_address.clone());

        if let Err(e) = self.consensus.prepare_block(&mut block, &self.state) {
            println!("Block preparation failed: {}", e);
            return;
        }

        println!("Block produced: {}", block.hash);
        if let Some(ref store) = self.storage {
            let _ = store.insert_block(&block);
            let _ = store.save_last_hash(&block.hash);
        }

        for tx in &block.transactions {
            let _ = self.state.apply_transaction(tx);
        }

        if block.index > 0 && block.index % EPOCH_LENGTH == 0 {
            self.state.advance_epoch(block.timestamp);
        }

        self.chain.push(block.clone());

        for tx in &block.transactions {
            self.mempool.remove_transaction(&tx.hash);
        }
    }
    pub fn mine_pending_transactions(&mut self, miner_address: String) {
        self.produce_block(miner_address);
    }
    pub fn add_transaction(&mut self, transaction: Transaction) -> Result<(), String> {
        if transaction.chain_id != self.chain_id {
            return Err(format!(
                "Invalid Chain ID: expected {}, got {}",
                self.chain_id, transaction.chain_id
            ));
        }
        if let Err(e) = self.state.validate_transaction(&transaction) {
            return Err(format!("Invalid transaction: {}", e));
        }

        self.mempool
            .add_transaction(transaction)
            .map_err(|e| format!("Mempool error: {:?}", e))
    }

    pub fn init_genesis_account(&mut self, address: &str) {
        self.state.add_balance(address, 1_000_000_000);
    }

    pub fn validate_and_add_block(&mut self, block: Block) -> Result<(), String> {
        if block.chain_id != self.chain_id {
            return Err(format!(
                "Invalid Chain ID: expected {}, got {}",
                self.chain_id, block.chain_id
            ));
        }

        if let Err(e) = self
            .consensus
            .validate_block(&block, &self.chain, &self.state)
        {
            return Err(format!("Consensus validation failed: {}", e));
        }

        let mut temp_state = self.state.clone();
        for (i, tx) in block.transactions.iter().enumerate() {
            if let Err(e) = temp_state.apply_transaction(tx) {
                return Err(format!("Invalid transaction at index {}: {}", i, e));
            }
        }

        if !block.state_root.is_empty() {
            let computed_root = temp_state.calculate_state_root();
            if computed_root != block.state_root {
                return Err(format!(
                    "State root mismatch: expected {}, got {}",
                    block.state_root, computed_root
                ));
            }
        }

        if let Some(ref store) = self.storage {
            let _ = store.insert_block(&block);
            let _ = store.save_last_hash(&block.hash);
        }

        if let Some(evidences) = &block.slashing_evidence {
            let slash_ratio = 0.1;
            temp_state.apply_slashing(evidences, slash_ratio);
        }

        if block.index > 0 && block.index % EPOCH_LENGTH == 0 {
            temp_state.advance_epoch(block.timestamp);
        }

        self.state = temp_state;

        self.chain.push(block);

        if let Err(e) = self.consensus.record_block(self.chain.last().unwrap()) {
            println!("Engine record block error: {}", e);
        }

        for tx in self.chain.last().unwrap().transactions.iter() {
            self.mempool.remove_transaction(&tx.hash);
        }

        Ok(())
    }

    pub fn is_valid(&self) -> bool {
        for i in 0..self.chain.len() {
            let block = &self.chain[i];
            let previous_chain = &self.chain[..i];
            let dummy_state = AccountState::new();
            if let Err(e) = self
                .consensus
                .validate_block(block, previous_chain, &dummy_state)
            {
                println!("Block {} validation failed: {}", i, e);
                return false;
            }
        }
        true
    }
    pub fn is_valid_chain(&self, chain: &[Block]) -> bool {
        if chain.is_empty() {
            return false;
        }

        let genesis = &chain[0];
        if genesis.index != 0
            || genesis.previous_hash != "0".repeat(64)
            || genesis.timestamp != GENESIS_TIMESTAMP
            || genesis.hash != genesis.calculate_hash()
            || genesis.chain_id != self.chain_id
        {
            return false;
        }

        for i in 0..chain.len() {
            let block = &chain[i];
            let previous_chain = &chain[..i];
            let dummy_state = AccountState::new();
            if let Err(_) = self
                .consensus
                .validate_block(block, previous_chain, &dummy_state)
            {
                return false;
            }
        }
        true
    }
    pub fn find_fork_point(&self, other_chain: &[Block]) -> Option<usize> {
        for (i, block) in self.chain.iter().enumerate() {
            if i >= other_chain.len() {
                return None;
            }
            if block.hash != other_chain[i].hash {
                return Some(i);
            }
        }
        None
    }
    pub fn try_reorg(&mut self, new_chain: Vec<Block>) -> Result<bool, String> {
        if !self.consensus.is_better_chain(&self.chain, &new_chain) {
            return Ok(false);
        }
        if !self.is_valid_chain(&new_chain) {
            return Err("Invalid chain".to_string());
        }

        let fork_point = self.find_fork_point(&new_chain).unwrap_or(0);
        let reorg_depth = self.chain.len().saturating_sub(fork_point);

        if reorg_depth > MAX_REORG_DEPTH {
            return Err(format!(
                "Reorg depth {} exceeds max {}",
                reorg_depth, MAX_REORG_DEPTH
            ));
        }

        let finalized_height = self.chain.len().saturating_sub(FINALITY_DEPTH);
        if fork_point < finalized_height {
            return Err("Cannot reorg past finality depth".to_string());
        }

        println!(
            "Reorg: replacing {} blocks from height {}",
            reorg_depth, fork_point
        );

        let new_state = Blockchain::rebuild_state(&new_chain)?;

        self.chain = new_chain;
        self.state = new_state;

        let mut new_pending = Vec::new();

        let mut chain_txs = std::collections::HashSet::new();
        for block in &self.chain {
            for tx in &block.transactions {
                chain_txs.insert(tx.hash.clone());
            }
        }

        for tx in &self.mempool.get_sorted_transactions(1000) {
            if !chain_txs.contains(&tx.hash) {
                if self.state.validate_transaction(tx).is_ok() {
                    new_pending.push(tx.clone());
                }
            }
        }

        self.mempool = Mempool::new(MempoolConfig::default());
        for tx in new_pending {
            let _ = self.mempool.add_transaction(tx);
        }

        if let Some(ref store) = self.storage {
            if let Some(last) = self.chain.last() {
                let _ = store.save_last_hash(&last.hash);
            }
            for block in &self.chain[fork_point..] {
                let _ = store.insert_block(block);
            }
        }

        Ok(true)
    }

    fn rebuild_state(chain: &[Block]) -> Result<AccountState, String> {
        let mut state = AccountState::new();

        for block in chain.iter() {
            for (tx_idx, tx) in block.transactions.iter().enumerate() {
                if tx.from == "genesis" {
                    continue;
                }
                if let Err(e) = state.apply_transaction(tx) {
                    return Err(format!(
                        "State replay failed at block {} tx {}: {}",
                        block.index, tx_idx, e
                    ));
                }
            }

            let mut total_fees: u64 = 0;
            for tx in &block.transactions {
                if tx.from != "genesis" {
                    total_fees += tx.fee;
                }
            }
            if let Some(producer) = &block.producer {
                if total_fees > 0 {
                    let producer_account = state.get_or_create(producer);
                    producer_account.balance += total_fees;
                }
            }
        }
        Ok(state)
    }
    pub fn print_info(&self) {
        println!("================================");
        println!("Blockchain Info");
        println!("================================");
        println!("Consensus: {}", self.consensus.info());
        println!("Length: {}", self.chain.len());
        println!("Pending Tx: {}", self.mempool.len());
        println!("================================");
        for block in &self.chain {
            println!(" Block #{}: {}", block.index, &block.hash[..16]);
        }
    }
    pub fn consensus(&self) -> &dyn ConsensusEngine {
        self.consensus.as_ref()
    }
}
impl Clone for Blockchain {
    fn clone(&self) -> Self {
        Blockchain {
            chain: self.chain.clone(),
            consensus: Arc::clone(&self.consensus),
            mempool: Mempool::default(),
            storage: self.storage.clone(),
            state: self.state.clone(),
            chain_id: self.chain_id,
            pruning_manager: self.pruning_manager.clone(),
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::consensus::PoWEngine;
    use crate::crypto::KeyPair;

    #[test]
    fn test_blockchain_with_pow() {
        let consensus = Arc::new(PoWEngine::new(1));
        let mut blockchain = Blockchain::new(consensus, None, 1337, None);

        let keypair = KeyPair::generate().unwrap();
        let pubkey = keypair.public_key_hex();

        blockchain.state.add_balance(&pubkey, 100);

        let mut tx = Transaction::new(pubkey.clone(), "bob".to_string(), 50, vec![]);
        tx.fee = 1;
        tx.sign(&keypair);

        blockchain.add_transaction(tx).unwrap();

        blockchain.produce_block("miner1".to_string());
        assert!(blockchain.is_valid());
        assert_eq!(blockchain.chain.len(), 2);
    }

    #[test]
    fn test_epoch_transition_and_unjailing() {
        let consensus = Arc::new(PoWEngine::new(1));
        let mut blockchain = Blockchain::new(consensus, None, 1337, None);

        
        let validator_addr = "validator1".to_string();
        blockchain.state.add_validator(validator_addr.clone(), 1000);

        
        if let Some(v) = blockchain.state.get_validator_mut(&validator_addr) {
            v.jailed = true;
            v.active = false;
            v.jail_until = 0; 
        }

        
        assert_eq!(blockchain.state.epoch_index, 0);
        if let Some(v) = blockchain.state.get_validator(&validator_addr) {
            assert!(v.jailed);
        }

        
        
        
        for _ in 0..EPOCH_LENGTH {
            blockchain.produce_block("miner".to_string());
        }

        
        assert_eq!(blockchain.chain.len(), (EPOCH_LENGTH as usize) + 1);

        
        assert_eq!(blockchain.state.epoch_index, 1);

        
        if let Some(v) = blockchain.state.get_validator(&validator_addr) {
            assert!(!v.jailed, "Validator should have been unjailed");
            assert!(v.active, "Validator should be active");
        } else {
            panic!("Validator not found");
        }
    }

    #[test]
    fn test_slashing_execution() {
        use crate::block::BlockHeader;
        use crate::consensus::pos::{PoSConfig, SlashingEvidence};
        use crate::consensus::PoSEngine;

        
        let alice_key = KeyPair::generate().unwrap();
        let alice_pub = alice_key.public_key_hex();

        
        let mut config = PoSConfig::default();
        config.slashing_penalty = 0.50; 

        let engine = Arc::new(PoSEngine::new(config, Some(alice_key.clone()))); 

        let mut blockchain = Blockchain::new(engine.clone(), None, 1337, None);

        
        blockchain.state.add_validator(alice_pub.clone(), 2000);
        blockchain.state.add_balance(&alice_pub, 100);

        
        let mut real_b1 = Block::new(10, "prev".into(), vec![]);
        real_b1.producer = Some(alice_pub.clone());
        real_b1.hash = real_b1.calculate_hash();
        let sig1 = alice_key.sign(real_b1.hash.as_bytes()).to_vec();
        real_b1.signature = Some(sig1.clone());
        let h1 = BlockHeader::from_block(&real_b1);

        let mut real_b2 = Block::new(10, "prev".into(), vec![]);
        real_b2.timestamp += 1; 
        real_b2.producer = Some(alice_pub.clone());
        real_b2.hash = real_b2.calculate_hash();
        let sig2 = alice_key.sign(real_b2.hash.as_bytes()).to_vec();
        real_b2.signature = Some(sig2.clone());
        let h2 = BlockHeader::from_block(&real_b2);

        let evidence = SlashingEvidence::new(h1, h2, sig1, sig2);

        
        {
            let mut guard = engine.slashing_evidence.write().unwrap();
            guard.push(evidence);
        }

        
        
        blockchain.produce_block(alice_pub.clone());

        let produced_block = blockchain.chain.last().unwrap();
        assert!(
            produced_block.slashing_evidence.is_some(),
            "Block should contain slashing evidence"
        );
        assert_eq!(produced_block.slashing_evidence.as_ref().unwrap().len(), 1);

        
        
        
        
        
        
        
        
        
        
        
        

        
        

        let mut blockchain2 = Blockchain::new(engine.clone(), None, 1337, None);
        blockchain2.state.add_validator(alice_pub.clone(), 2000);
        blockchain2
            .validate_and_add_block(produced_block.clone())
            .unwrap();

        let validator = blockchain2.state.get_validator(&alice_pub).unwrap();
        assert!(validator.slashed, "Validator should be slashed");
        assert!(!validator.active);
        assert!(validator.stake < 2000);
    }
}
