# Changelog

## [v2.5.0-alpha] - 2026-07-22

### Fixed
- **State-Healing Hard Fork**: Executed an irregular state change at block `110,000` to mathematically truncate microscopic non-deterministic dust (under 1000 microunits) from all account balances. This perfectly synchronizes all validators, allowing the network to permanently resolve the epoch pool bug divergence without a database wipe.
- **State Root Security Restored**: Removed the permanent exemption. `state_root` validation is now strictly enforced for all blocks `>= 110,000`.
- **Network Isolation**: Bumped `PROTOCOL_VERSION` to `33` and `TESTNET_MAGIC` to `QT33` to cleanly isolate upgraded nodes on the new state.

## [v2.4.33-alpha] - 2026-07-22

### Fixed
- **State Root Exemption Permanent (No-Wipe)**: Removed the upper bound on the state root exemption window for blocks `>= 100,000`. Since the network operators cannot perform a testnet wipe, the state divergence caused by the v2.4.26 epoch pool bug is permanent. Disabling state root validation from 100,000 onward allows the network to continue forming consensus without halting at arbitrary block heights.
- **Network Isolation**: Bumped `PROTOCOL_VERSION` to `32` and `TESTNET_MAGIC` to `QT32` to cleanly isolate upgraded nodes.

## [v2.4.32-alpha] - 2026-07-21

### Fixed
- **BFT Block Time Regression (Reverted v2.4.31 Targeted Backoff)**: Reverted the `unit_creation_delay` targeted backoff introduced in v2.4.31. The backoff (constant 500ms for `t < 100`, then linear up to 10s per round thereafter) caused block finalization time to balloon from ~6s to 30s+ in normal operation — the same root cause as v2.4.27's naive backoff. AlephBFT requires multiple DAG rounds per block, and delaying the first unit in any round (even at round 100) compounds across all rounds. CPU spike protection during genuine network partitions is already handled by the 600s session watchdog. Restored a strict constant 500ms `unit_creation_delay` for all rounds.

## [v2.4.31-alpha] - 2026-07-21

### Fixed
- **State Root Convergence Fix**: Fixed the non-deterministic `load_block` behavior in epoch pool distribution. The previous implementation skipped the current block during live processing but included it during replay, causing permanent state divergence at epoch boundaries. The logic now handles the current block deterministically without relying on local storage latency.
- **Reward Visibility**: Added the `EpochRewardDistributed` contract event so that block explorers like QuaScan can index and display epoch pool distributions.
- **Extended Exemption**: Bumped the state root exemption window to block 105,000 to allow nodes to survive the past state divergence and continue forming consensus while upgrading to the fix.
- **Network Isolation**: Bumped `PROTOCOL_VERSION` to `30` and `TESTNET_MAGIC` to `QT30` to cleanly isolate upgraded nodes.

## [v2.4.30-alpha] - 2026-07-20

### Changed
- **Network Isolation (Protocol v29 / QT29)**: Bumped `PROTOCOL_VERSION` to `29` and `TESTNET_MAGIC` to `QT29` to cleanly isolate nodes running the v2.4.29 constant-500ms `unit_creation_delay` fix from any nodes still running the v2.4.27 linear backoff (5s at `t=0`). Consensus timing behaviour changed — network isolation required to prevent mixed-speed DAG unit creation across validators, which can degrade finalization for the whole committee.

## [v2.4.29-alpha] - 2026-07-20

### Fixed
- **BFT Block Time Regression (Reverted v2.4.27 Backoff)**: Reverted the `unit_creation_delay` linear backoff introduced in v2.4.27. The backoff (5000ms at `t=0`, then linear up to 10s) caused block finalization time to balloon from ~6s to 30s+ in normal operation. Root cause: AlephBFT requires multiple DAG rounds per block, and delaying the first unit in each round by 5s compounded across rounds. CPU spike protection during genuine network partitions is already handled by the 600s session watchdog — the per-round backoff was unnecessary and actively harmed throughput. Restored a constant 500ms `unit_creation_delay`.

## [v2.4.28-alpha] - 2026-07-20

### Fixed
- **State Root Exemption Window (Critical)**: Extended the soft-fork state root validation exemption from blocks `100,000-101,017` to `100,000-102,000`. The non-deterministic HashMap iteration bug in the V3 epoch pool distribution affects **every** block produced by old (pre-v2.4.26) nodes since height 100,000, not just the initial fork at block 101,017. The extended window gives fixed nodes sufficient time to take over block production and re-establish a canonical deterministic state root.

## [v2.4.27-alpha] - 2026-07-20

### Fixed
- **BFT CPU Spike (Consensus Stall)**: Fixed an issue where the node's CPU usage would slowly climb to 300%+ when blocks were failing to finalize for several minutes. The root cause was an aggressive `500ms` constant delay in the `unit_creation_delay` configuration, which caused the DAG to explode in size (thousands of empty units) and overload the AlephBFT graph processing engine. Implemented a linear backoff capped at 10 seconds to drastically reduce CPU pressure during network partitions, while keeping recovery fast.

## [v2.4.26-alpha] - 2026-07-20

### Fixed
- **State Root Non-Determinism**: Fixed a critical bug in `blockchain.rs` where the `EPOCH_POOL_ADDRESS` remainder dust was distributed using a non-deterministic `HashMap` iteration, causing nodes to diverge and log "Invalid state root" starting at block 101,017. The validator tally is now explicitly sorted alphabetically before distribution.
- **Consensus Hotfix**: Added a soft-fork exemption for blocks `101000` through `101017` to allow nodes to sync seamlessly past the bugged sequence.

## [v2.4.25-alpha] - 2026-07-18

### Fixed
- **Network Performance**: Fixed a 300%+ CPU spike by throttling the `maintain_peers` reconnect loop from 10s to 30s and limiting concurrent TLS connection tasks to prevent overwhelming the node on startup.
- **Protocol Isolation**: Bumped `PROTOCOL_VERSION` to 25 and `TESTNET_MAGIC` to `QT25` to cleanly isolate upgraded nodes from older nodes stuck in AlephBFT consensus loops (which were spamming hundreds of units per second and contributing to the CPU exhaustion).

## [v2.4.24-alpha] - 2026-07-18

### Added
- **Explorer APIs**: Added several new endpoints and fields to power professional-grade block explorers:
  - Added `GET /api/richlist?limit=100` to retrieve top accounts by total balance.
  - Added `tps` (Transactions Per Second) to `/api/stats`, dynamically calculated from the last 10 blocks.
  - Added `active_validator_count` and `total_staked` (locked QUA) to `/api/stats`.
  - Added `circulating_supply` to `/api/stats` to differentiate minted coins from the max limit.
  - Added `total_fees_pending` to `/api/mempool` to measure network congestion in terms of QUA.

### Fixed
- **Transaction Visibility**: Fixed an issue where Quascan reported transactions as "Not Found" because they lacked the `tx_hash` wrapper. Refactored Node API (`/api/blocks/latest` and `/api/block/:height`) to automatically inject `tx_hash` into response objects.
- **System Transactions**: Modified the database indexer to correctly index System and Treasury transactions so they are fully queryable by the Node API and indexer.

## [v2.4.23-alpha] - 2026-07-18
### Fixed
- **BFT Consensus**: Fixed a critical bug where proposers would crash if they received BFT signatures from validators who unstaked mid-session. The certificate verification now correctly ignores these signatures instead of rejecting the entire block.

## [v2.4.22-alpha] - 2026-07-18
### Fixed
- **Unicast Routing**: Restored `broadcast_aleph_bft` fallback for Unicast messages in `send_aleph_bft_to_validator`. In a hub-and-spoke topology, community nodes (spokes) cannot communicate point-to-point. Without this fallback, Unicast AlephBFT Fetch Requests were permanently dropped, causing the DAG to stall and hit the Watchdog.

## [v2.4.21-alpha] - 2026-07-17
### Fixed
- **AlephBFT Unicast Routing**: Removed an aggressive CPU spike filter that was incorrectly dropping AlephBFT unicast messages. This fixes an issue where validators could not fetch missing DAG units across the P2P network, resolving stalled AlephBFT consensus when the network is not a full mesh.

## [v2.4.20-alpha] - 2026-07-17

### Changed
- **Network Isolation**: Bumped `PROTOCOL_VERSION` to 21 and `TESTNET_MAGIC` to `QT21`. This enforces clean isolation between v19 nodes (which restored the correct wire format) and legacy v18/v15 nodes, eliminating the `unexpected end of file` bincode deserialization errors caused by magic byte collisions across different payload formats.

