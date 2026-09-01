use crate::consensus::blockchain::{Blockchain, BlockchainError, AddressTransaction};
use crate::consensus::blockchain::BlockchainStats;
use crate::core::block::Block;
use crate::core::transaction::{AccountState, Transaction, AccountBalance};
use tokio::sync::{mpsc, oneshot, watch};
use thiserror::Error;

/// Messages sent to the Blockchain Actor.
pub enum BlockchainMessage {
    SubscribeNewBlocks {
        respond_to: oneshot::Sender<watch::Receiver<u64>>,
    },
    GetLatestBlock {
        respond_to: oneshot::Sender<Block>,
    },
    AddTransaction {
        transaction: Transaction,
        respond_to: oneshot::Sender<Result<(), BlockchainError>>,
    },
    CreateBlockTemplate {
        miner_address: String,
        respond_to: oneshot::Sender<Result<Block, BlockchainError>>,
    },
    CreateBftBlockTemplate {
        next_height: u64,
        proposer: String,
        epoch: u64,
        bft_round: u32,
        respond_to: oneshot::Sender<Result<Block, BlockchainError>>,
    },
    GetAccountStateSnapshot {
        respond_to: oneshot::Sender<AccountState>,
    },
    GetBlockByIndex {
        index: u64,
        respond_to: oneshot::Sender<Option<Block>>,
    },
    IsValid {
        respond_to: oneshot::Sender<bool>,
    },
    GetStats {
        respond_to: oneshot::Sender<BlockchainStats>,
    },
    GetBalance {
        address: String,
        respond_to: oneshot::Sender<u64>,
    },
    CumulativeWorkAt {
        tip_height: u64,
        respond_to: oneshot::Sender<u128>,
    },
    FlushStorage {
        respond_to: oneshot::Sender<()>,
    },
    LoadAccountStateAtHeight {
        height: u64,
        respond_to: oneshot::Sender<Option<AccountState>>,
    },
    GetCanonicalStateRoot {
        height: u64,
        respond_to: oneshot::Sender<Option<String>>,
    },
    IsCanonicalStateRoot {
        height: u64,
        state_root: String,
        respond_to: oneshot::Sender<bool>,
    },
    CurrentStateRoot {
        respond_to: oneshot::Sender<String>,
    },
    GetAccountStateClone {
        respond_to: oneshot::Sender<AccountState>,
    },
    ApplyCanonicalStateSnapshot {
        height: u64,
        state: AccountState,
        respond_to: oneshot::Sender<Result<(), BlockchainError>>,
    },
    AddNetworkBlock {
        block: Block,
        respond_to: oneshot::Sender<Result<(), BlockchainError>>,
    },
    HasBlock {
        hash: String,
        respond_to: oneshot::Sender<bool>,
    },
    HasBlockAtIndex {
        index: u64,
        hash: String,
        respond_to: oneshot::Sender<bool>,
    },
    GetBlockByHeight {
        height: u64,
        respond_to: oneshot::Sender<Option<Block>>,
    },
    GetHeight {
        respond_to: oneshot::Sender<u64>,
    },
    LoadBlockFromStorage {
        height: u64,
        respond_to: oneshot::Sender<Option<Block>>,
    },
    GetAddressInfo {
        address: String,
        respond_to: oneshot::Sender<Option<AccountBalance>>,
    },
    FindTransactionByHash {
        hash: String,
        respond_to: oneshot::Sender<Option<Transaction>>,
    },
    GetLatestBlocks {
        count: usize,
        respond_to: oneshot::Sender<Vec<Block>>,
    },
    GetAddressTransactions {
        address: String,
        max_blocks: u64,
        respond_to: oneshot::Sender<Vec<AddressTransaction>>,
    },
    GetBlockHashAt {
        height: u64,
        respond_to: oneshot::Sender<Option<String>>,
    },
    DeepReorg {
        rollback_to: u64,
        new_chain: Vec<Block>,
        respond_to: oneshot::Sender<Result<(), BlockchainError>>,
    },
    GetPendingTransactions {
        respond_to: oneshot::Sender<Vec<Transaction>>,
    },
    GetMempoolSize {
        respond_to: oneshot::Sender<usize>,
    }
}

/// A handle to the Blockchain Actor that allows asynchronous interaction.
#[derive(Clone)]
pub struct BlockchainHandle {
    sender: mpsc::Sender<BlockchainMessage>,
}

#[derive(Error, Debug)]
pub enum ActorError {
    #[error("Actor is down")]
    ActorDown,
}

