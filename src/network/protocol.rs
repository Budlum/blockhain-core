use crate::encoding::PROTOCOL_VERSION_MAJOR;
use crate::{Block, BlockHeader, Transaction};
use serde::{Deserialize, Serialize};

pub const MAX_MESSAGE_SIZE: usize = 10 * 1024 * 1024;
pub const MAX_BLOCK_SIZE: usize = 1 * 1024 * 1024;
pub const MAX_TX_SIZE: usize = 100 * 1024;
pub const MAX_CHAIN_SYNC_BLOCKS: usize = 500;
pub const MAX_HEADERS_PER_REQUEST: u32 = 2000;

#[derive(Debug, Clone, PartialEq)]
pub enum MessageError {
    TooLarge(usize),
    ParseError(String),
    VersionMismatch { expected: u32, got: u32 },
}

pub const MAX_SNAP_BATCH: u64 = 256;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum NetworkMessage {
    Handshake {
        version_major: u32,
        version_minor: u32,
        chain_id: u64,
        best_height: u64,
    },
    HandshakeAck {
        version_major: u32,
        version_minor: u32,
        chain_id: u64,
        best_height: u64,
    },

    Block(Block),
    Transaction(Transaction),

    GetHeaders {
        locator: Vec<String>,
        limit: u32,
    },

    Headers(Vec<BlockHeader>),

    GetBlocksRange {
        from: u64,
        to: u64,
    },

    Blocks(Vec<Block>),

    GetBlocksByHeight {
        from_height: u64,
        to_height: u64,
    },

    BlocksByHeight(Vec<Block>),

    StateSnapshotResponse {
        height: u64,
        state_root: String,
        ok: bool,
    },

    NewTip {
        height: u64,
        hash: String,
    },

    GetStateSnapshot {
        height: u64,
    },

    SnapshotChunk {
        height: u64,
        index: u32,
        total: u32,
        data: Vec<u8>,
    },
}
impl NetworkMessage {
    pub fn to_bytes(&self) -> Vec<u8> {
        serde_json::to_vec(self).unwrap()
    }
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, serde_json::Error> {
        serde_json::from_slice(bytes)
    }
    pub fn from_bytes_validated(bytes: &[u8]) -> Result<Self, MessageError> {
        if bytes.len() > MAX_MESSAGE_SIZE {
            return Err(MessageError::TooLarge(bytes.len()));
        }
        serde_json::from_slice(bytes).map_err(|e| MessageError::ParseError(e.to_string()))
    }
    pub fn validate_block_size(block: &Block) -> Result<(), MessageError> {
        let size = serde_json::to_vec(block).unwrap_or_default().len();
        if size > MAX_BLOCK_SIZE {
            return Err(MessageError::TooLarge(size));
        }
        Ok(())
    }
    pub fn validate_tx_size(tx: &Transaction) -> Result<(), MessageError> {
        let size = serde_json::to_vec(tx).unwrap_or_default().len();
        if size > MAX_TX_SIZE {
            return Err(MessageError::TooLarge(size));
        }
        Ok(())
    }
}