## [v2.4.19-alpha] - 2026-07-17

### Fixed
- **DAG Corruption Recovery**: Hard forked the AlephBFT session ID to jump from 1361 to 1362 for block heights >= 81664. This was necessary to rescue the network after operators manually deleted their `alephbft_backup_1361.dat` files (to recover from OOM crashes), permanently corrupting the DAG for session 1361.
- **Reverted v18**: Removed the incorrect fix from v18 that broke network compatibility for community nodes.

## [v2.4.17-alpha] - 2026-07-17

### Fixed
- **Consensus Deadlock**: Removed a 30s block generation delay hack that caused AlephBFT to stall and hit the watchdog timeout.
- **Watchdog Recovery**: Extended the watchdog timeout from 120s to 600s to allow AlephBFT enough time to rebuild DAGs during network recovery.
- **Constant Delay**: Removed exponential round-delay from AlephBFT config to keep network speed constant during catchup.

## [v2.4.16-alpha] - 2026-07-17

### Fixed
- **Startup CPU/OOM Recovery**: Added an automatic size check in `bft_proposer.rs` that wipes the AlephBFT backup file *before* opening it if it exceeds 10 MB. This allows nodes that were previously stuck accumulating multi-GB files (due to earlier bugs) to automatically recover and boot without needing manual user intervention to delete the files via `sudo rm`.

## [v2.4.14-alpha] - 2026-07-17

### Fixed
- **Startup CPU/OOM Recovery**: Added an automatic size check in `bft_proposer.rs` that wipes the AlephBFT backup file *before* opening it if it exceeds 10 MB. This allows nodes that were previously stuck accumulating multi-GB files (due to earlier bugs) to automatically recover and boot without needing manual user intervention to delete the files via `sudo rm`.

## [v2.4.13-alpha] - 2026-07-17

### Fixed
- **Unicast Broadcast Storm (CPU/Network Spike)**: Fixed a severe `O(N^2)` network broadcast storm triggered when AlephBFT lost quorum. When validators were missing, AlephBFT frantically tried to send Unicast `Fetch` requests to them. Because they were offline, the network layer fell back to broadcasting these Unicast messages. Furthermore, `handle_aleph_bft_message` mistakenly relayed incoming Unicast messages to the entire network. This resulted in exponential ZSTD-compression task spawns, completely locking up 100% of a CPU core and crashing nodes. Unicast messages are no longer relayed or blindly broadcast as a fallback.

## [v2.4.12-alpha] - 2026-07-17

### Fixed
- **AlephBFT Memory Leak (OOM)**: When the session watchdog terminates a stuck session, it now deletes the `alephbft_backup_{session}.dat` file. This prevents AlephBFT from loading a massive accumulated history of useless DAG units on every restart, which previously caused instant 1.8GB+ RAM usage and 100% CPU lockup leading to VM crashes.

## [v2.4.11-alpha] - 2026-07-17

### Fixed
- **AlephBFT Progressive Backoff**: Implemented a progressive unit creation delay in `bft_proposer.rs`. When the network is stuck, the AlephBFT unit proposal delay scales up from 500ms to 10 seconds. This cuts CPU usage by 20x during network partitions, fixing the 100% CPU utilization that occurred during the 120-second watchdog window.

## [v2.4.10-alpha] - 2026-07-17

### Fixed
- **AlephBFT CPU Spike (Root Cause)**: Added a 120-second session watchdog in `bft_proposer.rs`. When no block is finalized for >120s (quorum lost), the watchdog terminates the AlephBFT session and sleeps 30s before restarting. Root cause: AlephBFT creates a Falcon-512-signed DAG unit every 500ms per node even when stuck — 4 nodes × 2 units/sec = ~8 heavy crypto ops/sec = 80–90% CPU indefinitely. Also added a 30s stuck-backoff in `aleph_data.rs` (`get_data()`) when no block is finalized for >30s.

## [v2.4.9-alpha] - 2026-07-17

### Added
- **Epoch Pool Reward Model**: Activated at block 100,000 (aligned with V3 economics). Instead of the single DAG-race winner getting 100% of the block reward, all block rewards now accumulate in a dedicated `EPOCH_POOL_ADDRESS`. At every epoch boundary (every 1,000 blocks), the entire pool is distributed proportionally to all validators based on their **uptime** (number of blocks they proposed in the epoch). This completely eliminates latency-based reward inequality.
  - `authorities.rs`: Added `EPOCH_REWARD_ACTIVATION_HEIGHT = 100_000`.
  - `blockchain.rs`: Coinbase recipient switches to `EPOCH_POOL_ADDRESS` at activation height. Validation enforces this. Epoch boundary logic reads last 1,000 block proposers from disk and distributes proportionally.
  - `transaction.rs`: Added `debit_account_direct()` to zero out pool after distribution.


### Fixed
- **CPU Spike (0-Peer Bug)**: Added a minimum peer count guard in `bft_proposer.rs`. When the node has 0 connected peers, AlephBFT was being started despite having no way to reach 2/3+1 quorum. The DAG would spin forever calling `get_data()` in a tight loop, burning a full CPU core. The BFT proposer now waits (sleeping 10s between checks) until at least 1 peer is connected before starting a consensus session.


### Changed
- **Network Isolation**: Bumped `PROTOCOL_VERSION` from 18 to 19 and `NETWORK_MAGIC` to `QT19` to cleanly evict unpatched nodes running `v2.4.5-alpha` that were caught in an infinite proposing loop and flooding the network with garbage blocks and AlephBFT messages.

All notable changes to QuantaChain are documented here.

