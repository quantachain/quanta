use std::sync::Arc;
use tokio::sync::{watch, RwLock};
use tracing::{info, warn, error};
use aleph_bft::{run_session, Config as AlephConfig, LocalIO, SpawnHandle, Terminator, NodeIndex, NodeCount, default_config};
use std::time::Duration;

use crate::consensus::Blockchain;
use crate::core::block::Block;
use crate::crypto::wallet::QuantumWallet;
use crate::network::Network;

use super::aleph_keychain::{QuantaKeychain, QuantaHasher, FalconSignature};
use super::aleph_data::{QuantaDataProvider, QuantaFinalizationHandler};
use super::aleph_network::QuantaNetworkBridge;

#[derive(Clone)]
pub struct QuantaSpawnHandle;

impl SpawnHandle for QuantaSpawnHandle {
    fn spawn(&self, _name: &'static str, task: impl core::future::Future<Output = ()> + Send + 'static) {
        tokio::spawn(task);
    }
    
    fn spawn_essential(&self, _name: &'static str, task: impl core::future::Future<Output = ()> + Send + 'static) -> aleph_bft::TaskHandle {
        let (tx, rx) = tokio::sync::oneshot::channel();
        tokio::spawn(async move {
            task.await;
            let _ = tx.send(());
        });
        
        Box::pin(async move {
            if rx.await.is_err() {
                tracing::error!("AlephBFT essential task panicked or exited unexpectedly!");
                return Err(());
            }
            Ok(())
        })
    }
}

pub async fn run_bft_proposer(
    blockchain: Arc<RwLock<Blockchain>>,
    wallet: Arc<QuantumWallet>,
    network: Option<Arc<Network>>,
    _new_block_rx: watch::Receiver<u64>,
    data_dir: String,
) {
    info!("Starting AlephBFT Session for validator {}", wallet.address);

    let network_ref = if let Some(n) = network {
        n
    } else {
        warn!("BFT Proposer requires network access. Exiting BFT loop.");
        return;
    };

    // We need to determine the NodeIndex and NodeCount.
    // For a 5-node test network, we can hardcode it based on a known committee or just get it from the state.
    // For now, let's fetch the committee from the blockchain state to dynamically set this.
    
    let (committee, committee_pubkeys) = {
        let bc = blockchain.read().await;
        let snap = bc.get_account_state_snapshot();
        let comm = super::authorities::compute_committee(&snap);
        let mut pubkeys = Vec::new();
        for addr in &comm {
            if let Some(info) = snap.get_validator_info(addr) {
                pubkeys.push(info.falcon_pk.clone());
            } else {
                // If a validator is missing public key, we use dummy data so indexes align.
                // In a real network, this shouldn't happen for active committee members.
                pubkeys.push(vec![]);
            }
        }
        (comm, pubkeys)
    };
    
    if committee.is_empty() {
        warn!("BFT Proposer: no active validators — exiting BFT loop.");
        return;
    }

    let my_address = wallet.address.clone();
    let node_idx_opt = committee.iter().position(|addr| *addr == my_address);
    
    let node_idx = match node_idx_opt {
        Some(idx) => NodeIndex(idx),
        None => {
            info!("BFT Proposer: I am not in the committee. Observer mode.");
            // We should ideally still run some synchronization, but AlephBFT runs on nodes.
            // For now, just exit if not in committee.
            return;
        }
    };
    
    let node_count = NodeCount(committee.len());
    info!("BFT Proposer: I am validator {} out of {}", node_idx.0, node_count.0);

    // 1. Setup Keychain
    let keychain = QuantaKeychain::new(wallet.clone(), node_idx, node_count, committee_pubkeys);

    // 2. Setup Data Provider & Finalization Handler
    let data_provider = QuantaDataProvider::new(blockchain.clone(), my_address.clone());
    
    let (finalized_tx, mut finalized_rx) = tokio::sync::mpsc::unbounded_channel();
    let finalization_handler = QuantaFinalizationHandler::new(finalized_tx);
    
    // Spawn task to process finalized blocks
    let bc_for_finalization = blockchain.clone();
    let net_for_finalization = network_ref.clone();
    tokio::spawn(async move {
        while let Some(block) = finalized_rx.recv().await {
            info!("BFT Proposer: AlephBFT finalized block {}", block.index);
            let mut bc = bc_for_finalization.write().await;
            if let Err(e) = bc.add_network_block(block.clone()) {
                error!("BFT Proposer: failed to apply finalized block {}: {}", block.index, e);
            } else {
                info!("✓ BFT block {} applied to local chain.", block.index);
                net_for_finalization.broadcast_block(block).await;
            }
        }
    });

    // 3. Setup Network Bridge
    let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
    network_ref.register_aleph_bft_tx(tx).await;
    let network_bridge: QuantaNetworkBridge<aleph_bft::NetworkData<QuantaHasher, Block, FalconSignature, aleph_bft::SignatureSet<FalconSignature>>> = QuantaNetworkBridge::new(network_ref.clone(), rx, node_idx.0);

    // 4. Setup LocalIO with Crash Recovery (Persistent Unit Saver / Loader)
    use tokio_util::compat::{TokioAsyncReadCompatExt, TokioAsyncWriteCompatExt};
    use std::path::Path;

    let backup_path = Path::new(&data_dir).join("alephbft_backup.dat");
    info!("BFT Proposer: Using backup file {:?}", backup_path);

    // Open file for saving (append only). Create if missing.
    let file_for_saving = tokio::fs::OpenOptions::new()
        .write(true)
        .create(true)
        .append(true)
        .open(&backup_path)
        .await
        .expect("Failed to open AlephBFT backup file for writing");
        
    let unit_saver = file_for_saving.compat_write();

    // Open file for loading (read only). If file is empty or new, loader will just hit EOF.
    let file_for_loading = tokio::fs::File::open(&backup_path)
        .await
        .expect("Failed to open AlephBFT backup file for reading");
        
    let unit_loader = file_for_loading.compat();

    let local_io = LocalIO::new(data_provider, finalization_handler, unit_saver, unit_loader);

    // 5. Config
    // Provide node_count, node_idx, session_id, max_round (e.g. 5000), unit_creation_delay (e.g. 500ms)
    let config = default_config(node_count, node_idx, 0, 5000, Duration::from_millis(500))
        .expect("Valid default config");
    // We can tune config here if it wasn't immutable, but default is fine.
    
    // 6. SpawnHandle & Terminator
    let spawn_handle = QuantaSpawnHandle;
    let (terminator_tx, terminator_rx) = futures::channel::oneshot::channel();
    let terminator = Terminator::create_root(terminator_rx, "QuantaBFT");

    info!("BFT Proposer: running aleph_bft::run_session...");
    
    // Spawn run_session
    // AlephBFT run_session blocks until session ends (which is never in our case unless an error occurs)
    run_session(
        config,
        local_io,
        network_bridge,
        keychain,
        spawn_handle,
        terminator,
    ).await;
    
    warn!("BFT Proposer: aleph_bft session exited unexpectedly.");
}
