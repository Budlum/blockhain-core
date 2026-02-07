use crate::hash::calculate_hash;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Transaction {
    pub from: String,
    pub to: String,
    pub amount: u64,
    pub data: Vec<u8>,
    pub timestamp: u128,
    pub hash: String,
}

impl Transaction {
    pub fn new(from: String, to: String, amount: u64, data: Vec<u8>) -> Self {
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis();

        let mut tx = Transaction {
            from,
            to,
            amount,
            data,
            timestamp,
            hash: String::new(),
        };
        tx.hash = tx.calculate_hash();
        tx
    }

    pub fn genesis() -> Self {
        Transaction::new(
            "genesis".to_string(),
            "genesis".to_string(),
            0,
            b"Budlum Genesis Block".to_vec(),
        )
    }

    pub fn calculate_hash(&self) -> String {
        let data = format!(
            "{}{}{}{}{}",
            self.from,
            self.to,
            self.amount,
            hex::encode(&self.data),
            self.timestamp
        );
        calculate_hash(data.as_bytes())
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        serde_json::to_vec(self).unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_transaction_hash_consistency() {
        let tx = Transaction::new(
            "alice".to_string(),
            "bob".to_string(),
            100,
            b"test".to_vec(),
        );

        assert_eq!(tx.hash, tx.calculate_hash());
        assert!(!tx.hash.is_empty());
    }
}
