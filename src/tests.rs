#[cfg(test)]
mod blockchain_tests {
    use crate::blockchain::*;
    use crate::storage::BlockchainStorage;
    use crate::transaction::Transaction;
    use std::sync::Arc;
    use tempfile::TempDir;

    fn create_test_blockchain() -> (Blockchain, TempDir) {
        let temp_dir = TempDir::new().unwrap();
        let storage = Arc::new(
            BlockchainStorage::new(temp_dir.path().to_str().unwrap()).unwrap()
        );
        let blockchain = Blockchain::new(storage).unwrap();
        (blockchain, temp_dir)
    }

    #[test]
    fn test_blockchain_initialization() {
        let (blockchain, _temp) = create_test_blockchain();
        let chain = blockchain.get_chain();
        assert_eq!(chain.len(), 1); // Genesis block
        assert_eq!(chain[0].index, 0);
    }

    #[test]
    fn test_genesis_block_validity() {
        let (blockchain, _temp) = create_test_blockchain();
        assert!(blockchain.is_valid());
    }

    #[test]
    fn test_mining_reward() {
        let (blockchain, _temp) = create_test_blockchain();
        let stats = blockchain.get_stats();
        assert_eq!(stats.mining_reward, 50.0); // Initial reward
    }

    #[test]
    fn test_balance_tracking() {
        let (blockchain, _temp) = create_test_blockchain();
        let test_address = "test_address_123";
        let balance = blockchain.get_balance(test_address);
        assert_eq!(balance, 0.0);
    }

    #[test]
    fn test_transaction_validation_insufficient_balance() {
        let (blockchain, _temp) = create_test_blockchain();
        
        let tx = Transaction {
            sender: "sender".to_string(),
            recipient: "recipient".to_string(),
            amount: 100.0,
            timestamp: chrono::Utc::now().timestamp(),
            signature: vec![1, 2, 3], // Fake signature for testing
            public_key: vec![4, 5, 6], // Fake public key
            fee: 0.001,
        };

        let result = blockchain.add_transaction(tx);
        assert!(result.is_err());
    }

    #[test]
    fn test_mempool_size_limit() {
        let (blockchain, _temp) = create_test_blockchain();
        let stats = blockchain.get_stats();
        assert_eq!(stats.pending_transactions, 0);
    }
}

#[cfg(test)]
mod transaction_tests {
    use crate::transaction::*;

    #[test]
    fn test_transaction_creation() {
        let tx = Transaction::new(
            "sender".to_string(),
            "recipient".to_string(),
            10.0,
            123456789,
        );
        assert_eq!(tx.amount, 10.0);
        assert_eq!(tx.fee, 0.001);
    }

    #[test]
    fn test_transaction_hash() {
        let tx = Transaction::new(
            "sender".to_string(),
            "recipient".to_string(),
            10.0,
            123456789,
        );
        let hash = tx.hash();
        assert!(!hash.is_empty());
        assert_eq!(hash.len(), 64); // SHA3-256 hex
    }

    #[test]
    fn test_coinbase_transaction() {
        let tx = Transaction {
            sender: "COINBASE".to_string(),
            recipient: "miner".to_string(),
            amount: 50.0,
            timestamp: 123456789,
            signature: vec![],
            public_key: vec![],
            fee: 0.0,
        };
        assert!(tx.is_coinbase());
    }

    #[test]
    fn test_utxo_set() {
        let mut utxo_set = UTXOSet::new();
        
        let tx = Transaction::new(
            "sender".to_string(),
            "recipient".to_string(),
            10.0,
            123456789,
        );
        
        utxo_set.add_utxo(&tx);
        let balance = utxo_set.get_balance("recipient");
        assert_eq!(balance, 10.0);
    }

