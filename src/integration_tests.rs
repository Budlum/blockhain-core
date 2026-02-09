#[cfg(test)]
mod integration_tests {
    use crate::account::{AccountState, Validator};
    use crate::block::Block;
    use crate::blockchain::Blockchain;
    use crate::consensus::poa::PoAConfig;
    use crate::consensus::pos::PoSConfig; 
    use crate::consensus::{ConsensusEngine, PoAEngine, PoSEngine, PoWEngine};
    use crate::crypto::KeyPair;
    use crate::transaction::Transaction;
    use std::sync::Arc;

    #[test]
    fn test_poa_rejects_unsigned_block() {
        let keypair = KeyPair::generate().unwrap();
        let validator_pubkey = keypair.public_key_hex();

        let mut state = AccountState::new();
        state.validators.insert(
            validator_pubkey.clone(),
            Validator::new(validator_pubkey.clone(), 0),
        );
        state.validators.get_mut(&validator_pubkey).unwrap().active = true;

        
        let config = PoAConfig::default();
        let engine = PoAEngine::new(config, Some(keypair));
        let mut block = Block::new(1, "0".repeat(64), vec![]);
        block.hash = block.calculate_hash();

        let result = engine.validate_block(&block, &[], &state);
        assert!(result.is_err(), "Unsigned block should be rejected in PoA");
    }

    #[test]
    fn test_poa_rejects_forged_signature() {
        let validator_keypair = KeyPair::generate().unwrap();
        let validator_pubkey = validator_keypair.public_key_hex();

        let mut state = AccountState::new();
        state.validators.insert(
            validator_pubkey.clone(),
            Validator::new(validator_pubkey.clone(), 0),
        );
        state.validators.get_mut(&validator_pubkey).unwrap().active = true;

        let config = PoAConfig::default();
        let engine = PoAEngine::new(config, Some(validator_keypair));

        let attacker_keypair = KeyPair::generate().unwrap();
        let mut block = Block::new(1, "0".repeat(64), vec![]);
        block.producer = Some(validator_pubkey);
        block.sign(&attacker_keypair);

        let result = engine.validate_block(&block, &[], &state);
        assert!(result.is_err(), "Forged signature should be rejected");
    }

    #[test]
    fn test_pos_requires_signature() {
        let keypair = KeyPair::generate().unwrap();
        let validator_pubkey = keypair.public_key_hex();

        let mut state = AccountState::new();
        state.add_balance(&validator_pubkey, 2000);
        let mut validator = Validator::new(validator_pubkey.clone(), 1000);
        validator.active = true;
        state.validators.insert(validator_pubkey.clone(), validator);

        let config = PoSConfig {
            min_stake: 100,
            ..Default::default()
        };
        let engine = PoSEngine::new(config, Some(keypair));

        let mut block = Block::new(1, "0".repeat(64), vec![]);
        block.producer = Some(validator_pubkey);
        block.hash = block.calculate_hash();

        let result = engine.validate_block(&block, &[], &state);
        assert!(result.is_err(), "PoS should reject unsigned blocks");
    }

    #[test]
    fn test_signed_transaction_flow() {
        let sender_keypair = KeyPair::generate().unwrap();
        let sender_pubkey = sender_keypair.public_key_hex();
        let consensus = Arc::new(PoWEngine::new(1));
        let mut blockchain = Blockchain::new(consensus, None, 1337, None);
        blockchain.init_genesis_account(&sender_pubkey);

        let mut tx = Transaction::new(sender_pubkey.clone(), "recipient".to_string(), 100, vec![]);
        tx.fee = 1;
        tx.nonce = 0;
        tx.sign(&sender_keypair);

        let result = blockchain.add_transaction(tx);
        assert!(result.is_ok(), "Signed TX with balance should be accepted");

        blockchain.produce_block("miner".to_string());
        assert!(blockchain.is_valid());
        assert_eq!(blockchain.chain.len(), 2);
    }

    #[test]
    fn test_unsigned_transaction_rejected() {
        let consensus = Arc::new(PoWEngine::new(1));
        let mut blockchain = Blockchain::new(consensus, None, 1337, None);
        let tx = Transaction::new("alice".to_string(), "bob".to_string(), 100, vec![]);
        let result = blockchain.add_transaction(tx);
        assert!(result.is_err(), "Unsigned TX should be rejected");
    }