impl BlockchainHandle {
    pub fn new(sender: mpsc::Sender<BlockchainMessage>) -> Self {
        Self { sender }
    }

    async fn send_msg<T, F>(&self, msg_factory: F) -> Result<T, ActorError>
    where
        F: FnOnce(oneshot::Sender<T>) -> BlockchainMessage,
    {
        let (tx, rx) = oneshot::channel();
        let msg = msg_factory(tx);
        if self.sender.send(msg).await.is_err() {
            return Err(ActorError::ActorDown);
        }
        rx.await.map_err(|_| ActorError::ActorDown)
    }

    pub async fn subscribe_new_blocks(&self) -> Result<watch::Receiver<u64>, ActorError> {
        self.send_msg(|respond_to| BlockchainMessage::SubscribeNewBlocks { respond_to }).await
    }

    pub async fn get_latest_block(&self) -> Result<Block, ActorError> {
        self.send_msg(|respond_to| BlockchainMessage::GetLatestBlock { respond_to }).await
    }

    pub async fn add_transaction(&self, transaction: Transaction) -> Result<Result<(), BlockchainError>, ActorError> {
        self.send_msg(|respond_to| BlockchainMessage::AddTransaction { transaction, respond_to }).await
    }

    pub async fn create_block_template(&self, miner_address: String) -> Result<Result<Block, BlockchainError>, ActorError> {
        self.send_msg(|respond_to| BlockchainMessage::CreateBlockTemplate { miner_address, respond_to }).await
    }

    pub async fn create_bft_block_template(&self, next_height: u64, proposer: String, epoch: u64, bft_round: u32) -> Result<Result<Block, BlockchainError>, ActorError> {
        self.send_msg(|respond_to| BlockchainMessage::CreateBftBlockTemplate { next_height, proposer, epoch, bft_round, respond_to }).await
    }

    pub async fn get_account_state_snapshot(&self) -> Result<AccountState, ActorError> {
        self.send_msg(|respond_to| BlockchainMessage::GetAccountStateSnapshot { respond_to }).await
    }

    pub async fn get_block_by_index(&self, index: u64) -> Result<Option<Block>, ActorError> {
        self.send_msg(|respond_to| BlockchainMessage::GetBlockByIndex { index, respond_to }).await
    }

    pub async fn is_valid(&self) -> Result<bool, ActorError> {
        self.send_msg(|respond_to| BlockchainMessage::IsValid { respond_to }).await
    }

    pub async fn get_stats(&self) -> Result<BlockchainStats, ActorError> {
        self.send_msg(|respond_to| BlockchainMessage::GetStats { respond_to }).await
    }

    pub async fn get_balance(&self, address: String) -> Result<u64, ActorError> {
        self.send_msg(|respond_to| BlockchainMessage::GetBalance { address, respond_to }).await
    }

    pub async fn cumulative_work_at(&self, tip_height: u64) -> Result<u128, ActorError> {
        self.send_msg(|respond_to| BlockchainMessage::CumulativeWorkAt { tip_height, respond_to }).await
    }

    pub async fn flush_storage(&self) -> Result<(), ActorError> {
        self.send_msg(|respond_to| BlockchainMessage::FlushStorage { respond_to }).await
    }

    pub async fn load_account_state_at_height(&self, height: u64) -> Result<Option<AccountState>, ActorError> {
        self.send_msg(|respond_to| BlockchainMessage::LoadAccountStateAtHeight { height, respond_to }).await
    }

    pub async fn get_canonical_state_root(&self, height: u64) -> Result<Option<String>, ActorError> {
        self.send_msg(|respond_to| BlockchainMessage::GetCanonicalStateRoot { height, respond_to }).await
    }

    pub async fn is_canonical_state_root(&self, height: u64, state_root: String) -> Result<bool, ActorError> {
        self.send_msg(|respond_to| BlockchainMessage::IsCanonicalStateRoot { height, state_root, respond_to }).await
    }

    pub async fn current_state_root(&self) -> Result<String, ActorError> {
        self.send_msg(|respond_to| BlockchainMessage::CurrentStateRoot { respond_to }).await
    }

    pub async fn get_account_state_clone(&self) -> Result<AccountState, ActorError> {
        self.send_msg(|respond_to| BlockchainMessage::GetAccountStateClone { respond_to }).await
    }

    pub async fn apply_canonical_state_snapshot(&self, height: u64, state: AccountState) -> Result<Result<(), BlockchainError>, ActorError> {
        self.send_msg(|respond_to| BlockchainMessage::ApplyCanonicalStateSnapshot { height, state, respond_to }).await
    }

