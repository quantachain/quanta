use sled::Db;
use crate::block::Block;
use crate::transaction::UTXOSet;
use std::path::Path;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum StorageError {
    #[error("Database error: {0}")]
    Database(#[from] sled::Error),
    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
    #[error("Block not found: {0}")]
    BlockNotFound(u64),
}

/// Persistent storage for blockchain data
pub struct BlockchainStorage {
    db: Db,
}

impl BlockchainStorage {
    /// Open or create blockchain database
    pub fn new<P: AsRef<Path>>(path: P) -> Result<Self, StorageError> {
        let db = sled::open(path)?;
        tracing::info!("Blockchain database opened");
        Ok(Self { db })
    }

    /// Save a block to disk
    pub fn save_block(&self, block: &Block) -> Result<(), StorageError> {
        let key = format!("block:{}", block.index);
        let value = serde_json::to_vec(block)?;
        self.db.insert(key.as_bytes(), value)?;
        self.db.flush()?;
        tracing::debug!("Block {} saved to database", block.index);
        Ok(())
    }

    /// Load a block from disk
    pub fn load_block(&self, index: u64) -> Result<Block, StorageError> {
        let key = format!("block:{}", index);
        let value = self.db.get(key.as_bytes())?
            .ok_or(StorageError::BlockNotFound(index))?;
        let block: Block = serde_json::from_slice(&value)?;
        Ok(block)
    }

    /// Get the height of the blockchain (number of blocks)
    pub fn get_chain_height(&self) -> Result<u64, StorageError> {
        let height_key = b"chain_height";
        if let Some(value) = self.db.get(height_key)? {
            let height_bytes: [u8; 8] = value.as_ref().try_into()
                .map_err(|_| StorageError::Database(sled::Error::Unsupported("Invalid height data".into())))?;
            Ok(u64::from_be_bytes(height_bytes))
        } else {
            Ok(0)
        }
    }

    /// Update the chain height
    pub fn set_chain_height(&self, height: u64) -> Result<(), StorageError> {
        let height_key = b"chain_height";
        self.db.insert(height_key, &height.to_be_bytes())?;
        Ok(())
    }

    /// Save UTXO set
    pub fn save_utxo_set(&self, utxo_set: &UTXOSet) -> Result<(), StorageError> {
        let key = b"utxo_set";
        let value = serde_json::to_vec(utxo_set)?;
        self.db.insert(key, value)?;
        self.db.flush()?;
        tracing::debug!("UTXO set saved to database");
        Ok(())
    }

    /// Load UTXO set
    pub fn load_utxo_set(&self) -> Result<Option<UTXOSet>, StorageError> {
        let key = b"utxo_set";
        if let Some(value) = self.db.get(key)? {
            let utxo_set: UTXOSet = serde_json::from_slice(&value)?;
            Ok(Some(utxo_set))
        } else {
            Ok(None)
        }
    }

    /// Load entire blockchain from disk
    pub fn load_chain(&self) -> Result<Vec<Block>, StorageError> {
        let height = self.get_chain_height()?;
        let mut chain = Vec::new();
        
        for i in 0..height {
            match self.load_block(i) {
                Ok(block) => chain.push(block),
                Err(e) => {
                    tracing::warn!("Failed to load block {}: {}", i, e);
                    break;
                }
            }
        }
        
        tracing::info!("Loaded {} blocks from database", chain.len());
        Ok(chain)
    }

    /// Clear all data (use with caution!)
    pub fn clear(&self) -> Result<(), StorageError> {
        self.db.clear()?;
        self.db.flush()?;
        tracing::warn!("Database cleared");
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_storage_persistence() {
        let temp_dir = TempDir::new().unwrap();
        let storage = BlockchainStorage::new(temp_dir.path()).unwrap();
        
        let block = Block::genesis();
        storage.save_block(&block).unwrap();
        storage.set_chain_height(1).unwrap();
        
        let loaded_block = storage.load_block(0).unwrap();
        assert_eq!(loaded_block.index, block.index);
        
        let height = storage.get_chain_height().unwrap();
        assert_eq!(height, 1);
    }
}