Format: [Keep a Changelog](https://keepachangelog.com/en/1.0.0/)  
Versioning: [Semantic Versioning](https://semver.org/spec/v2.0.0.html)

---

## [Unreleased]

## [2.4.6-alpha] — 2026-07-17

### Fixed
- **BFT Infinite Loop Hotfix:** Fixed an infinite block proposal loop that caused 120% CPU utilization and unbound memory leaks (up to 1.5GB+).
- **Mempool Transaction Type Validation:** Fixed a bug where `add_transaction` accepted completely invalid `Stake` and `Unstake` transactions into the mempool by omitting state-dependent `TransactionType` checks.
- **Proposer Poison Block Prevention:** `create_block_template` now correctly filters out state-invalid transactions (such as redundant stakes or unstakes from inactive validators) *before* including them in a block, preventing honest nodes from proposing blocks that would fail application during consensus.

## [2.4.5-alpha] — 2026-07-16

### Changed
- **Protocol Bump:** Increased protocol version to `18` and network magic to `QT18` to hard-fork away from nodes running the faulty `v2.4.4-alpha` code.

### Fixed
- **Header Buffer OOM Fix**: Bounded the block header sync buffer to 10,000 headers to prevent a memory leak and OOM crash via header spamming.
- **Transaction Signature Pre-verification**: Offloaded `Falcon-512` mempool signature validation to the blocking threadpool *before* acquiring the Blockchain lock. This stops a massive Tokio executor starvation attack caused by spamming invalid transactions.
- **Block Signature Pre-verification**: Offloaded Rayon multi-threaded signature validation inside blocks to the blocking threadpool to prevent freezing the entire Tokio runtime.
- **AlephBFT Message Limit**: Bounded incoming BFT gossip messages to 1MB max.
- **Lock Scope Deadlock Fix**: Fixed a bug where a read lock was artificially held across the blocking Zstd decompression task.

## [2.4.4-alpha] — 2026-07-16

### Changed
- **Protocol Bump:** Increased protocol version to `17` and network magic to `QT17` to hard-fork away from nodes running the faulty `v2.4.3-alpha` code.

### Fixed
- **Decompression Bomb (OOM/CPU Leak)**: Fixed a vulnerability in `deserialize_message` where an eager 8MB allocation for every compressed message caused massive allocator churn and OOM crashes. Buffer now scales dynamically.
- **Queue Memory Exhaustion**: Reduced `message_tx` channel capacity from `10,000` to `1,000` to prevent attackers from hoarding up to 80GB of RAM with backlogged maximum-size blocks.

## [2.4.3-alpha] — 2026-07-15

### Changed
- **Protocol Bump:** Increased protocol version to `16` and network magic to `QT16` to cleanly hard fork away from nodes running the faulty `v2.4.2-alpha` code.

### Fixed
- Fixed the root cause of the TOCTOU stream corruption bug by holding the write lock *during* the timeout evaluation, guaranteeing that partial bytes from timed-out futures cannot be silently interleaved with new messages.

## [2.4.2-alpha] — 2026-07-15

### Changed
- **Protocol Bump:** Increased protocol version to `15` and network magic to `QT15` to hard-fork away from old nodes that contain the TOCTOU streaming bug, protecting the healthy network.

### Fixed
- Fixed a major TOCTOU stream corruption bug in `send_message` causing `Decompression read error` and `Stream corrupted or dead` storms across the network.
- Fixed IP-based dialing suppression in `maintain_peers` which prevented nodes on the same host (e.g. VPS deployments) from meshing successfully.

## [2.4.1-alpha] — 2026-07-15

### Fixed
- Fixed critical P2P network bug where any TCP error resulted in a 100-strike instant IP ban, causing rapid network collapse and BFT stall. Streams are now correctly marked as dead without triggering malicious behavior bans.

## [2.4.0-alpha] — 2026-07-15

### Fixed
- Fixed BFT session restart timestamp reset to prevent 286s slot gate stalls.
- Fixed BFT block production write lock being held across error branches.
- Removed redundant Docker compose CLI args.

## [2.3.9] — 2026-07-15

### Fixed
- **MANDATORY UPDATE (PROTOCOL V14).** Bumped `PROTOCOL_VERSION` to `14` and `NETWORK_MAGIC` to `QT14` to permanently reject all old nodes (v13 and below) that were connecting and corrupting TCP streams.
- Silenced `AlephBFT signature verification FAILED` log spam (demoted to `debug`).
- Silenced `Failed to decode incoming AlephBFT message` log spam (demoted to `debug`).

## [2.3.8] — 2026-07-15

### Fixed
- Fixed critical TCP stream corruption causing massive CPU spikes, decompression errors, and AlephBFT decode panics when a slow peer's read/write operation timed out but the stream remained open.
- Optimized BFT broadcast to immediately skip dead peers instead of spinning up tasks and waiting for timeouts.

## [2.3.7] — 2026-07-15

### Fixed
- Fixed secondary AlephBFT log spam during unicast fallback to disconnected peers.

## [2.3.6] — 2026-07-15

### Fixed
- Fixed BFT broadcast log spam during peer disconnects by demoting the send failure log to `debug`.
- Removed verbose BFT quorum observability logs.

## [2.3.5] — 2026-07-15

### Fixed
- Fixed BFT consensus stall caused by P2P LRU cache dropping AlephBFT retry messages.
- Added verbose BFT quorum observability logs.
- Bumped `PROTOCOL_VERSION` to 13.

## [2.3.4] — 2026-07-15

### Fixed
- Fixed silent handshake failures caused by `Result::is_ok()` dropping errors.
- Fixed P2P connection flapping where nodes continually attempt to reconnect every 10 seconds.
- Added explicit `PROTOCOL_VERSION` bump to force clean network upgrades after network magic changes.

## [2.3.3] — 2026-07-15

### Fixed
- **Network Discovery Fix**: Re-enabled gossiping of inbound connections. This prevents the network from forming a disconnected "star topology" around bootstrap nodes, allowing AlephBFT validators to discover each other and reach consensus.
- **API Bind Fix**: The REST API now securely defaults to `0.0.0.0` (configurable via `api_bind_host` in `quanta.toml`), fixing the issue where LUA and other nodes appeared OFFLINE to block explorers.

### Changed
- **Network Isolation**: Bumped `PROTOCOL_VERSION` from 10 to 11 and `TESTNET_MAGIC` from `Q8TE` to `Q9TE` to cleanly evict un-patched nodes from polluting the consensus layer.
## [2.3.2] — 2026-07-14

### Fixed
- **AlephBFT Backup Loop**: Removed buggy logic that aggressively deleted AlephBFT backup files at session boundaries during restarts. This prevented the node from continuously throwing "Backup state behind unit collection state" errors and infinitely restarting the consensus service.

## [2.3.1] — 2026-07-14

### Fixed
- **Sync Throttling Bug**: Fixed a logic error in `handle_new_block` where the 100-block anti-gossip protection dropped valid sync blocks, artificially capping download speed to 100 blocks per minute. P2P block syncing will now reliably scale up to the `MAX_SYNC_BATCH` limit (5,000 blocks).

## [2.3.0] — 2026-07-14

### Added
- **Validator API Expansion**: `/api/validators` and `/api/validators/:address` now return advanced statistics (uptime, `blocks_signed`, lockup status, and `slash_cooldown_until_epoch`).
- **Dynamic Batching Engine**: P2P block syncing now auto-scales batch sizes from 25 to 5,000 blocks based on exact payload bytes, preventing OOM while drastically improving sync speed on empty networks.

### Changed
- **Tokenomics V3 Target Price**: Changed block emission reward from 50 QUA to 0.5 QUA starting at Block 100,000.
- **Protocol Version Updated**: Bumped `PROTOCOL_VERSION` to `10` to enforce node upgrades before the Tokenomics V3 cutoff height.

## [2.2.12] — 2026-07-14

### Fixed
- **Time Warp Recovery**: Reverted the 15-second block limit back to 2 hours. Because the network's tip was already 2 hours in the future due to the exploit, enforcing a 15-second limit caused all new (healing) blocks to be rejected. The root cause (using chain-time instead of real-time to trigger block creation) remains fixed.
- **DAG Cleanup**: Added logic to automatically clear old `alephbft_backup` files at session boundaries, ensuring validators don't get stuck waiting for parents from aborted runs.

### Changed
- **Network Magic Updated**: Changed network magic to `Q8TE` and protocol version to 9.

## [2.2.11] — 2026-07-14

### Fixed
- **Time Warp Protection**: Fixed a critical vulnerability where blocks with future timestamps could cause the AlephBFT slot gate to stall indefinitely, resulting in a complete network consensus halt. The slot gate now uses local system time to measure elapsed time, and the maximum allowed future timestamp drift for blocks has been reduced from 2 hours (7200 seconds) to 15 seconds.
- **Monotonic Timestamps**: Block validation now strictly enforces that block timestamps must always move forward, preventing DOS loops in data provisioning.

### Changed
- **Network Magic Updated**: Changed network magic to `Q7TE` and protocol version to 8.

## [2.2.10] — 2026-07-13

### Changed
- **Sybil Limit Increased**: Increased the maximum connections allowed from a single IP from 2 to 100 to allow multiple validators to run on the same VPS without being banned.
- **Network Magic Updated**: Changed network magic to `Q6TE`.

## [2.2.9] — 2026-07-13

### Added
- **Devnet Mode**: Added `--devnet <ID>` and `--devnet-nodes <N>` to auto-bootstrap deterministic Devnet testing instances without manual wallet configuration.
- **Docker Auto-Devnet**: Removed the need for pre-generated `genesis.json` and static wallets for Devnet orchestration.

### Fixed
- **Merkle Proof Bug**: Fixed a bug where odd-sized SPV subtrees would fail verification due to a midpoint rounding error in `collect_proof`. Proof generation now strictly mirrors the `ceil(n/2)` split logic used in `build_tree`.

## [2.2.7] — 2026-07-08

### Fixed
- **BFT Consensus Freeze (block production stopped)**: Fixed a critical write-lock deadlock in `Peer::send_message` introduced in v2.2.6. The `write_half` RwLock was acquired *before* the timeout started, meaning one slow/congested peer held the lock for up to 60 s and starved all AlephBFT message delivery to *every other* peer. Consensus round-trips timed out, nodes showed Online but produced zero blocks. Lock acquisition is now inside the timeout future so it is correctly cancelled on expiry. Timeout also reduced from 60 s → 10 s.
- **Log Spam (binary blob output)**: `P2PMessage::AlephBFTMessage` was debug-printed with `{:?}` in two hot paths (receive loop and error handler), dumping hundreds of raw byte integers per message into operator logs. Replaced with compact human-readable labels: `AlephBFT(342 bytes)`, `Block(#1234)`, `NewTx(abcd1234)`.

---

## [2.2.6] — 2026-07-07

### Fixed
- **Peer Memory Leak (OOM)**: Added a 5-second timeout to `Peer::send_message` to prevent the `broadcast` tokio tasks from hanging indefinitely when a peer's TCP buffer is full and they stop reading. Stalled connections are now immediately closed.

### Security
- **Network Isolation**: Bumped `PROTOCOL_VERSION` from 5 to 6 and `TESTNET_MAGIC` from `Q4TE` to `Q5TE` to evict nodes running the unpatched v2.2.5 software that were causing network instability and RAM leaks.

---

## [2.2.5] — 2026-07-07

### Added
- **Validator Connectivity API**: Added `/api/validators` to expose live BFT connectivity (`is_online`) and peer protocol version (`node_version`).
- **Stats API Upgrade**: Added `current_session` and `blocks_until_next_session` to `/api/stats` to accurately reflect the 60-block BFT activation boundaries.

### Fixed
- **Network Flapping (early eof)**: Implemented deterministic tie-breaker (via `node_id` comparison) to resolve outbound/inbound TCP loop collisions.
- **Peer Memory Leak (OOM)**: Bounded concurrent Tokio tasks using `tokio::sync::Semaphore` and implemented strict AlephBFT broadcast deduplication to halt storm loops.
- **Discovery Deadlock**: Added `dedup()` to the peer discovery loop to prevent aggressive self-connection attempts.
- **Wallet Message**: Corrected staking confirmation message to explicitly mention 60-block "session boundaries" rather than 1000-block "epoch boundaries".

### Security
- **Network Isolation**: Bumped `PROTOCOL_VERSION` from 4 to 5 and `TESTNET_MAGIC` from `Q3TE` to `Q4TE` to evict legacy v2.2.0 nodes from polluting the consensus layer.

---

## [2.2.0] — 2026-07-06

### Security
- **P2P Sync Vulnerability Patched**: Required BFT proposer signatures on all blocks to mitigate a critical Sybil network sync vulnerability.
- **API DoS**: Capped transaction history lookups (`/api/address/:address/txs`) to 1000 blocks to prevent asynchronous executor starvation.
- **Peer Memory Leaks**: Enforced strict `lru::LruCache` constraints on PeerManager banned IPs (5000 max) and PeerDiscovery known peers (5000 max).
- **Staking Exploit**: Added unbonding guards to `register_validator` to prevent stakes from being inadvertently burned.

### Added
- **CLI Version Output**: Expose version from the node binary via `quanta -V` and `quanta --version`.

## [2.1.2-alpha] — 2026-07-06

### Changed
- **Hard Reset**: Transitioned network to a 4-core validator set due to unresponsive nodes.
- **Validator Registration**: Set `OPEN_VALIDATOR_REGISTRATION_HEIGHT` to `0` to allow standby validators to join instantly.
- **Network Magic**: Updated `TESTNET_MAGIC` to `Q2TB` to enforce the network wipe and prevent old nodes from connecting.

## [2.1.1-alpha] — 2026-07-06

### Fixed
- **Consensus Halt**: Fixed a consensus state root mismatch bug where the block proposer would omit validator staking/unstaking operations during the block generation phase but evaluate them during validation. 

### Changed
- **Soft Update**: Reverted `TESTNET_MAGIC` back to `Q2T9` and added a hardcoded bypass for block 12615 to allow the network to cleanly recover from the state root consensus bug without a data wipe.

## [2.1.0-alpha] — 2026-07-05

### Added
- **JSON-RPC Guide**: Added a comprehensive guide on using the JSON-RPC endpoints.

### Fixed
- **BFT Session Termination**: Replaced `return None` with an async wait loop in AlephBFT `get_data` to ensure the consensus session isn't prematurely killed.
- **Backup Deletion**: Fixed an issue causing "Backup state behind" errors by exclusively wiping BFT backups on session transitions instead of node restarts.
- **BFT Start Race Condition**: Added a sync-wait loop ensuring a node finishes downloading missing blocks before initiating an AlephBFT session, stopping older sessions from flooding the network.
- **Network Partitioning Cascades**: Reduced peer ban penalty for connection failures from 24 hours to 60 seconds to prevent nodes from permanently banning each other during sequential rolling restarts.
- **Database Pruning**: Fixed underlying mechanics behind database pruning.

### Removed
- **Dead Code**: Cleaned up significant dead code and orphaned modules across the consensus, network, core, and RPC implementations.

---

## [2.0.2-alpha] — 2026-07-04

> **TESTNET WIPE REQUIRED.** Genesis block has been modified.
> Deploy by deleting `quanta_data` folder on all nodes before restarting.

### Changed
- `MAX_COMMITTEE_SIZE` increased to `21` to allow dynamic expansion of the active validator set.
- Genesis validators removed from the liquid `testnet_faucets` array. This ensures genesis validators only receive locked stake and no liquid QUA, mathematically preventing Sybil attacks.

### Fixed
- **AlephBFT unicast routing (bandwidth critical)** — `QuantaNetworkBridge::send()` was
  broadcasting every `Recipient::Node(idx)` message to ALL peers instead of routing it
  to the single intended validator. For an N-node committee this caused O(N²) bandwidth
  blowup — every vote, signature, and DAG unit was sent (N-1)× more than necessary.
  At 7 nodes this was the dominant source of the observed ~15 GB/day traffic.
  **Fix:** `send()` now inspects `Recipient` and calls `send_aleph_bft_to_validator()`
  for `Node` targets, routing to a single TCP peer. Falls back to broadcast if the target
  is temporarily disconnected. Estimated reduction: ~80% of total BFT traffic.
- **Peer flapping loop (`Connection reset by peer`)** — Dead peers held the IP slot in
  `PeerManager` for up to 180 s after TCP stream death. Every reconnect attempt during
  that window was rejected, causing "Connection reset by peer" → immediate retry every
  10 s → up to 18 failed handshakes per dead link, each triggering a full mempool
  transfer on the flapping pair.
  **Fix (FLAP-1):** `add_peer()` evicts stale peers (last_seen > 30 s) instead of
  hard-rejecting. **Fix (FLAP-2):** Rejected inbound peers receive an explicit
  `Disconnect` before the stream drops so the remote backs off gracefully.
  **Fix (FLAP-3):** `maintain_peers` `is_connected` guard now also checks `is_alive()`
  so dead peers no longer block outbound reconnect attempts indefinitely.
- **Heartbeat 6× too frequent** — heartbeat task used hardcoded `Duration::from_secs(10)`
  while `protocol.rs` defines `PING_INTERVAL_SECS = 60`. Fixed to use the protocol
  constant, reducing Ping/Pong traffic by ~83%.
- **`GetMempool` fired on every peer reconnect** — previously unconditional on each
  `connect_to_peer()` call. Combined with the flapping bug, this caused full mempool
  transfers at ~10 s intervals on flapping node pairs. Now guarded: only fires if the
  local mempool is empty.

### Changed
- **`node_id` is now the validator wallet address** — previously a random UUID, now set
  to the validator's wallet address at startup. This enables the AlephBFT bridge to
  resolve `NodeIndex → peer TCP connection` for unicast routing. Non-validator (observer)
  nodes retain a random UUID and are unaffected.

### Performance

| Metric | Before | After (est.) |
|---|---|---|
| Daily bandwidth / node (7 validators) | ~5 GB | ~1 GB |
| Peer reconnect storm duration | up to 180 s | < 30 s |
| Heartbeat messages / hour (7-node cluster) | ~360 | ~60 |

---

## [2.0.1] — 2026-07-02

> **TESTNET RESET — All nodes must wipe their database and restart from the new genesis.**
>
> This release includes major updates to the consensus engine, block size optimizations,
> Smart Contracts V3 (AI layer), and full validator staking. It also includes the previously
> required genesis reset replacing lost validator wallets and fixing block timing drift.

### Fixed
- **AlephBFT block timing drift (critical)** — `create_block_template` used
  `timestamp = max(now, prev+1)`. When AlephBFT finalised blocks faster than 1 block/sec,
  this incremented the chain timestamp on every block. After N fast-finalised blocks the
  chain timestamp was N seconds ahead of wall-clock time, making the 6-second gate in
  `aleph_data.rs` (`current_time < last_ts + 6`) permanently `true` and stalling block
  production entirely. Symptoms: fast blocks → progressively slower → near-stuck.
  **Fix:** `timestamp = min_ts if min_ts <= now else now` — timestamps are now hard-capped
  to wall-clock time and can never drift ahead of real time.
- **License header missing** — `LICENSE` file contained only raw AGPLv3 boilerplate with no
  QuantaLabs copyright notice or dual-license clarification. Fixed by prepending the proper
  project-level header.
- **AlephBFT 30s block time compounding delays** — Resolved by fixing round-robin proposer timeout and block time compounding logic. All validators now propose at 6s slot open, returning block time to ~6s.
- **Round-robin tx distribution and mempool propagation** — Fixed block time, round-robin distribution, and mempool issues.
- **`handlers.rs` compile errors** — Resolved by awaiting the blockchain lock correctly.
- **`EscrowInitArgs` missing field** — Added `refund_height` to fix `deploy-escrow` compilation.

### Changed
- **Genesis timestamp** updated from `1748736001` (2025-06-01) to `1780704001` (2026-06-06).
  This changes the genesis block hash, forcing all nodes to perform a clean wipe-and-resync.
- **Testnet genesis hash** updated: `ae37fe2f40a7e7dbe6d2d1337f260d57185ef5fb169008e2600f245809fd1fbf`
  (was `48119d35c293531f1438b29a50d674575e4d5002e789699fe8efbd955eea2115`).
- **Network magic bytes** updated to `Q2T4` — isolates the new network from old nodes.
- **Validator 5 wallet replaced** — the original validator wallet (`0x822dd149...`) was lost.
  Replaced with new wallet `0x591277eb458e3185bef4fcf18c1c7136fb8bbad6` in both `genesis.json`
  and `blockchain.rs`. Old `gentx5.json` deleted.
- **Genesis faucet wallets rotated** (all 10) — new HD wallet generated on 2026-06-06 using
  `cargo run --bin gen_faucet_wallets`. Encrypted backup saved to `faucet_wallet.json`.
  Faucet 0 (API sender): `0xec4f49553e31f22b27a83036a044aff7d697f524`.
- **Network magic bytes bumped to `Q2T6`** — Isolates the new v2.0.1 network from older nodes due to the hard fork introduced by increased block capacity and Smart Contracts V3.
- **Block size and tx capacity increased** — Block size increased 2MB → 4MB, maximum transactions per block increased 1200 → 2000, and max P2P message size increased 4MB → 8MB to double the TPS ceiling to ~400 TPS.

### Added
- **`src/bin/get_testnet_hash`** — new binary that calls `Block::genesis()`, prints all
  structural fields and the deterministic genesis hash, and verifies it is reproducible.
  Run after any change to `block.rs` to get the updated `TESTNET_GENESIS_HASH`.
  `cargo run --bin gen_faucet_wallets`. Encrypted backup saved to `faucet_wallet.json`, prints both address arrays for
  `blockchain.rs`, and prints the genesis hash.
- **`QUANTA_WALLET_PASSPHRASE` env var** support in `quanta-wallet restore` — optional BIP39
  25th-word passphrase support, defaults to `""` (backward compatible with all existing wallets).
- **Smart Contracts V3** — Complete AI contract layer + contract API endpoints (5 native templates: Escrow+refund, AgentJob+deadline+refund, AgentBid multi-agent auction, Stream pay-per-block, AgentRegistry). Added `ContractEvent` logs.
- **Validator Staking & Slashing** — Full staking, slashing, unbonding, and an open validator registration switch.
- **Deterministic HD Wallet Key Derivation** — Falcon-512 HD wallet keypairs are now derived deterministically from the account seed.
- **`setup-validator.sh` script** — Added a secure validator setup script for easy deployment with dry-run mode.
- **`show-mnemonic` command** — Added to the `quanta-wallet` CLI.

---

## [2.0.0-alpha] — 2026-06-01

> **CRITICAL NETWORK UPDATE (v2.0.0-alpha)**
> 
> We have released a mandatory consensus-critical update. This update replaces the legacy consensus engine with the new **AlephBFT** consensus engine and includes critical fixes for block timing and network broadcast storms.
> 
> **Action Required for All Node Operators:**
> To prevent old nodes from connecting to the new consensus network, we have updated the network magic bytes to `Q2T2`. 
> 
> 1. You **must** pull the latest code.
> 2. You **must** completely wipe your old `quanta_data` directory (hard network reset).
> 3. Restart your node.
> 
> Nodes running older versions will no longer be able to connect to the network.

### Added
- Replaced legacy PoW and Tendermint-style BFT with AlephBFT consensus engine.
- Hardcoded `Q2T2` magic bytes to isolate the V2 network.
- Fixed BFT block timing stalls and broadcast bugs in `QuantaNetworkBridge`.

---

## [0.7.5-alpha] — 2026-05-08

> **CONSENSUS-CRITICAL. All nodes must upgrade. No testnet reset required.**
> Fixes the persistent "Invalid state root" errors at block 91,096, stale mining,
> and nonce corruption after every reorg. Adds the block 90,000 checkpoint.

### Fixed
- **State root mismatch at block 91,096 (root cause fix)** — `create_block_template`
  and `validate_block_consensus` both computed the state root without first calling
  `unlock_mature_coinbase(index)`. At block 91,096 (exactly `COINBASE_MATURITY=100`
  blocks after heavy bootstrap mining at ~90,996) locked coinbase entries matured,
  causing the two sides to hash structurally different account states. Both paths now
  call `unlock_mature_coinbase` before applying transactions and computing the hash.
- **Invalid nonce after every reorg ("expected 5, got 1")** — the `pending_nonces`
  DashMap was not cleared on reorg. After a fork discarded txs from the abandoned branch,
  the map still held nonce=4 from those erased txs, causing the next canonical-chain
  block (nonce=1) to be rejected as "expected 5". All three reorg paths now call
  `pending_nonces.clear()` or a stale-nonce sweep after every chain switch.
- **All mined blocks stale (abort-on-new-block)** — `block.mine()` ran an
  uninterruptible PoW loop. Miners could not stop even when a peer block arrived,
  wasting an entire 30 s block interval. Added `Block::mine_with_cancel(&AtomicBool)`
  which polls a cancel flag every 10,000 hashes (~10 ms). The mining loop now subscribes
  to a `watch::Sender<u64>` that fires on every accepted block and aborts within ~10 ms.
- **Stale-nonce sweep in normal block accept** — after any non-reorg block accept,
  `pending_nonces` now evicts all entries where the cached nonce ≤ the confirmed
  chain nonce, preventing accumulation of stale entries over time.
- **Unicode escape compile error in server.rs** — `\u2014` (JavaScript syntax)
  replaced with `--` (Rust requires `\u{2014}` brace syntax).

### Added
- **Checkpoint at block 90,000** — verified live from `scan.quantachain.org` on 2026-05-08:
  `(90_000, "000000dc0e178a5140a5c68481234a9541373ac349b1ae3cbc3f0f3f1fc58d5e")`
  Anchors the `STATE_ROOT_SORT_FIX_HEIGHT` boundary; all nodes must be on v0.7.5+ to
  sync past this height.
- **New-block notification channel** (`watch::Sender<u64>`) — `Blockchain` now exposes
  `subscribe_new_blocks()`. Any subsystem (mining, future getblocktemplate RPC) can
  subscribe and be notified within one async tick when the chain moves.
- **`Block::mine_with_cancel()`** — cancellable PoW variant; returns `true` (found
  nonce) or `false` (cancelled). Used by the mining loop to abort instantly.

### Changed
- **Falcon-512 signing unified under `falcon-rust`** — `FalconKeypair::sign_raw` now
  uses `falcon_rust::sign` instead of `pqcrypto_falcon::sign`. All native signing paths
  (CLI wallet, faucet, benchmarks, tests) now produce byte-identical output to the
  browser WASM wallet, eliminating the cross-library format ambiguity documented in
  `FALCON_SIGNING_INTERNALS.md`. Key generation still uses `pqcrypto-falcon` (the
  authoritative NIST reference C implementation). Public keys are cross-compatible.
  `pqcrypto-falcon` and `pqcrypto-traits` remain in `Cargo.toml` (still needed for
  key generation and Kyber-1024 wallet encryption).
- **Mining loop delay reduced** from 100 ms to 10 ms between attempts — the watch
  channel now drives restarts, so a fixed delay is no longer needed for responsiveness.

---

## [0.7.4-alpha] — 2026-05-06

> **Chain-sync compatibility + sync-stuck patch. No testnet reset required.**
> Fixes block 84,812 nonce incompatibility, state root corruption zone, and a sync
> loop where nodes reported "Already on heaviest chain" while 20+ blocks behind.

### Fixed
- **Pre-checkpoint nonce override (block 84,812 compat)** — block 84,812 contains
  a TX with nonce=1 from `0xf23f...` but a clean-sync node expects nonce=5. Root cause:
  the reorg that produced this block ran under the v0.7.2 snapshot-fallback bug and
  rebuilt state without the sender's earlier 4 transactions. Fix: for blocks below the
  highest hardcoded checkpoint, nonce mismatches override `temp_state` to the block's
  claimed nonce instead of rejecting. Full enforcement still applies at/above checkpoint.
  Added `AccountState::set_nonce()`.
- **State root skip height raised 85,000 → 90,000** — blocks 85,000–89,999 were mined
  while the main node had corrupted account state (from the v0.7.2 reorg bug). Sequential-
  sync nodes can never reproduce those state roots. Raising `STATE_ROOT_SORT_FIX_HEIGHT`
  to 90,000 skips state root validation for the entire damage zone. A new checkpoint at
  90,000 will be added once the main node restarts on v0.7.4 and reaches that height.
- **`cumulative_work_at` off-by-one in slow path** — `for h in 0..tip_height` excluded
  the block AT `tip_height`, making every deep-reorg reset cumulative_work ~8.3M too low.
  Over repeated reorgs this drift caused the sync peer-selection to see local work ≥ peer
  work even when 20 blocks behind. Fixed to `0..=tip_height` (inclusive).
- **Sync stuck: "Already on heaviest chain" when 20 blocks behind** — the peer selection
  in `sync_blockchain` compared cumulative_work only. When local work drifted above the
  peer's (due to the off-by-one above), no peer was selected and sync silently stopped.
  Fix: if any peer is >5 blocks ahead by height, always select it for sync regardless of
  cumulative_work comparison (`far_ahead` safety-net path).
- **Mempool cleanup by `(sender, nonce)` in addition to tx hash** — confirmed transactions
  were staying in the mempool when the tx hash comparison failed due to `public_key`
  serialization differences between the mempool submission path and P2P block path.
  `pending_transactions.retain` now also evicts when `(sender, nonce)` matches any mined tx.
- **Faucet duplicate-nonce race condition** — rapid concurrent faucet claims both read
  the same confirmed nonce and submitted identical nonce values; the second was silently
  rejected by the node. Added a module-level async mutex and in-memory pending-nonce
  tracker to serialise claim submissions in `quanta-web/app/api/faucet/route.ts`.

---

## [0.7.3-alpha] — 2026-05-06

> **Sync stability patch. No testnet reset required.**
> Nodes on v0.7.2 may get stuck during reorg due to an O(n) Sled scan per reorg cycle
> and an incorrect LWMA bounds check during replay. Drop-in upgrade — no data wipe.

### Fixed
- **O(n) Sled scan in `deep_reorg` (CRITICAL)** — `base_work` was recalculated by
  reading every block from 0 to `rollback_to` from Sled. At height 85k with a 5-block
  rollback this was 85,000 sequential reads while holding the write lock, causing 30–60s
  stalls and peer timeouts that logged as `"Reorg failed: Invalid block"` then retried
  infinitely. **Fix:** replaced with `cumulative_work_at(rollback_to)` — O(1) in-memory read.
- **Wrong LWMA bounds check during reorg replay** — `validate_block_consensus_reorg`
  called `calculate_next_difficulty()` on a partially-rebuilt chain (incomplete LWMA window),
  producing a wrong estimate that rejected valid peer blocks as „outside ±50% LWMA bounds“.
  **Fix:** removed the LWMA bounds check from the reorg path; `has_valid_hash()` PoW
  already proves real work, `MIN_DIFFICULTY` still guards the floor.
- **Snapshot fallback skipped all blocks in `rebuild_account_state_up_to`** — when a
  1000-block snapshot was missing, the code fell back to genesis-only state but then set
  `replay_start = snapshot_height + 1`, silently skipping all blocks 1…snapshot_height.
  Every reorg block subsequently failed with insufficient balance / wrong nonce.
  **Fix:** when no snapshot is loaded `replay_start` is always `1`.

### Added
- **Checkpoint at block 85,000** — verified live from `scan.quantachain.org` on 2026-05-06:
  `(85_000, "0000007305d4ceeaf72a4f3c58001295a335d588e16a05f037d21dfb21ac06ca")`
  Anchors the `STATE_ROOT_SORT_FIX_HEIGHT` boundary; prevents deep reorgs into
  pre-sort-fix territory.

---

## [0.7.2-alpha] — 2026-05-05

> **CONSENSUS-CRITICAL patch. No testnet reset required.**
> All nodes must upgrade. Nodes on v0.7.1 will diverge from upgraded nodes at any block
> containing a TimeLock credit to the miner's address above height 85,000.
> Existing `quanta_data/` directories are fully compatible — drop-in upgrade.

### Fixed
- **State root determinism (`calculate_state_root`)** — `locked_balances` is a `Vec`
  whose insertion order differs between `create_block_template` (coinbase credited first)
  and `validate_block_consensus` (user txs applied first, coinbase in a second pass).
  This caused a deterministic hash mismatch whenever a block contained a `TimeLockTransfer`
  credit to the same address as the miner's coinbase — the `locked_balances` vec was
  identical in content but different in order, producing a different SHA3-256 state root.
  **Fix:** `calculate_state_root` now sorts `locked_balances` by `(unlock_height, amount)`
  before hashing — result is order-independent on every node regardless of apply order.

### Added
- **`STATE_ROOT_SORT_FIX_HEIGHT = 85_000`** — blocks below this height skip state root
  validation (they are secured by hardcoded checkpoints). This avoids re-validating the
  ~80k already-committed blocks under the new sort rule, which would fail for the small
  number of historically mismatched roots.
- **Checkpoints extended to block 80,000** — three new testnet checkpoints verified live
  from `scan.quantachain.org` on 2026-05-05:
  - `(60_000, "0000010ce22920660ba1e42423ea46e76dc7582963d6f9f220e3930031bd9bc9")`
  - `(70_000, "000001fcb0637b06601b4f111b22070e856c8cabf2eaa545c41b938b4478d186")`
  - `(80_000, "0000002d80e66bce37596616a9c9c3c1988da6e65811ad132926162c7e000a0e")`

---

## [0.7.1-alpha] — 2026-04-10

> **No testnet reset. Drop-in upgrade from v0.7.0.**

### Fixed
- `add_block_to_main_chain_reorg` called the strict `validate_block_consensus` instead
  of the permissive `validate_block_consensus_reorg` — peer reorg blocks were rejected
  if their difficulty didn't exactly match our local LWMA
- `deep_reorg` did not reset `cumulative_work` before replaying new blocks — the stale
  old-chain work value caused double-counting, making the node think it always had more
  work than peers and skip future syncs
- `reorg_to_block` (single-block tip swap) never updated `cumulative_work` — counter
  was left at the old tip's value after every shallow reorg
- `add_block_to_main_chain_reorg` had a dangling orphan code block (missing
  `for tx in &block.transactions` loop header) — compile error on affected builds
- `sync_blockchain`: `request_start <= bc_height` changed to `request_start < bc_height`
  — linear sync was incorrectly triggering `deep_reorg` when downloading the next
  sequential block, causing O(n²) account-state rebuilds during IBD

### Changed
- `save_block` and `save_account_state` no longer call `db.flush()` after every write —
  sled WAL guarantees crash safety; a single flush is issued at end of sync batch and
  after mining. Removes ~90 s of wasted fsync IO during an 18k-block IBD
- `cumulative_work` is now persisted in sled and cached in memory (`Arc<Mutex<u128>>`) —
  `cumulative_work_at(tip)` is O(1) for the current tip; previously O(height) disk scan
  while holding the blockchain read lock (primary cause of seed-node connection timeouts)
- Account state snapshots saved every 1000 blocks — `rebuild_account_state_up_to()`
  loads the nearest snapshot and replays only the delta instead of always from genesis

---

## [0.7.0-alpha] — 2026-04-08

> **Testnet reset required.** Wire format changed (cumulative_work in handshake/Height).

### Added
- `GetHeaders` / `Headers` P2P messages — headers-first sync (Bitcoin IBD architecture)
- `header_buffer` in the sync engine — collects headers before planning block downloads
- `sync_request_range` — exact [start, end] range set before GetBlocks so
  `handle_new_block` can buffer reorg blocks whose index is below the current tip
- `cumulative_work: u128` field in `P2PMessage::Height` and handshake
- `validate_block_consensus_reorg()` — permissive difficulty validator (50% LWMA bounds)
  for reorg replay path; full sig verification included
- `network_id: u32` field in `Transaction` — included in signing bytes and hash,
  cryptographically binding signatures to a specific network (replay protection)
- Inbound connection cap in `listen_for_connections()` — enforces `max_peers` before
  accepting the TCP stream (botnet exhaustion protection)
- Keep-alive ping during block download (every 15 s) — prevents seed from closing
  idle-looking connection during slow batch transfer
- Sub-batch block serving in `handle_get_blocks` (20 blocks + `yield_now`) — releases
  read lock between sub-batches so other tasks (mining, heartbeat) can run on seed
- Partial-batch guard — skips `deep_reorg` if received block count ≠ expected
- Atomic deep reorg with full rollback — saves original chain before rollback, restores
  on failure; node never left at a partial intermediate state
- `seen_blocks` / `seen_txs` LRU dedup caches — prevents broadcast storms
- `sync_request_range` mutex — allows reorg blocks (index ≤ tip) to be buffered during sync

### Fixed
- State root empty-string bypass — `state_root = ""` could skip state root validation
- Reorg path skipped transaction signature verification entirely
- Duplicate serial signature verification — `block.is_valid()` ran a redundant serial
  Falcon-512 pass before the parallel Rayon pass in `validate_block_consensus()`
- `maintain_peers` spammed new TCP connections to seed during active sync — triggered
  Sybil-protection rejection every 10 s

### Changed
- `broadcast_block()` now sends only the block header (~200 B) instead of full block
  (~2 MB) — O(peers × 200 B) instead of O(peers × 2 MB) per block
- `network_id` propagated from node config — coinbase/treasury txs use
  `self.network.network_id()` instead of hardcoded `0`
- Header wait timeout increased from 10 s to 30 s
- Block batch size reduced from 100 to 50; idle timeout increased from 30 s to 60 s
- `handle_get_headers`: running cumulative-work sum instead of O(height) per-header
  call (was causing 36M sled reads per 2000-header batch at height 18k)
- `maintain_peers` skips new connection attempts while `syncing` flag is set

---

## [0.6.0-alpha] — 2026-04-05

### Fixed
- Block template nonce sequence — mempool assembler sorted by fee without enforcing
  nonce ordering; blocks could be rejected with `InvalidNonce` at consensus
- Permanent nonce desync on reorg — `reorg_to_block` did not call `increment_nonce()`
- Faucet balance zero after sync — genesis premine applied in memory but not stored
  in genesis block struct; `rebuild_account_state_up_to()` replayed an empty tx list
- Sync stuck at block 1 — `MIN_DIFFICULTY` constant was above early testnet block
  difficulties, causing every incoming block at those heights to be rejected

---

## [0.5.0-alpha] — 2026-03-28

### Added
- LWMA (Linearly Weighted Moving Average) difficulty algorithm — adjusts every block
  on a 45-block sliding window (~22.5 min); replaces 2016-block Bitcoin-style intervals
- `deep_reorg()` — multi-block chain reorganisation engine
- Parallel Rayon signature verification with LRU cache (1800 ms → ~300 ms per block)
- Bloom filter for O(1) mempool duplicate detection
- Atomic orphan pool (`VecDeque` for O(1) pop-front)

---

## [0.3.0-alpha] — 2026-03-20 (Testnet V2)

### Added
- Testnet V2 genesis reset with realistic difficulty (6,972,889)
- Bitcoin-style DoSMan weighted peer scoring (0–100) replacing flat 3-strike system
- Block explorer API: address history, transaction lookup, latest blocks
- Subnet Sybil protection (IPv4 /24, IPv6 /48)
- Persistent IP ban list

---

## [0.2.0-alpha] — 2026-03-14

### Added
- Mnemonic-based faucet wallet system (10 reserve wallets, BIP-39 derived)
- Block size increased to 2 MB for Falcon-512 transaction sizes
- License changed to Apache 2.0

### Fixed
- Nonce atomicity race condition
- Coinbase amount validation
- MTP (Median Time Past) timestamp enforcement
- Per-sender mempool cap
- State root enforcement

---

[Unreleased]: https://github.com/quantachain/quanta/compare/v2.0.2-alpha...HEAD
[2.0.2-alpha]: https://github.com/quantachain/quanta/compare/v2.0.1-alpha...v2.0.2-alpha
[2.0.1-alpha]: https://github.com/quantachain/quanta/compare/v2.0.0-alpha...v2.0.1-alpha
[2.0.0-alpha]: https://github.com/quantachain/quanta/compare/v0.7.5-alpha...v2.0.0-alpha
[0.7.5-alpha]: https://github.com/quantachain/quanta/compare/v0.7.4-alpha...v0.7.5-alpha
[0.7.4-alpha]: https://github.com/quantachain/quanta/compare/v0.7.3-alpha...v0.7.4-alpha
[0.7.3-alpha]: https://github.com/quantachain/quanta/compare/v0.7.2-alpha...v0.7.3-alpha
[0.7.2-alpha]: https://github.com/quantachain/quanta/compare/v0.7.1-alpha...v0.7.2-alpha
[0.7.1-alpha]: https://github.com/quantachain/quanta/compare/v0.7.0-alpha...v0.7.1-alpha
[0.7.0-alpha]: https://github.com/quantachain/quanta/compare/v0.6.0-alpha...v0.7.0-alpha
### Fixed
- **O(n) Sled scan in `deep_reorg` (CRITICAL)** — `base_work` was recalculated by
  reading every block from 0 to `rollback_to` from Sled. At height 85k with a 5-block
  rollback this was 85,000 sequential reads while holding the write lock, causing 30–60s
  stalls and peer timeouts that logged as `"Reorg failed: Invalid block"` then retried
  infinitely. **Fix:** replaced with `cumulative_work_at(rollback_to)` — O(1) in-memory read.
- **Wrong LWMA bounds check during reorg replay** — `validate_block_consensus_reorg`
  called `calculate_next_difficulty()` on a partially-rebuilt chain (incomplete LWMA window),
  producing a wrong estimate that rejected valid peer blocks as „outside ±50% LWMA bounds“.
  **Fix:** removed the LWMA bounds check from the reorg path; `has_valid_hash()` PoW
  already proves real work, `MIN_DIFFICULTY` still guards the floor.
- **Snapshot fallback skipped all blocks in `rebuild_account_state_up_to`** — when a
  1000-block snapshot was missing, the code fell back to genesis-only state but then set
  `replay_start = snapshot_height + 1`, silently skipping all blocks 1…snapshot_height.
  Every reorg block subsequently failed with insufficient balance / wrong nonce.
  **Fix:** when no snapshot is loaded `replay_start` is always `1`.

### Added
- **Checkpoint at block 85,000** — verified live from `scan.quantachain.org` on 2026-05-06:
  `(85_000, "0000007305d4ceeaf72a4f3c58001295a335d588e16a05f037d21dfb21ac06ca")`
  Anchors the `STATE_ROOT_SORT_FIX_HEIGHT` boundary; prevents deep reorgs into
  pre-sort-fix territory.

---

## [0.7.2-alpha] — 2026-05-05

> **CONSENSUS-CRITICAL patch. No testnet reset required.**
> All nodes must upgrade. Nodes on v0.7.1 will diverge from upgraded nodes at any block
> containing a TimeLock credit to the miner's address above height 85,000.
> Existing `quanta_data/` directories are fully compatible — drop-in upgrade.

### Fixed
- **State root determinism (`calculate_state_root`)** — `locked_balances` is a `Vec`
  whose insertion order differs between `create_block_template` (coinbase credited first)
  and `validate_block_consensus` (user txs applied first, coinbase in a second pass).
  This caused a deterministic hash mismatch whenever a block contained a `TimeLockTransfer`
  credit to the same address as the miner's coinbase — the `locked_balances` vec was
  identical in content but different in order, producing a different SHA3-256 state root.
  **Fix:** `calculate_state_root` now sorts `locked_balances` by `(unlock_height, amount)`
  before hashing — result is order-independent on every node regardless of apply order.

### Added
- **`STATE_ROOT_SORT_FIX_HEIGHT = 85_000`** — blocks below this height skip state root
  validation (they are secured by hardcoded checkpoints). This avoids re-validating the
  ~80k already-committed blocks under the new sort rule, which would fail for the small
  number of historically mismatched roots.
- **Checkpoints extended to block 80,000** — three new testnet checkpoints verified live
  from `scan.quantachain.org` on 2026-05-05:
  - `(60_000, "0000010ce22920660ba1e42423ea46e76dc7582963d6f9f220e3930031bd9bc9")`
  - `(70_000, "000001fcb0637b06601b4f111b22070e856c8cabf2eaa545c41b938b4478d186")`
  - `(80_000, "0000002d80e66bce37596616a9c9c3c1988da6e65811ad132926162c7e000a0e")`

---

## [0.7.1-alpha] — 2026-04-10

> **No testnet reset. Drop-in upgrade from v0.7.0.**

### Fixed
- `add_block_to_main_chain_reorg` called the strict `validate_block_consensus` instead
  of the permissive `validate_block_consensus_reorg` — peer reorg blocks were rejected
  if their difficulty didn't exactly match our local LWMA
- `deep_reorg` did not reset `cumulative_work` before replaying new blocks — the stale
  old-chain work value caused double-counting, making the node think it always had more
  work than peers and skip future syncs
- `reorg_to_block` (single-block tip swap) never updated `cumulative_work` — counter
  was left at the old tip's value after every shallow reorg
- `add_block_to_main_chain_reorg` had a dangling orphan code block (missing
  `for tx in &block.transactions` loop header) — compile error on affected builds
- `sync_blockchain`: `request_start <= bc_height` changed to `request_start < bc_height`
  — linear sync was incorrectly triggering `deep_reorg` when downloading the next
  sequential block, causing O(n²) account-state rebuilds during IBD

### Changed
- `save_block` and `save_account_state` no longer call `db.flush()` after every write —
  sled WAL guarantees crash safety; a single flush is issued at end of sync batch and
  after mining. Removes ~90 s of wasted fsync IO during an 18k-block IBD
- `cumulative_work` is now persisted in sled and cached in memory (`Arc<Mutex<u128>>`) —
  `cumulative_work_at(tip)` is O(1) for the current tip; previously O(height) disk scan
  while holding the blockchain read lock (primary cause of seed-node connection timeouts)
- Account state snapshots saved every 1000 blocks — `rebuild_account_state_up_to()`
  loads the nearest snapshot and replays only the delta instead of always from genesis

---

## [0.7.0-alpha] — 2026-04-08

> **Testnet reset required.** Wire format changed (cumulative_work in handshake/Height).

### Added
- `GetHeaders` / `Headers` P2P messages — headers-first sync (Bitcoin IBD architecture)
- `header_buffer` in the sync engine — collects headers before planning block downloads
- `sync_request_range` — exact [start, end] range set before GetBlocks so
  `handle_new_block` can buffer reorg blocks whose index is below the current tip
- `cumulative_work: u128` field in `P2PMessage::Height` and handshake
- `validate_block_consensus_reorg()` — permissive difficulty validator (50% LWMA bounds)
  for reorg replay path; full sig verification included
- `network_id: u32` field in `Transaction` — included in signing bytes and hash,
  cryptographically binding signatures to a specific network (replay protection)
- Inbound connection cap in `listen_for_connections()` — enforces `max_peers` before
  accepting the TCP stream (botnet exhaustion protection)
- Keep-alive ping during block download (every 15 s) — prevents seed from closing
  idle-looking connection during slow batch transfer
- Sub-batch block serving in `handle_get_blocks` (20 blocks + `yield_now`) — releases
  read lock between sub-batches so other tasks (mining, heartbeat) can run on seed
- Partial-batch guard — skips `deep_reorg` if received block count ≠ expected
- Atomic deep reorg with full rollback — saves original chain before rollback, restores
  on failure; node never left at a partial intermediate state
- `seen_blocks` / `seen_txs` LRU dedup caches — prevents broadcast storms
- `sync_request_range` mutex — allows reorg blocks (index ≤ tip) to be buffered during sync

### Fixed
- State root empty-string bypass — `state_root = ""` could skip state root validation
- Reorg path skipped transaction signature verification entirely
- Duplicate serial signature verification — `block.is_valid()` ran a redundant serial
  Falcon-512 pass before the parallel Rayon pass in `validate_block_consensus()`
- `maintain_peers` spammed new TCP connections to seed during active sync — triggered
  Sybil-protection rejection every 10 s

### Changed
- `broadcast_block()` now sends only the block header (~200 B) instead of full block
  (~2 MB) — O(peers × 200 B) instead of O(peers × 2 MB) per block
- `network_id` propagated from node config — coinbase/treasury txs use
  `self.network.network_id()` instead of hardcoded `0`
- Header wait timeout increased from 10 s to 30 s
- Block batch size reduced from 100 to 50; idle timeout increased from 30 s to 60 s
- `handle_get_headers`: running cumulative-work sum instead of O(height) per-header
  call (was causing 36M sled reads per 2000-header batch at height 18k)
- `maintain_peers` skips new connection attempts while `syncing` flag is set

---

## [0.6.0-alpha] — 2026-04-05

### Fixed
- Block template nonce sequence — mempool assembler sorted by fee without enforcing
  nonce ordering; blocks could be rejected with `InvalidNonce` at consensus
- Permanent nonce desync on reorg — `reorg_to_block` did not call `increment_nonce()`
- Faucet balance zero after sync — genesis premine applied in memory but not stored
  in genesis block struct; `rebuild_account_state_up_to()` replayed an empty tx list
- Sync stuck at block 1 — `MIN_DIFFICULTY` constant was above early testnet block
  difficulties, causing every incoming block at those heights to be rejected

---

## [0.5.0-alpha] — 2026-03-28

### Added
- LWMA (Linearly Weighted Moving Average) difficulty algorithm — adjusts every block
  on a 45-block sliding window (~22.5 min); replaces 2016-block Bitcoin-style intervals
- `deep_reorg()` — multi-block chain reorganisation engine
- Parallel Rayon signature verification with LRU cache (1800 ms → ~300 ms per block)
- Bloom filter for O(1) mempool duplicate detection
- Atomic orphan pool (`VecDeque` for O(1) pop-front)

---

## [0.3.0-alpha] — 2026-03-20 (Testnet V2)

### Added
- Testnet V2 genesis reset with realistic difficulty (6,972,889)
- Bitcoin-style DoSMan weighted peer scoring (0–100) replacing flat 3-strike system
- Block explorer API: address history, transaction lookup, latest blocks
- Subnet Sybil protection (IPv4 /24, IPv6 /48)
- Persistent IP ban list

---

## [0.2.0-alpha] — 2026-03-14

### Added
- Mnemonic-based faucet wallet system (10 reserve wallets, BIP-39 derived)
- Block size increased to 2 MB for Falcon-512 transaction sizes
- License changed to Apache 2.0

### Fixed
- Nonce atomicity race condition
- Coinbase amount validation
- MTP (Median Time Past) timestamp enforcement
- Per-sender mempool cap
- State root enforcement

---

[Unreleased]: https://github.com/quantachain/quanta/compare/v2.0.2-alpha...HEAD
[2.0.2-alpha]: https://github.com/quantachain/quanta/compare/v2.0.1-alpha...v2.0.2-alpha
[2.0.1-alpha]: https://github.com/quantachain/quanta/compare/v2.0.0-alpha...v2.0.1-alpha
[2.0.0-alpha]: https://github.com/quantachain/quanta/compare/v0.7.5-alpha...v2.0.0-alpha
[0.7.5-alpha]: https://github.com/quantachain/quanta/compare/v0.7.4-alpha...v0.7.5-alpha
[0.7.4-alpha]: https://github.com/quantachain/quanta/compare/v0.7.3-alpha...v0.7.4-alpha
[0.7.3-alpha]: https://github.com/quantachain/quanta/compare/v0.7.2-alpha...v0.7.3-alpha
[0.7.2-alpha]: https://github.com/quantachain/quanta/compare/v0.7.1-alpha...v0.7.2-alpha
[0.7.1-alpha]: https://github.com/quantachain/quanta/compare/v0.7.0-alpha...v0.7.1-alpha
[0.7.0-alpha]: https://github.com/quantachain/quanta/compare/v0.6.0-alpha...v0.7.0-alpha
[0.6.0-alpha]: https://github.com/quantachain/quanta/compare/v0.5.0-alpha...v0.6.0-alpha
[0.5.0-alpha]: https://github.com/quantachain/quanta/compare/v0.3.0-alpha...v0.5.0-alpha
[0.3.0-alpha]: https://github.com/quantachain/quanta/compare/v0.2.0-alpha...v0.3.0-alpha
[0.2.0-alpha]: https://github.com/quantachain/quanta/releases/tag/v0.2.0-alpha

## [2.4.21-alpha] - 2026-07-17
### Fixed
- **AlephBFT Unicast Routing**: Removed an aggressive CPU spike filter that was incorrectly dropping AlephBFT unicast messages. This fixes an issue where validators could not fetch missing DAG units across the P2P network, resolving stalled AlephBFT consensus when the network is not a full mesh.