    pub async fn add_network_block(&self, block: Block) -> Result<Result<(), BlockchainError>, ActorError> {
        self.send_msg(|respond_to| BlockchainMessage::AddNetworkBlock { block, respond_to }).await
    }

    pub async fn has_block(&self, hash: String) -> Result<bool, ActorError> {
        self.send_msg(|respond_to| BlockchainMessage::HasBlock { hash, respond_to }).await
    }

    pub async fn has_block_at_index(&self, index: u64, hash: String) -> Result<bool, ActorError> {
        self.send_msg(|respond_to| BlockchainMessage::HasBlockAtIndex { index, hash, respond_to }).await
    }

    pub async fn get_block_by_height(&self, height: u64) -> Result<Option<Block>, ActorError> {
        self.send_msg(|respond_to| BlockchainMessage::GetBlockByHeight { height, respond_to }).await
    }

    pub async fn get_height(&self) -> Result<u64, ActorError> {
        self.send_msg(|respond_to| BlockchainMessage::GetHeight { respond_to }).await
    }

    pub async fn load_block_from_storage(&self, height: u64) -> Result<Option<Block>, ActorError> {
        self.send_msg(|respond_to| BlockchainMessage::LoadBlockFromStorage { height, respond_to }).await
    }

    pub async fn get_address_info(&self, address: String) -> Result<Option<AccountBalance>, ActorError> {
        self.send_msg(|respond_to| BlockchainMessage::GetAddressInfo { address, respond_to }).await
    }

    pub async fn find_transaction_by_hash(&self, hash: String) -> Result<Option<Transaction>, ActorError> {
        self.send_msg(|respond_to| BlockchainMessage::FindTransactionByHash { hash, respond_to }).await
    }

    pub async fn get_latest_blocks(&self, count: usize) -> Result<Vec<Block>, ActorError> {
        self.send_msg(|respond_to| BlockchainMessage::GetLatestBlocks { count, respond_to }).await
    }

    pub async fn get_address_transactions(&self, address: String, max_blocks: u64) -> Result<Vec<AddressTransaction>, ActorError> {
        self.send_msg(|respond_to| BlockchainMessage::GetAddressTransactions { address, max_blocks, respond_to }).await
    }

    pub async fn get_block_hash_at(&self, height: u64) -> Result<Option<String>, ActorError> {
        self.send_msg(|respond_to| BlockchainMessage::GetBlockHashAt { height, respond_to }).await
    }

    pub async fn deep_reorg(&self, rollback_to: u64, new_chain: Vec<Block>) -> Result<Result<(), BlockchainError>, ActorError> {
        self.send_msg(|respond_to| BlockchainMessage::DeepReorg { rollback_to, new_chain, respond_to }).await
    }

    pub async fn get_pending_transactions(&self) -> Result<Vec<Transaction>, ActorError> {
        self.send_msg(|respond_to| BlockchainMessage::GetPendingTransactions { respond_to }).await
    }

    pub async fn get_mempool_size(&self) -> Result<usize, ActorError> {
        self.send_msg(|respond_to| BlockchainMessage::GetMempoolSize { respond_to }).await
    }
}

/// The main Actor loop that processes messages.
pub struct BlockchainActor {
    blockchain: Blockchain,
    receiver: mpsc::Receiver<BlockchainMessage>,
}

impl BlockchainActor {
    pub fn new(blockchain: Blockchain, receiver: mpsc::Receiver<BlockchainMessage>) -> Self {
        Self { blockchain, receiver }
    }

