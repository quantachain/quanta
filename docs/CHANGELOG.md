# Changelog

All notable changes to QuantaChain are documented here.

Format: [Keep a Changelog](https://keepachangelog.com/en/1.0.0/)  
Versioning: [Semantic Versioning](https://semver.org/spec/v2.0.0.html)

---

## [Unreleased]

---

## [2.0.1] — 2026-06-06

> **TESTNET RESET — All nodes must wipe their database and restart from the new genesis.**
>
> This release replaces a lost validator wallet, rotates all 10 genesis faucet wallets,
> bumps the genesis timestamp to today, and fixes a critical AlephBFT block-timing bug
> that caused blocks to slow from 6 s to 1–2 h over time.

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

### Added
- **`src/bin/get_testnet_hash`** — new binary that calls `Block::genesis()`, prints all
  structural fields and the deterministic genesis hash, and verifies it is reproducible.
  Run after any change to `block.rs` to get the updated `TESTNET_GENESIS_HASH`.
- **`src/bin/gen_faucet_wallets` v2** — rewritten to read passphrase and file-encryption
  password from environment variables only (never CLI args). Generates 10 Falcon-512 HD
  faucet accounts, saves encrypted `faucet_wallet.json`, prints both address arrays for
  `blockchain.rs`, and prints the genesis hash.
- **`QUANTA_WALLET_PASSPHRASE` env var** support in `quanta-wallet restore` — optional BIP39
  25th-word passphrase support, defaults to `""` (backward compatible with all existing wallets).

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

[Unreleased]: https://github.com/quantachain/quanta/compare/v0.7.5-alpha...HEAD
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
