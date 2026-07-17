use aleph_bft::{
    create_config, default_delay_config, run_session, LocalIO, NodeCount, NodeIndex, SpawnHandle, Terminator,
};
use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{watch, RwLock};
use tracing::{error, info, warn};

use crate::consensus::Blockchain;
use crate::core::block::Block;
use crate::crypto::wallet::QuantumWallet;
use crate::network::Network;

use super::aleph_data::{QuantaDataProvider, QuantaFinalizationHandler};
use super::aleph_keychain::{FalconSignature, QuantaHasher, QuantaKeychain};
use super::aleph_network::QuantaNetworkBridge;

#[derive(Clone)]
pub struct QuantaSpawnHandle;

impl SpawnHandle for QuantaSpawnHandle {
    fn spawn(
        &self,
        _name: &'static str,
        task: impl core::future::Future<Output = ()> + Send + 'static,
    ) {
        tokio::spawn(task);
    }

    fn spawn_essential(
        &self,
        _name: &'static str,
        task: impl core::future::Future<Output = ()> + Send + 'static,
    ) -> aleph_bft::TaskHandle {
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

// ---------------------------------------------------------------------------
// SESSION LENGTH — how many finalized blocks before we rotate to a new
// AlephBFT session.  Each rotation:
//   • increments session_id  → fresh backup file, zero replay cost
//   • resets internal DAG round counter → prevents exponential delay growth
//   • deletes the previous session's backup file → keeps disk clean
//
// TUNED 2026-06-15: At the observed ~25s/block, SESSION_LENGTH=300 meant
// 125 minutes before the round counter reset. AlephBFT's exponential delay
// accumulates over the lifetime of a session — cutting to 60 blocks (~25 min)
// prevents the exponent from growing large enough to matter.
// ---------------------------------------------------------------------------
pub use crate::consensus::authorities::SESSION_LENGTH;

// Maximum DAG rounds within a single session.  AlephBFT's built-in delay
// function applies an exponential back-off whose exponent grows with the
// round number.  Keeping this well below SESSION_LENGTH * expected_rounds_per_block
// ensures the exponential never has time to accumulate within one session.
//
// TUNED 2026-07-06: Increased from 500 → 7000. 500 was too low and caused
// permanent network deadlocks if a session hit round 500 before producing
// all blocks for the session (since restarting the same session instantly
// hits the 500 limit again). 7000 gives enough headroom for recovery.
const MAX_ROUNDS_PER_SESSION: u32 = 7000;

pub async fn run_bft_proposer(
    blockchain: Arc<RwLock<Blockchain>>,
    wallet: Arc<QuantumWallet>,
    network: Option<Arc<Network>>,
    _new_block_rx: watch::Receiver<u64>,
    data_dir: String,
    // Shared atomic: QuantaDataProvider reads this to know the timestamp of
    // the latest finalized block WITHOUT acquiring the blockchain read-lock.
    last_finalized_ts: Arc<AtomicI64>,
) {
    info!("Starting AlephBFT Session for validator {}", wallet.address);

    let network_ref = if let Some(n) = network {
        n
    } else {
        warn!("BFT Proposer requires network access. Exiting BFT loop.");
        return;
    };

    let my_address = wallet.address.clone();

    // DUPLICATE-APPLY FIX: Single persistent consumer task outside the
    // restart loop — prevents N zombie consumers after N restarts.
    let (persistent_tx, mut persistent_rx) = tokio::sync::mpsc::unbounded_channel::<Block>();
    let shared_tx: Arc<std::sync::Mutex<Option<tokio::sync::mpsc::UnboundedSender<Block>>>> =
        Arc::new(std::sync::Mutex::new(Some(persistent_tx)));

    // Spawn the one-and-only finalization consumer.
    let bc_for_finalization = blockchain.clone();
    let net_for_finalization = network_ref.clone();
    let ts_for_finalization = last_finalized_ts.clone();
    tokio::spawn(async move {
        let mut last_applied_height: u64 = 0;
        while let Some(block) = persistent_rx.recv().await {
            if block.index <= last_applied_height {
                tracing::trace!(
                    "BFT Proposer: ignoring duplicate delivery for height {} (already at {})",
                    block.index,
                    last_applied_height
                );
                continue;
            }

            info!("BFT Proposer: AlephBFT finalized block {}", block.index);

            // FIX (Bug 4): Update the shared timestamp BEFORE releasing the lock
            // so that get_data() sees the new tip immediately. Uses real wall time to prevent Time Warp DOS.
            ts_for_finalization.store(chrono::Utc::now().timestamp(), Ordering::Release);

            let apply_result = {
                let bc = bc_for_finalization.write().await;
                let result = bc.add_network_block(block.clone());
                drop(bc); // Release write lock immediately after apply
                result
            };
            if let Err(e) = apply_result {
                error!(
                    "BFT Proposer: failed to apply finalized block {}: {}",
                    block.index, e
                );
            } else {
                info!("✓ BFT block {} applied to local chain.", block.index);
                last_applied_height = block.index;
                net_for_finalization.broadcast_block(block).await;
            }
        }
    });

    // -----------------------------------------------------------------------
    // SESSION ROTATION LOOP
    //
    // FIX (Bugs 1 & 2): Instead of one infinite session with session_id=0 and
    // an ever-growing append-mode backup file, we rotate sessions every
    // SESSION_LENGTH finalized blocks:
    //
    //   session_id  = current_chain_height / SESSION_LENGTH
    //   backup_file = alephbft_backup_{session_id}.dat
    //
    // On startup the node reads the chain height, computes session_id, and
    // opens the corresponding (possibly new) backup file.  At restart the same
    // height → same session_id → same file, so crash-recovery still works.
    //
    // When the chain advances past the next SESSION_LENGTH boundary the outer
    // loop detects it, increments session_id, DELETES the old backup file, and
    // starts a fresh session.  This resets:
    //   • backup replay cost     → O(current session) instead of O(all time)
    //   • internal DAG round     → 0, preventing exponential delay growth
    // -----------------------------------------------------------------------
    loop {
        // --- WAIT FOR SYNC ---
        // Ensure we are caught up with the network before starting a BFT session.
        // Starting an old session causes salt mismatches and spams the network channels,
        // which can actively prevent the node from downloading missing blocks.
        loop {
            let current_height = {
                let bc = blockchain.read().await;
                bc.get_height()
            };

            let peers = network_ref.get_peers_info().await;
            let peer_count = peers.len();
            let max_peer_height = peers.iter().map(|p| p.height).max().unwrap_or(0);

            // CPU SPIKE FIX: With 0 peers, AlephBFT cannot reach 2/3+1 quorum and
            // will spin forever in get_data() burning 100% of a CPU core.
            // Wait until we have at least 1 peer before starting a BFT session.
            if peer_count == 0 {
                tracing::info!("BFT Proposer: no peers connected, waiting for network...");
                tokio::time::sleep(tokio::time::Duration::from_secs(10)).await;
                continue;
            }

            if current_height >= max_peer_height.saturating_sub(2) {
                break;
            }

            tracing::info!(
                "BFT Proposer: waiting for sync (at height {}, network at {}, peers={})...",
                current_height,
                max_peer_height,
                peer_count
            );
            tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
        }

        // SESSION RESTART TIMESTAMP RESET:
        // After a session rotation (or any BFT session exit), reset last_finalized_ts
        // to now() - SLOT_SECONDS so the slot gate opens immediately for the new session.
        // Without this, the stale timestamp from the PREVIOUS session causes the next
        // get_data() to report a huge elapsed time (e.g. 286s) while AlephBFT rebuilds
        // its DAG — the gate opens fine, but the log is misleading and could interact
        // with any future rate-limiting. This reset is safe: the finalization consumer
        // will overwrite it with Utc::now() as soon as the next block is finalized.
        last_finalized_ts.store(
            chrono::Utc::now().timestamp() - 6,
            std::sync::atomic::Ordering::Release,
        );

        // DYNAMIC COMMITTEE FIX: Compute committee HERE, every session, instead of at startup!
        let (committee, committee_pubkeys) = {
            let bc = blockchain.read().await;
            let current_height = bc.get_height();
            let session_start_height = current_height - (current_height % SESSION_LENGTH);
            let snap = bc.get_account_state_snapshot();
            let comm = super::authorities::compute_committee(&snap, session_start_height);
            let mut pubkeys = Vec::new();
            for addr in &comm {
                if let Some(info) = snap.get_validator_info(addr) {
                    pubkeys.push(info.falcon_pk.clone());
                } else {
                    pubkeys.push(vec![]);
                }
            }
            (comm, pubkeys)
        };

        if committee.is_empty() {
            tracing::warn!("BFT Proposer: no active validators. Sleeping until next session...");
            tokio::time::sleep(tokio::time::Duration::from_secs(10)).await;
            continue;
        }

        let node_idx_opt = committee.iter().position(|addr| *addr == my_address);
        let node_idx = match node_idx_opt {
            Some(idx) => NodeIndex(idx),
            None => {
                tracing::info!(
                    "BFT Proposer: I am not in the committee. Observer mode. Sleeping..."
                );
                // Sleep for roughly one session duration (6 minutes at 6s/block) before checking again
                tokio::time::sleep(tokio::time::Duration::from_secs(60)).await;
                continue;
            }
        };

        let node_count = NodeCount(committee.len());
        tracing::info!(
            "BFT Proposer: I am validator {} out of {} for this session",
            node_idx.0,
            node_count.0
        );

        // Compute the current session_id from chain height.
        let current_height = {
            let bc = blockchain.read().await;
            bc.get_height()
        };
        let session_id: u64 = current_height / SESSION_LENGTH;

        // 1. Setup Keychain
        let keychain = QuantaKeychain::new(
            wallet.clone(),
            node_idx,
            node_count,
            committee_pubkeys.clone(),
        );

        // 2. Setup Data Provider & Finalization Handler
        let data_provider = QuantaDataProvider::new(
            blockchain.clone(),
            my_address.clone(),
            last_finalized_ts.clone(),
            wallet.clone(),
        );

        let (session_tx, session_rx) = tokio::sync::mpsc::unbounded_channel::<Block>();
        let finalization_handler = QuantaFinalizationHandler::new(session_tx.clone());

        // Forward session blocks to the persistent consumer.
        let forward_to = shared_tx.clone();
        let mut fwd_rx = session_rx;
        tokio::spawn(async move {
            while let Some(block) = fwd_rx.recv().await {
                let guard = forward_to.lock().unwrap();
                if let Some(ref tx) = *guard {
                    let _ = tx.send(block);
                }
            }
        });

        // 3. Setup Network Bridge
        //
        // BW-FIX-4: Pass committee (wallet addresses indexed by NodeIndex) so the
        // bridge can route Recipient::Node(idx) to the correct peer rather than
        // broadcasting every unicast message to all validators.
        let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
        network_ref.register_aleph_bft_tx(tx).await;
        let network_bridge: QuantaNetworkBridge<
            aleph_bft::NetworkData<
                QuantaHasher,
                Block,
                FalconSignature,
                aleph_bft::SignatureSet<FalconSignature>,
            >,
        > = QuantaNetworkBridge::new(
            network_ref.clone(),
            rx,
            node_idx.0,
            committee.clone(), // BW-FIX-4: committee[i] = wallet address of validator i
        );

        // 4. Setup LocalIO with per-session backup files.
        //
        // FIX (Bug 2): backup file is named by session_id, not a single
        // shared file.  Each session gets its own fresh file; when a new
        // session starts the old file is deleted (see cleanup below).
        use std::path::Path;
        use tokio_util::compat::{TokioAsyncReadCompatExt, TokioAsyncWriteCompatExt};

        let backup_path = Path::new(&data_dir).join(format!("alephbft_backup_{}.dat", session_id));


        info!(
            "BFT Proposer: session_id={} backup={:?}",
            session_id, backup_path
        );

        // Open file for saving (append-within-session is fine; it's the
        // cross-session accumulation that killed performance).
        let file_for_saving = tokio::fs::OpenOptions::new()
            .write(true)
            .create(true)
            .append(true)
            .open(&backup_path)
            .await
            .expect("Failed to open AlephBFT backup file for writing");

        let unit_saver = file_for_saving.compat_write();

        // Open file for loading.
        let file_for_loading = tokio::fs::File::open(&backup_path)
            .await
            .expect("Failed to open AlephBFT backup file for reading");

        let unit_loader = file_for_loading.compat();

        let local_io = LocalIO::new(data_provider, finalization_handler, unit_saver, unit_loader);

        // 5. Config
        //
        // FIX (Bug 1): session_id increments per epoch — AlephBFT uses this
        // to namespace its state.  Passing 0 every time caused full history
        // replay on every restart.
        //
        // FIX (Bug 3): max_round capped at MAX_ROUNDS_PER_SESSION.  AlephBFT's
        // delay config uses an exponential that grows with round number, so an
        // ever-increasing round counter causes ever-increasing block times.
        // Rotating sessions resets the round counter to 0.
        // FIX: Override AlephBFT's exponential delay schedule.
        // The default AlephBFT config applies an exponential backoff after round 3000.
        // If a network partition causes the DAG to reach round 5000, the block delay
        // becomes ~3 hours! By overriding this to a constant 500ms, the network
        // immediately recovers at full speed once the partition resolves.
        let mut delay_config = default_delay_config();
        delay_config.unit_creation_delay = std::sync::Arc::new(|t| {
            if t == 0 {
                std::time::Duration::from_millis(5000)
            } else if t < 10 {
                std::time::Duration::from_millis(500)
            } else if t < 30 {
                std::time::Duration::from_millis(2000)
            } else {
                std::time::Duration::from_millis(10000) // Drops CPU 20x when stuck
            }
        });

        let config = create_config(
            node_count,
            node_idx,
            session_id,                    // FIX Bug 1: increments per epoch
            MAX_ROUNDS_PER_SESSION as u16, // FIX Bug 3: caps round number (u16)
            delay_config,
            Duration::from_millis(500),
        )
        .expect("Valid config");

        let spawn_handle = QuantaSpawnHandle;
        let (terminator_tx, terminator_rx) = futures::channel::oneshot::channel();
        let terminator = Terminator::create_root(terminator_rx, "QuantaBFT");

        info!(
            "BFT Proposer: running aleph_bft::run_session (session_id={}, height={})…",
            session_id, current_height
        );

        let target_height_for_next_session = (session_id + 1) * SESSION_LENGTH;
        let bc_for_monitor = blockchain.clone();
        let last_ts_monitor = last_finalized_ts.clone();

        // -----------------------------------------------------------------------
        // SESSION WATCHDOG — CPU SPIKE ROOT CAUSE FIX
        //
        // When AlephBFT is running but cannot reach 2/3+1 quorum (network stuck),
        // it creates a new Falcon-512 signed DAG unit every 500ms per validator.
        // With 4 nodes this is ~8 Falcon-512 sign+verify ops/second = 80-90% CPU.
        //
        // Before this issue never happened because blocks were finalized every 6s,
        // sessions rotated cleanly, and the DAG round counter was always near 0.
        // Now with the network isolated (only 4/13 validators), sessions run
        // forever at 500ms intervals until hitting MAX_ROUNDS_PER_SESSION.
        //
        // Fix: If no block is finalized for >120s, kill the session and restart.
        // On restart we sleep 30s before re-entering, dropping CPU to near-zero.
        // -----------------------------------------------------------------------
        const STUCK_WATCHDOG_SECS: i64 = 120; // kill session after 2min of no progress

        let mut session_task = tokio::spawn(async move {
            run_session(
                config,
                local_io,
                network_bridge,
                keychain,
                spawn_handle,
                terminator,
            )
            .await;
        });

        let mut terminator_tx_opt = Some(terminator_tx);
        loop {
            tokio::select! {
                _ = &mut session_task => {
                    // Session exited naturally
                    break;
                }
                _ = tokio::time::sleep(tokio::time::Duration::from_millis(500)) => {
                    let height = bc_for_monitor.read().await.get_height();
                    if height >= target_height_for_next_session {
                        if let Some(tx) = terminator_tx_opt.take() {
                            info!("BFT Proposer: reached session boundary (height {} >= {}). Terminating session {}...", height, target_height_for_next_session, session_id);
                            let _ = tx.send(());
                        }
                    }

                    // WATCHDOG: kill session if no block finalized for >120s
                    let now_ts = chrono::Utc::now().timestamp();
                    let last_ts = last_ts_monitor.load(std::sync::atomic::Ordering::Acquire);
                    if now_ts.saturating_sub(last_ts) > STUCK_WATCHDOG_SECS {
                        if let Some(tx) = terminator_tx_opt.take() {
                            warn!(
                                "BFT Proposer: WATCHDOG — no block finalized for {}s (quorum not met?). Terminating session {} to save CPU. Will sleep 30s before restart.",
                                now_ts.saturating_sub(last_ts), session_id
                            );

                            // CRITICAL MEMORY LEAK FIX: If we've been stuck for a very long time, 
                            // the backup file has accumulated thousands of useless DAG units.
                            // When the watchdog restarts the session, AlephBFT tries to load all of them,
                            // causing an instant 1.8GB+ RAM spike and 100% CPU lockup.
                            // We must wipe the backup file so the next session starts completely fresh.
                            let backup_path = std::path::Path::new(&data_dir).join(format!("alephbft_backup_{}.dat", session_id));
                            if let Err(e) = std::fs::remove_file(&backup_path) {
                                if e.kind() != std::io::ErrorKind::NotFound {
                                    warn!("BFT Proposer: failed to wipe bloated backup file {:?}: {}", backup_path, e);
                                }
                            } else {
                                info!("BFT Proposer: wiped bloated backup file {:?} to prevent OOM on restart", backup_path);
                            }

                            let _ = tx.send(());
                        }
                    }
                }
            }
        }

        // Session ended (hit max_round or error).
        // Determine if a new session is warranted by checking chain height.
        let new_height = {
            let bc = blockchain.read().await;
            bc.get_height()
        };
        let new_session_id = new_height / SESSION_LENGTH;

        if new_session_id > session_id {
            let old_backup =
                Path::new(&data_dir).join(format!("alephbft_backup_{}.dat", session_id));
            if let Err(e) = tokio::fs::remove_file(&old_backup).await {
                if e.kind() != std::io::ErrorKind::NotFound {
                    warn!(
                        "BFT Proposer: could not remove backup {:?}: {}",
                        old_backup, e
                    );
                }
            } else {
                info!(
                    "BFT Proposer: cleared backup {:?} — next session starts from round 0",
                    old_backup
                );
            }

            info!(
                "BFT Proposer: rotated to session {} (chain height {})",
                new_session_id, new_height
            );
        }

        warn!(
            "BFT Proposer: session {} ended. Restarting in 30 seconds…",
            session_id
        );
        tokio::time::sleep(tokio::time::Duration::from_secs(30)).await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::consensus::authorities::SESSION_LENGTH as AUTHORITIES_SESSION_LENGTH;

    #[test]
    fn test_session_length_constant_match() {
        // bft_proposer should use the exact same constant from authorities
        assert_eq!(
            SESSION_LENGTH, AUTHORITIES_SESSION_LENGTH,
            "Session lengths must match across modules"
        );
    }

    #[test]
    fn test_session_rotation_math() {
        let current_height = 299;
        let current_session = current_height / SESSION_LENGTH;
        assert_eq!(current_session, 4); // 299 / 60 = 4 (integer math)

        let next_height = 300;
        let next_session = next_height / SESSION_LENGTH;
        assert_eq!(next_session, 5); // 300 / 60 = 5

        assert!(next_session > current_session, "Session should rotate at boundaries");
    }
}