    pub async fn run(mut self) {
        while let Some(msg) = self.receiver.recv().await {
            match msg {
                BlockchainMessage::SubscribeNewBlocks { respond_to } => {
                    let _ = respond_to.send(self.blockchain.subscribe_new_blocks());
                }
                BlockchainMessage::GetLatestBlock { respond_to } => {
                    let _ = respond_to.send(self.blockchain.get_latest_block());
                }
                BlockchainMessage::AddTransaction { transaction, respond_to } => {
                    let _ = respond_to.send(self.blockchain.add_transaction(transaction));
                }
                BlockchainMessage::CreateBlockTemplate { miner_address, respond_to } => {
                    let _ = respond_to.send(self.blockchain.create_block_template(miner_address));
                }
                BlockchainMessage::CreateBftBlockTemplate { next_height, proposer, epoch, bft_round, respond_to } => {
                    let _ = respond_to.send(self.blockchain.create_bft_block_template(next_height, proposer, epoch, bft_round));
                }
                BlockchainMessage::GetAccountStateSnapshot { respond_to } => {
                    let _ = respond_to.send(self.blockchain.get_account_state_snapshot());
                }
                BlockchainMessage::GetBlockByIndex { index, respond_to } => {
                    let _ = respond_to.send(self.blockchain.get_block_by_index(index));
                }
                BlockchainMessage::IsValid { respond_to } => {
                    let _ = respond_to.send(self.blockchain.is_valid());
                }
                BlockchainMessage::GetStats { respond_to } => {
                    let _ = respond_to.send(self.blockchain.get_stats());
                }
                BlockchainMessage::GetBalance { address, respond_to } => {
                    let _ = respond_to.send(self.blockchain.get_balance(&address));
                }
                BlockchainMessage::CumulativeWorkAt { tip_height, respond_to } => {
                    let _ = respond_to.send(self.blockchain.cumulative_work_at(tip_height));
                }
                BlockchainMessage::FlushStorage { respond_to } => {
                    self.blockchain.flush_storage();
                    let _ = respond_to.send(());
                }
                BlockchainMessage::LoadAccountStateAtHeight { height, respond_to } => {
                    let _ = respond_to.send(self.blockchain.load_account_state_at_height(height));
                }
                BlockchainMessage::GetCanonicalStateRoot { height, respond_to } => {
                    let _ = respond_to.send(self.blockchain.get_canonical_state_root(height));
                }
                BlockchainMessage::IsCanonicalStateRoot { height, state_root, respond_to } => {
                    let _ = respond_to.send(self.blockchain.is_canonical_state_root(height, &state_root));
                }
                BlockchainMessage::CurrentStateRoot { respond_to } => {
                    let _ = respond_to.send(self.blockchain.current_state_root());
                }
                BlockchainMessage::GetAccountStateClone { respond_to } => {
                    let _ = respond_to.send(self.blockchain.get_account_state_clone());
                }
                BlockchainMessage::ApplyCanonicalStateSnapshot { height, state, respond_to } => {
                    let _ = respond_to.send(self.blockchain.apply_canonical_state_snapshot(height, state));
                }
                BlockchainMessage::AddNetworkBlock { block, respond_to } => {
                    let _ = respond_to.send(self.blockchain.add_network_block(block));
                }
                BlockchainMessage::HasBlock { hash, respond_to } => {
                    let _ = respond_to.send(self.blockchain.has_block(&hash));
                }
                BlockchainMessage::HasBlockAtIndex { index, hash, respond_to } => {
                    let _ = respond_to.send(self.blockchain.has_block_at_index(index, &hash));
                }
                BlockchainMessage::GetBlockByHeight { height, respond_to } => {
                    let _ = respond_to.send(self.blockchain.get_block_by_height(height));
                }
                BlockchainMessage::GetHeight { respond_to } => {
                    let _ = respond_to.send(self.blockchain.get_height());
                }
                BlockchainMessage::LoadBlockFromStorage { height, respond_to } => {
                    let _ = respond_to.send(self.blockchain.load_block_from_storage(height));
                }
                BlockchainMessage::GetAddressInfo { address, respond_to } => {
                    let _ = respond_to.send(self.blockchain.get_address_info(&address));
                }
                BlockchainMessage::FindTransactionByHash { hash, respond_to } => {
                    let _ = respond_to.send(self.blockchain.find_transaction_by_hash(&hash));
                }
                BlockchainMessage::GetLatestBlocks { count, respond_to } => {
                    let _ = respond_to.send(self.blockchain.get_latest_blocks(count));
                }
                BlockchainMessage::GetAddressTransactions { address, max_blocks, respond_to } => {
                    let _ = respond_to.send(self.blockchain.get_address_transactions(&address, max_blocks));
                }
                BlockchainMessage::GetBlockHashAt { height, respond_to } => {
                    let _ = respond_to.send(self.blockchain.get_block_hash_at(height));
                }
                BlockchainMessage::DeepReorg { rollback_to, new_chain, respond_to } => {
                    let _ = respond_to.send(self.blockchain.deep_reorg(rollback_to, new_chain));
                }
                BlockchainMessage::GetPendingTransactions { respond_to } => {
                    let pending = self.blockchain.get_pending_transactions().clone();
                    let _ = respond_to.send(pending);
                }
                BlockchainMessage::GetMempoolSize { respond_to } => {
                    let size = self.blockchain.get_pending_transactions().len();
                    let _ = respond_to.send(size);
                }
            }
        }
    }
}