    #[test]
    fn test_insufficient_balance_rejected() {
        let keypair = KeyPair::generate().unwrap();
        let pubkey = keypair.public_key_hex();
        let consensus = Arc::new(PoWEngine::new(1));
        let mut blockchain = Blockchain::new(consensus, None, 1337, None);

        let mut tx = Transaction::new(pubkey.clone(), "recipient".to_string(), 100, vec![]);
        tx.fee = 1;
        tx.nonce = 0;
        tx.sign(&keypair);

        let result = blockchain.add_transaction(tx);
        assert!(
            result.is_err(),
            "TX with insufficient balance should be rejected"
        );
    }

    #[test]
    fn test_replay_attack_protection() {
        let keypair = KeyPair::generate().unwrap();
        let pubkey = keypair.public_key_hex();
        let consensus = Arc::new(PoWEngine::new(1));
        let mut blockchain = Blockchain::new(consensus, None, 1337, None);
        blockchain.init_genesis_account(&pubkey);

        let mut tx1 = Transaction::new(pubkey.clone(), "recipient".to_string(), 10, vec![]);
        tx1.fee = 1;
        tx1.nonce = 0;
        tx1.sign(&keypair);

        blockchain.add_transaction(tx1.clone()).unwrap();
        blockchain.produce_block("miner".to_string());

        let result = blockchain.add_transaction(tx1);
        assert!(result.is_err(), "Replay attack should be prevented");
    }

    #[test]
    fn test_invalid_nonce_rejected() {
        let keypair = KeyPair::generate().unwrap();
        let pubkey = keypair.public_key_hex();
        let consensus = Arc::new(PoWEngine::new(1));
        let mut blockchain = Blockchain::new(consensus, None, 1337, None);
        blockchain.init_genesis_account(&pubkey);

        let mut tx = Transaction::new(pubkey.clone(), "recipient".to_string(), 10, vec![]);
        tx.fee = 1;
        tx.nonce = 1; 
        tx.sign(&keypair);

        let result = blockchain.add_transaction(tx);
        assert!(result.is_err(), "TX with invalid nonce should be rejected");
    }

    #[test]
    fn test_block_signature_verification() {
        let keypair = KeyPair::generate().unwrap();
        let pubkey = keypair.public_key_hex();
        let mut block = Block::new(1, "0".repeat(64), vec![]);
        block.sign(&keypair);

        assert_eq!(block.producer.as_ref().unwrap(), &pubkey);
        assert!(block.verify_signature());

        block.transactions.push(Transaction::new(
            "attacker".to_string(),
            "attacker".to_string(),
            1000000,
            vec![],
        ));
        block.tx_root = block.calculate_tx_root();
        block.hash = block.calculate_hash();

        assert!(
            !block.verify_signature(),
            "Signature for old hash should fail verification"
        );
    }

    #[test]
    fn test_poa_round_robin_signed() {
        let keypair1 = KeyPair::generate().unwrap();
        let keypair2 = KeyPair::generate().unwrap();
        let pubkey1 = keypair1.public_key_hex();
        let pubkey2 = keypair2.public_key_hex();

        let mut state = AccountState::new();
        state
            .validators
            .insert(pubkey1.clone(), Validator::new(pubkey1.clone(), 0));
        state
            .validators
            .insert(pubkey2.clone(), Validator::new(pubkey2.clone(), 0));
        state.validators.get_mut(&pubkey1).unwrap().active = true;
        state.validators.get_mut(&pubkey2).unwrap().active = true;

        let config = PoAConfig {
            quorum_ratio: 0.66,
            block_period: 5,
            ..PoAConfig::default()
        };
        
        

        let engine = PoAEngine::new(config, Some(keypair1));

        let validators = state.get_active_validators();

        if validators.len() < 2 {
            return;
        }

        let expected = engine.expected_proposer(0, &validators).unwrap();

        assert!(state.validators.contains_key(&expected.address));

        let mut block = Block::new(0, "0".repeat(64), vec![]);

        let mut my_slot = 0;
        if expected.address != pubkey1 {
            my_slot = 1;
        }
        block.index = my_slot;

        let expected_my_slot = engine.expected_proposer(my_slot, &validators).unwrap();

        if expected_my_slot.address == pubkey1 {
            let result = engine.prepare_block(&mut block, &state);
            assert!(result.is_ok());
            assert!(block.signature.is_some());
        }
    }
}