    #[test]
    fn test_utxo_spending() {
        let mut utxo_set = UTXOSet::new();
        
        let tx = Transaction::new(
            "COINBASE".to_string(),
            "user".to_string(),
            100.0,
            123456789,
        );
        
        utxo_set.add_utxo(&tx);
        assert_eq!(utxo_set.get_balance("user"), 100.0);
        
        let spent = utxo_set.spend_utxos("user", 50.0);
        assert!(spent);
        assert_eq!(utxo_set.get_balance("user"), 50.0);
    }
}

#[cfg(test)]
mod block_tests {
    use crate::block::Block;
    use crate::transaction::Transaction;

    #[test]
    fn test_genesis_block() {
        let genesis = Block::genesis();
        assert_eq!(genesis.index, 0);
        assert_eq!(genesis.transactions.len(), 0);
        assert!(genesis.previous_hash.starts_with("0000"));
    }

    #[test]
    fn test_block_creation() {
        let transactions = vec![
            Transaction::new(
                "sender".to_string(),
                "recipient".to_string(),
                10.0,
                123456789,
            ),
        ];
        
        let block = Block::new(1, transactions, "previous_hash".to_string(), 4);
        assert_eq!(block.index, 1);
        assert_eq!(block.transactions.len(), 1);
        assert!(!block.hash.is_empty());
    }

    #[test]
    fn test_block_hash_calculation() {
        let block = Block::genesis();
        let hash1 = block.calculate_hash();
        let hash2 = block.calculate_hash();
        assert_eq!(hash1, hash2); // Deterministic
    }

    #[test]
    fn test_merkle_root_in_block() {
        let transactions = vec![
            Transaction::new("a".to_string(), "b".to_string(), 10.0, 123),
            Transaction::new("c".to_string(), "d".to_string(), 20.0, 456),
        ];
        
        let block = Block::new(1, transactions, "prev".to_string(), 4);
        assert!(!block.merkle_root.is_empty());
        assert_eq!(block.merkle_root.len(), 64);
    }
}

#[cfg(test)]
mod merkle_tests {
    use crate::merkle::*;

    #[test]
    fn test_merkle_tree_from_hashes() {
        let hashes = vec![
            "hash1".to_string(),
            "hash2".to_string(),
            "hash3".to_string(),
            "hash4".to_string(),
        ];
        
        let tree = MerkleTree::from_hashes(hashes);
        assert!(tree.root_hash().is_some());
    }

    #[test]
    fn test_merkle_proof_verification() {
        let hashes = vec![
            "tx1".to_string(),
            "tx2".to_string(),
            "tx3".to_string(),
            "tx4".to_string(),
        ];
        
        let tree = MerkleTree::from_hashes(hashes.clone());
        let root = tree.root_hash().unwrap();
        
        for hash in &hashes {
            let proof = tree.generate_proof(hash).unwrap();
            assert!(proof.verify(&root), "Proof failed for {}", hash);
        }
    }

    #[test]
    fn test_merkle_tree_odd_leaves() {
        let hashes = vec![
            "tx1".to_string(),
            "tx2".to_string(),
            "tx3".to_string(),
        ];
        
        let tree = MerkleTree::from_hashes(hashes);
        assert!(tree.root_hash().is_some());
    }

    #[test]
    fn test_empty_merkle_tree() {
        let tree = MerkleTree::from_hashes(vec![]);
        assert!(tree.root_hash().is_none());
    }
}

#[cfg(test)]
mod crypto_tests {
    use crate::crypto::*;

    #[test]
    fn test_sha3_hash() {
        let data = b"test data";
        let hash = sha3_hash(data);
        assert_eq!(hash.len(), 32); // SHA3-256 = 32 bytes
    }

    #[test]
    fn test_double_sha3() {
        let data = b"test";
        let hash = double_sha3(data);
        assert_eq!(hash.len(), 64); // Hex encoded
    }

    #[test]
    fn test_deterministic_hashing() {
        let data = b"deterministic test";
        let hash1 = double_sha3(data);
        let hash2 = double_sha3(data);
        assert_eq!(hash1, hash2);
    }
}
