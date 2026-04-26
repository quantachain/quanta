# Release Notes

---

## Alpha v0.7.1 — April 2026

**No testnet reset required.** Drop-in upgrade from v0.7.0. All `quanta_data/` directories are fully compatible.

```bash
docker pull xd637/quanta-node:latest
docker stop quanta-node && docker rm quanta-node
docker run -d --name quanta-node --restart always --network host \
  -v ~/quanta_data:/home/quanta/quanta_data \
  xd637/quanta-node:latest
```

### Fixes

**`deep_reorg` used wrong block validator**

`add_block_to_main_chain_reorg()` was calling the strict validator which requires the incoming block's difficulty to exactly match the local LWMA. During a deep reorg, peer blocks are mined against their LWMA which can differ slightly. Fixed: reorg path now calls `validate_block_consensus_reorg()` — the permissive 50%-bounds validator written for this purpose.

**`deep_reorg` corrupted `cumulative_work` counter**

After rolling back the chain, `cumulative_work` remained at the old tip's value. Each replayed block added to the stale total, producing a value roughly double the correct amount. The node would always believe it had more work than peers and skip future syncs. Fixed: `deep_reorg` recomputes the correct base work from storage before replaying, and resets both the in-memory counter and the sled key.

**Single-block tip swap never updated `cumulative_work`**

The 1-deep reorg path swapped the block and rebuilt account state, but left `cumulative_work` at the old tip's value. Fixed: subtracts the old tip's difficulty and adds the new tip's difficulty after commit.

**`add_block_to_main_chain_reorg` dangling orphan code**

A previous edit left a `if !tx.is_coinbase() { ... }` block without its enclosing `for tx in &block.transactions` loop — a compile-time error. Fixed: restored the complete nonce-clearing loop.

**Linear sync treated as reorg**

When the sync engine requested blocks starting exactly at the current chain height, `request_start == bc_height` triggered a `deep_reorg` call. This caused O(n²) behaviour during normal linear sync — every downloaded block triggered a full chain rollback. Fixed: condition changed to strictly-less-than so only blocks below the current tip enter the reorg path.

### Improvements

**No per-block `fsync`**

`save_block` and `save_account_state` no longer call `db.flush()` after every write. Sled's write-ahead log guarantees crash safety without per-block fsync. A single `flush_storage()` is issued at the end of each sync batch and after mining a block. At 18,000 blocks × ~5 ms/fsync this removes ~90 seconds of wasted I/O during initial block download.

**O(1) cumulative work lookup**

`cumulative_work` is now stored as a sled key and kept in an in-memory `Arc<Mutex<u128>>`. Returns O(1) for the current tip. Previously every call scanned all blocks from genesis — O(height) disk reads while holding the blockchain read lock, the primary cause of seed-node connection timeouts.

**Account state snapshots every 1,000 blocks**

`add_block_to_main_chain` saves a full account state snapshot at every 1,000-block boundary. `rebuild_account_state_up_to()` loads the nearest snapshot and replays only the delta. Previously it always replayed from genesis — O(height) on every reorg.

---

## Alpha v0.7.0 — April 2026

**Testnet reset required.** v0.6.0 and v0.7.0 nodes are not mutually compatible (wire format changed).

### Major: Headers-First Sync (Bitcoin IBD Style)

Two new wire messages — `GetHeaders` and `Headers` — allow a syncing node to download light headers before requesting full blocks. The sync engine validates headers first, finds the fork point, then requests only missing blocks in ordered batches.

### Major: Cumulative Work Peer Selection

The handshake now exchanges `cumulative_work` alongside `height`. Sync always targets the peer with the highest cumulative PoW, not the tallest chain. Prevents chain-length attacks.

### Major: Atomic Deep Reorg with Rollback

Failed reorgs no longer leave the node at a partial intermediate state. The original chain is saved before rollback and restored on failure.

### Security: Cross-Chain Replay Protection

`network_id: u32` added to `Transaction`. Testnet = `0`, Mainnet = `1`. Signatures are cryptographically bound to a specific network — a testnet signature cannot be replayed on mainnet.

### Security: State Root Empty-String Bypass Closed

Blocks with `state_root = ""` can no longer bypass state root validation.

### Security: Reorg Path Signature Verification

`validate_block_consensus_reorg()` now runs the parallel Rayon Falcon-512 signature pass.

### Security: Inbound Connection Cap

`listen_for_connections()` now enforces `max_peers` before accepting the TCP stream. Prevents resource exhaustion from inbound connection floods.

### Improvement: Light Block Gossip

`broadcast_block()` sends only the block header (~200 bytes) instead of the full block (~2 MB). Peers request the full block if they need it. Significantly reduces bandwidth during mining.

---

## Testnet Parameters

| Parameter | Value |
|-----------|-------|
| Network | Testnet (QUA7) |
| Genesis Timestamp | `1775001600` (2026-04-01 00:00:00 UTC) |
| Genesis Hash | `00000012d3a2cbb7eb9579330ccdaa4f83ca9e6e016bfe6d2c8a38539cf3733b` |
| Difficulty | 8,304,130 |
| Block Time | 30 seconds |

---

## Security Notice

Alpha releases have undergone internal audit only and have not been formally verified by a third-party security firm. Do not use for real financial transactions.

Report security issues to [admin@quantachain.org](mailto:admin@quantachain.org) — do not open public issues for security vulnerabilities.
