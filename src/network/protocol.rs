use crate::{Block, Transaction};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum NetworkMessage {
    Block(Block),
    Transaction(Transaction),
    GetBlocks,
    Chain(Vec<Block>),
}

impl NetworkMessage {
    pub fn to_bytes(&self) -> Vec<u8> {
        serde_json::to_vec(self).unwrap()
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<Self, serde_json::Error> {
        serde_json::from_slice(bytes)
    }
}
