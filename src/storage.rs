use crate::Block;
use sled::Db;
use std::str::from_utf8;

#[derive(Clone, Debug)]
pub struct Storage {
    db: Db,
}

impl Storage {
    pub fn new(path: &str) -> std::io::Result<Self> {
        let db = sled::open(path)?;
        Ok(Storage { db })
    }

    pub fn insert_block(&self, block: &Block) -> std::io::Result<()> {
        let key = block.hash.clone();
        let val = serde_json::to_vec(block)?;
        self.db.insert(key, val)?;
        self.db.flush()?;
        Ok(())
    }

    pub fn get_block(&self, hash: &str) -> std::io::Result<Option<Block>> {
        if let Some(val) = self.db.get(hash)? {
            let block: Block = serde_json::from_slice(&val)?;
            Ok(Some(block))
        } else {
            Ok(None)
        }
    }

    pub fn save_last_hash(&self, hash: &str) -> std::io::Result<()> {
        self.db.insert("LAST", hash.as_bytes())?;
        self.db.flush()?;
        Ok(())
    }

    pub fn get_last_hash(&self) -> std::io::Result<Option<String>> {
        if let Some(val) = self.db.get("LAST")? {
            let hash = from_utf8(&val).unwrap().to_string();
            Ok(Some(hash))
        } else {
            Ok(None)
        }
    }
}
