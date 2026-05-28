# QuantaChain Testnet — Alpha v0.7.5

Post-quantum secure blockchain using Falcon-512 signatures and SHA3-256 Proof of Work.

> **v0.7.5 — Consensus-critical: state root fix + stale mining fix + 90k checkpoint.**
> All nodes MUST upgrade. Nodes stuck at block 91,096 ("Invalid state root") and all
> nodes experiencing stale mined blocks or nonce errors after reorg are fixed.
> **No testnet reset. No data wipe required.**

This is a pre-release testnet build. Do not use real funds. APIs and chain parameters may change between alpha releases.

---

## Genesis Block

| Parameter | Value |
|---|---|
| Network | Testnet (QUA7) |
| Timestamp | `1775001600` (2026-04-01 00:00:00 UTC) |
| Testnet Genesis Hash | `00000012d3a2cbb7eb9579330ccdaa4f83ca9e6e016bfe6d2c8a38539cf3733b` |
| Mainnet Genesis Hash | `1cdbccdff3db462378f4acbe4553b49040ffcdebf74b5c77e685ba05ccfa8cb0` |
| Difficulty | 8,304,130 (Testnet) / 16,777,216 (Mainnet) |
| Block Time | 30 seconds |

---

## Testnet Faucet Wallets (Genesis Premine)

Each address below received **1,000,000 QUA** at genesis. Faucet account 0 is the active sender used by the faucet API.

| Index | Address | Role |
|---|---|---|
| 0 | `0x1683be267318d2ddd8cee8df4a4548dcffb1e088` | Faucet Sender (active) |
| 1 | `0xd528c18ce7a8844e4a4dcd841975b20ae599b020` | Faucet Reserve |
| 2 | `0xfd6e36bfa2b2798d08592802206c943d5513adfb` | Faucet Reserve |
| 3 | `0xed15573ad312d41aaef74cff56a8ef28122ec2db` | Faucet Reserve |
| 4 | `0xaffd6d4f74c5651110efcf1b9736f7a5cf2ccdbb` | Faucet Reserve |
| 5 | `0xbf5ee055f399323fdd0cefe3d4aa923678d46107` | Faucet Reserve |
| 6 | `0x1dc9637b183093d723ea8d1fb18083b06490facb` | Faucet Reserve |
| 7 | `0xa2270f30ca1aad922510375508bf68cd95509f29` | Faucet Reserve |
| 8 | `0xe15a689775685ae324559ea9a492fc650354ca0b` | Faucet Reserve |
| 9 | `0x005dcff212d27b55e7a74bf745e1349ab44ca25d` | Faucet Reserve |

---

## Treasury

| Parameter | Value |
|---|---|
| Treasury Address | `ms69216b1d10425689704d5ae3b2a4aa17049f59b1` |
| Multisig Scheme | 3-of-5 Falcon-512 |
| Block Reward to Treasury | 5% per block |
| Fee to Treasury | 20% of transaction fees |

---

## Tokenomics

| Parameter | Value |
|---|---|
| Year 1 Block Reward | 100 QUA |
| Annual Reward Reduction | 15% per year |
| Minimum Reward Floor | 5 QUA (reached ~year 20) |
| Mining Reward Lock | 50% locked for 6 months |
| Fee Burn | 70% of all transaction fees |
| Fee to Treasury | 20% of all transaction fees |
| Fee to Miner | 10% of all transaction fees |
| Max Block Transactions | 1,200 |
| Max Block Size | 2 MB |
| Min Transaction Fee | 0.0001 QUA |

---

## Quick Start with Docker

### Option 1: Docker Desktop (Graphical Interface)
1. Open Docker Desktop and find `xd637/quanta-node:v0.7.1-alpha` (or `:latest`) in your Images.
2. Click **Run**.
3. Under **Optional settings**, configure:
   - **Container name**: `quanta-node`
   - **Ports**: Map `3000`, `7782`, `8333`, `9090` to themselves.
   - **Volumes**:
     - Add host path `quanta-data` to container path `/home/quanta/quanta_data`
     - Add host path `quanta-logs` to container path `/home/quanta/logs`
4. Click **Run**.

### Option 2: Docker CLI
```bash
docker pull xd637/quanta-node:v0.7.5-alpha

docker run -d \
  --name quanta-node \
  -p 3000:3000 -p 8333:8333 -p 7782:7782 -p 9090:9090 \
  -v quanta-data:/home/quanta/quanta_data \
  -v quanta-logs:/home/quanta/logs \
  xd637/quanta-node:v0.7.4-alpha
```

### Option 3: Docker Compose (Recommended)

**Upgrading from v0.7.0 — no data wipe needed:**
```bash
docker compose -f docker-compose.single.yml pull
docker compose -f docker-compose.single.yml up -d
```

---

## Server Setup (Ubuntu / VPS)

**1. Update system and install Docker:**
```bash
sudo apt update && sudo apt upgrade -y
curl -fsSL https://get.docker.com | sh
sudo usermod -aG docker ubuntu && newgrp docker
```

**2. Open necessary ports:**
```bash
sudo ufw allow 8333/tcp
sudo ufw allow 7782/tcp
sudo ufw allow 3000/tcp
sudo ufw allow ssh
sudo ufw --force enable
```

**3. Upgrade to v0.7.5 (no data wipe required):**
```bash
docker pull xd637/quanta-node:latest
docker stop quanta-node && docker rm quanta-node
docker run -d \
  --name quanta-node \
  --restart always \
  --network host \
  -v ~/quanta_data:/home/quanta/quanta_data \
  xd637/quanta-node:latest
```

**4. Check logs:**
```bash
docker logs quanta-node --tail 30 -f
```

---

## 🔄 Clean Start Guide (Wipe Data & Resync from Genesis)

> **When should I do a clean start?**
> - Node stuck at same block height for > 30 minutes
> - Logs show repeated `"Invalid block"` or `"Reorg failed"` errors
> - Running a version older than v0.7.0 (wire format changed)
> - Support advises it

> ⚠️ **Will I lose my mining rewards?**
> Your **wallet file** is separate from the node database. Mining rewards live on-chain —
> your balance is safe as long as your address exists on the canonical chain.
> Wipe only the **data directory**, never your wallet file.

---

### Docker — named volume (most common)

```bash
# 1. Stop and remove the container
docker stop quanta-node
docker rm quanta-node

# 2. Delete the chain data volume
docker volume rm quanta-data

# 3. Pull latest image and start fresh
docker pull xd637/quanta-node:latest
docker run -d \
  --name quanta-node \
  --restart always \
  --network host \
  -v quanta-data:/home/quanta/quanta_data \
  -v quanta-logs:/home/quanta/logs \
  xd637/quanta-node:latest

# 4. Watch sync progress
docker logs quanta-node --tail 50 -f
```

---

### Docker — host path mount (`~/quanta_data`)

```bash
docker stop quanta-node && docker rm quanta-node
rm -rf ~/quanta_data && mkdir -p ~/quanta_data

docker pull xd637/quanta-node:latest
docker run -d \
  --name quanta-node \
  --restart always \
  --network host \
  -v ~/quanta_data:/home/quanta/quanta_data \
  xd637/quanta-node:latest

docker logs quanta-node --tail 50 -f
```

---

### Bare Metal (no Docker)

```bash
pkill -f "quanta start"    # stop the node
rm -rf ./quanta_data       # wipe chain data (adjust path if changed in quanta.toml)
./target/release/quanta start -c quanta.toml
```

---

### Windows — Docker Desktop

1. **Docker Desktop → Volumes** → delete `quanta-data`
2. Open a terminal:
```bash
docker stop quanta-node && docker rm quanta-node
docker pull xd637/quanta-node:latest
docker run -d --name quanta-node ^
  -p 3000:3000 -p 8333:8333 -p 7782:7782 -p 9090:9090 ^
  -v quanta-data:/home/quanta/quanta_data ^
  xd637/quanta-node:latest
```

---

### How long does resync take?

> Times depend on VPS CPU core count (Rayon uses all cores for Falcon-512 verification)
> and network speed to the bootstrap node.

| Chain Height | Good VPS (4+ cores) | Weak VPS / slow link |
|---|---|---|
| 0 → 50,000 | ~3–6 min | ~10–15 min |
| 0 → 91,000+ | ~5–15 min | ~15–25 min |

The main bottleneck is **Falcon-512 signature verification** — each block's signatures are
verified in parallel via Rayon, and the LRU cache skips re-verification of seen sigs.
State replay is fast due to 1,000-block snapshots (only the delta is replayed, not from genesis).

```bash
# Watch sync in real time
docker logs quanta-node -f | grep -i "accepted\|height\|sync"
```

Or check live at [scan.quantachain.org](https://scan.quantachain.org)

---

## Manual Build from Source

```bash
git clone https://github.com/quantachain/quanta
cd quanta
git checkout v0.7.5-alpha
cargo build --release

./target/release/quanta start -c quanta.toml
```

---

## Wallet Management

```bash
# Native
./target/release/quanta new-wallet --file wallet.qua

# Docker
docker exec -it quanta-node quanta new_wallet --file wallet.qua

# HD Wallet (recommended — 24-word recovery phrase)
docker exec -it quanta-node quanta new_hd_wallet --file hd_wallet.json
```

---

## Mining

```bash
# Native
./target/release/quanta start_mining YOUR_WALLET_ADDRESS --rpc-port 7782

# Docker (background)
docker exec -d quanta-node quanta start_mining YOUR_WALLET_ADDRESS --rpc-port 7782

# Stop
docker exec -it quanta-node quanta stop_mining --rpc-port 7782
```

---

## Node Status

```bash
docker exec -it quanta-node quanta status --rpc-port 7782
docker exec -it quanta-node quanta print_height --rpc-port 7782
docker exec -it quanta-node quanta mining_status --rpc-port 7782
```

---

## Ports

| Port | Service |
|---|---|
| `3000` | REST API |
| `8333` | P2P Network |
| `7782` | RPC |
| `9090` | Prometheus Metrics |

---

## What Changed in Alpha v0.7.5

**No testnet reset. No wire format change. All nodes must upgrade.**

> Nodes stuck at block 91,096 with repeated "Invalid state root" errors will be fixed
> by this release. The 90,000 checkpoint means syncing nodes can pass this height cleanly.

### Fix — State root mismatch at block 91,096 (root cause)

`create_block_template` (miner) and `validate_block_consensus` (receiver) both computed
the state root hash from a cloned account state **without** first calling
`unlock_mature_coinbase(index)`. At block 91,096 — exactly 100 blocks (`COINBASE_MATURITY`)
after the bootstrap node's heavy mining burst around block 90,996 — locked coinbase
entries matured. The two sides hashed structurally different account states:

```
WARN Invalid state root at block 91096:
  computed=c372afa7b...  block=5de69d916...
```

Fix: both paths now call `unlock_mature_coinbase(block.index)` **before** applying
transactions and computing the state root hash. This is the same step that
`add_block_to_main_chain` already performed when committing — now all three code paths
are consistent.

### Fix — Invalid nonce after every reorg ("expected 5, got 1")

The `pending_nonces` DashMap tracked the highest mempool nonce per sender. After any
reorg, transactions from the abandoned fork were discarded — but `pending_nonces` still
held those stale nonces. The next canonical-chain block (nonce=1 from a clean state)
was rejected with "expected 5, got 1".

Fix: all three reorg paths (`reorg_to_block`, `add_block_to_main_chain_reorg`,
`add_block_to_main_chain`) now clear or sweep `pending_nonces` after every chain switch.

### Fix — All mined blocks stale (abort-on-new-block)

`block.mine()` was an **uninterruptible PoW loop** — it could not stop mid-computation
even when a peer block arrived. Miners wasted up to 30 s finishing dead work.

Fix:
- New `Block::mine_with_cancel(&AtomicBool)` — polls a cancel flag every 10,000 hashes
  (~10 ms at typical hashrate) and returns `false` immediately when cancelled.
- New `Blockchain::subscribe_new_blocks()` — returns a `watch::Receiver<u64>` that
  fires every time any block is accepted (normal, reorg, or shallow swap).
- Mining loop rewritten with `tokio::select!` — when the watch channel fires, the
  `AtomicBool` is set and the PoW thread exits within ~10 ms. A fresh template is
  started on the next loop iteration.

### Added — Checkpoint at block 90,000

| Height | Hash |
|--------|------|
| 90,000 | `000000dc0e178a5140a5c68481234a9541373ac349b1ae3cbc3f0f3f1fc58d5e` |

Verified live from `scan.quantachain.org` on 2026-05-08. Anchors the
`STATE_ROOT_SORT_FIX_HEIGHT` boundary. Nodes must be on v0.7.5+ to sync past this.

### Changed — Falcon-512 signing unified under `falcon-rust`

All signing paths (CLI wallet, `wallet_cli`, faucet distributor, benchmarks, unit tests)
now use `falcon_rust::sign` instead of `pqcrypto_falcon::sign`. This guarantees
byte-identical output with the browser WASM wallet on every path — eliminating a
latent cross-library format ambiguity where native-signed transactions could have
produced a different byte blob than WASM-signed ones.

`pqcrypto-falcon` is still present (key generation), `pqcrypto-kyber` is still present
(Kyber-1024 wallet encryption). Nothing is removed from `Cargo.toml`.

---

## What Changed in Alpha v0.7.4

**No testnet reset. No wire format change. All nodes must upgrade.**

### Fix — Block 84,812 nonce incompatibility (clean-start nodes stuck)

A previous reorg ran under the v0.7.2 snapshot-fallback bug, causing the main node to
rebuild account state without the faucet wallet's 4 earlier transactions. It accepted
block 84,812 with nonce=1 (should be 5). Clean-start nodes expecting nonce=5 rejected
the block permanently. Fix: for blocks below the highest checkpoint (85,000), nonce
mismatches override `temp_state` to the block's claimed nonce instead of rejecting.

### Fix — State root skip height raised 85,000 → 90,000

Blocks 85,000–89,999 were mined with corrupted account state (from the v0.7.2 reorg
bug). No clean-sync node can reproduce those state roots. `STATE_ROOT_SORT_FIX_HEIGHT`
raised to 90,000. A new checkpoint at 90,000 will be added once the main node reaches
that height on v0.7.4.

### Fix — `cumulative_work_at` off-by-one (deep-reorg path)

`for h in 0..tip_height` excluded the block AT `tip_height`, making every deep-reorg
reset cumulative_work ~8.3M too low. Drift from repeated reorgs caused sync to
incorrectly believe local work ≥ peer work. Fixed to `0..=tip_height`.

### Fix — Sync loop: "Already on heaviest chain" when 20+ blocks behind

Peer selection in `sync_blockchain` compared cumulative_work only. When local work
drifted above the peer's, no peer was selected and sync silently stopped. Added a
`far_ahead` safety net: if a peer is >5 blocks ahead by height, always sync.

### Fix — Mempool retained confirmed TXs (hash comparison mismatch)

`public_key` byte differences between mempool and P2P block paths caused `tx.hash()`
comparison to fail, leaving confirmed TXs in the mempool. Added `(sender, nonce)`
matching as a fallback eviction path.

### Fix — Faucet duplicate-nonce race condition (`quanta-web`)

Concurrent faucet claims submitted identical nonces. Added an async mutex and
in-memory pending-nonce tracker to serialise claim submissions.

---

## What Changed in Alpha v0.7.3

**No testnet reset. No wire format change. Sync stability patch.**

### Fix — O(n) Sled scan on every reorg (`deep_reorg`)

`deep_reorg` recalculated `base_work` by reading every block from 0 to `rollback_to`
from Sled — 85,000 reads at height 85k, taking 30–60s. Fixed with O(1)
`cumulative_work_at(rollback_to)`.

### Fix — Wrong LWMA bounds check during reorg replay

`validate_block_consensus_reorg()` rejected valid peer blocks as "outside LWMA bounds"
because the LWMA window was incomplete mid-reorg. Removed the bounds check from the
reorg path.

### Fix — Snapshot fallback replayed wrong block range

`replay_start` was set to `snapshot_height + 1` even when no snapshot was loaded,
skipping all blocks 1…snapshot_height. Fixed to always use `replay_start = 1`.

### Added — Checkpoint at block 85,000

| Height | Hash |
|--------|------|
| 85,000 | `0000007305d4ceeaf72a4f3c58001295a335d588e16a05f037d21dfb21ac06ca` |

---



## What Changed in Alpha v0.7.2

**No testnet reset. No wire format change. Consensus-critical patch — all nodes must upgrade.**

### Fix — State root determinism (`calculate_state_root`)

The `locked_balances` field on each account is a `Vec<LockedBalance>`. When a block
contains a `TimeLockTransfer` credit to the miner's own address *alongside* a coinbase
credit, two `LockedBalance` entries are pushed to that address's vec — but in different
orders depending on which code path runs:

- **Mining path** (`create_block_template`): coinbase tx processed first → coinbase lock
  pushed first, TimeLock lock pushed second.
- **Validation path** (`validate_block_consensus`): user txs applied first → TimeLock
  lock pushed first, coinbase lock pushed second.

Both vecs contain the same two entries, but SHA3-256 is order-sensitive — the resulting
state root hashes differed between the mining node and every syncing peer, causing:

```
[ERROR] Invalid state root at block N: expected <mining_hash>, got <validation_hash>
```

This manifested sporadically (only when a miner received a `TimeLockTransfer` to their
own wallet in the same block they mined) and was the root cause of the "nodes fail at
varying heights" sync bug reported across the testnet.

**Fix:** `calculate_state_root` now sorts `locked_balances` by `(unlock_height, amount)`
before iterating. The sort is stable, deterministic on all platforms, and
order-independent — both code paths now produce an identical SHA3-256 digest.

### Guard — `STATE_ROOT_SORT_FIX_HEIGHT = 85_000`

Blocks below height 85,000 skip state root validation — they are already secured by
hardcoded checkpoints and were committed under the old (buggy) ordering rule. Applying
the new sort rule retroactively would fail for any historical block that happened to
have the mismatch, turning a sync fix into a sync break.

From height 85,000 onward, the new deterministic state root is enforced on all nodes.

### New Checkpoints (through block 80,000)

Three testnet checkpoints verified live from `scan.quantachain.org` on 2026-05-05:

| Height | Hash |
|--------|------|
| 60,000 | `0000010ce22920660ba1e42423ea46e76dc7582963d6f9f220e3930031bd9bc9` |
| 70,000 | `000001fcb0637b06601b4f111b22070e856c8cabf2eaa545c41b938b4478d186` |
| 80,000 | `0000002d80e66bce37596616a9c9c3c1988da6e65811ad132926162c7e000a0e` |

These protect the chain from deep reorgs below 80k even on nodes that have not yet
reached that height.

---

## What Changed in Alpha v0.7.1

**No testnet reset. No wire format change. Drop-in upgrade.**

### Fix — `deep_reorg` used wrong validator on peer blocks

`add_block_to_main_chain_reorg()` was calling `validate_block_consensus()` — the strict
validator that requires the incoming block's difficulty to exactly match the local LWMA.
During a deep reorg, peer blocks were mined against *their* LWMA which can differ slightly
from ours (the two chains diverged at a prior block with a different timestamp).

Fix: reorg path now calls `validate_block_consensus_reorg()`, the 50%-bounds permissive
validator that was already written for this purpose but wasn't being used.

### Fix — `deep_reorg` corrupted `cumulative_work` counter

After rolling back the chain, the in-memory `cumulative_work` was still at the old tip's
value. Each new block applied by `add_block_to_main_chain_reorg` *added* to this stale
total, producing a `cumulative_work` value roughly double the correct amount. This caused
the node to always believe it had more work than all peers and skip future syncs.

Fix: `deep_reorg` now recomputes the correct base work from storage before replaying
new blocks, and resets both the in-memory counter and the sled key to this value.

### Fix — single-block tip swap (`reorg_to_block`) never updated `cumulative_work`

The 1-deep reorg path correctly swapped the block and rebuilt account state, but never
adjusted the `cumulative_work` counter. The counter was left at the old tip's value.

Fix: subtracts the old tip's difficulty and adds the incoming tip's difficulty after commit.

### Fix — `add_block_to_main_chain_reorg` had dangling orphan code (compile error)

A previous edit left a `if !tx.is_coinbase() { ... }` block without its enclosing
`for tx in &block.transactions` loop. This was a compile-time error in practice.

Fix: restored the complete nonce-clearing loop matching `add_block_to_main_chain`.

### Fix — linear sync treated as reorg (`request_start <= bc_height` → `< bc_height`)

When the sync engine requested the next batch of blocks starting exactly at the current
chain height, `request_start == bc_height` evaluated true for the reorg branch and
triggered a `deep_reorg` call. This caused O(n²) behaviour during normal linear sync —
every downloaded block triggered a full chain rollback and account-state rebuild.

Fix: condition changed to strictly-less-than so only blocks *below* the current tip
are treated as a reorg.

### Improvement — Storage: no per-block `fsync`

`save_block` and `save_account_state` no longer call `db.flush()` after every write.
Sled's write-ahead log guarantees crash safety without a per-block fsync. A single
`flush_storage()` call is issued at the end of each sync batch and after mining a block.
At 18,000 blocks × ~5 ms/fsync this removes ~90 seconds of wasted IO during IBD.

### Improvement — O(1) cumulative work lookup

`cumulative_work` is now stored as a sled key and kept in an in-memory `Arc<Mutex<u128>>`.
`cumulative_work_at(tip)` returns the stored value in O(1) for the current tip.
Previously every call scanned all blocks from genesis (O(height) disk reads while
holding the blockchain read lock — the primary cause of seed-node connection timeouts).

### Improvement — Account state snapshots every 1000 blocks

`add_block_to_main_chain` now saves a full account state snapshot at every 1000-block
boundary. `rebuild_account_state_up_to()` loads the nearest snapshot and replays only
the delta — previously it always replayed from genesis, O(height) on every reorg.

---

## What Changed in Alpha v0.7.0

> **UPGRADE NOTICE — v0.7.0 required a testnet reset.**
> The cumulative_work handshake field changed the binary wire format.
> v0.6.0 and v0.7.0 nodes are not mutually compatible.

### Major Architecture — Headers-First Sync (Bitcoin IBD style)

Two new wire messages — `GetHeaders` and `Headers` — allow a syncing node to download
light headers (index, hash, previous_hash, difficulty, cumulative_work) before requesting
full blocks. The sync engine validates headers first, finds the fork point, then requests
only the missing full blocks in ordered batches.

### Cumulative work-based peer selection

The handshake now exchanges `cumulative_work` alongside `height`. Sync always targets
the peer with the highest cumulative PoW — not the tallest chain.

### Atomic deep reorg with rollback

Failed reorgs no longer leave the node at a partial intermediate state. The original
chain is saved before rollback and restored on failure.

### Security — Cross-chain replay protection

`network_id: u32` added to `Transaction`. Signatures are cryptographically bound to a
specific network. Testnet = `0`, Mainnet = `1`.

### Security — State root empty-string bypass closed

Blocks with `state_root = ""` can no longer bypass state root validation.

### Security — Reorg path now verifies all transaction signatures

`validate_block_consensus_reorg()` now runs the parallel Rayon signature pass.

### Security — Inbound peer connection cap

`listen_for_connections()` now enforces `max_peers` before accepting the TCP stream.

### Improvement — Light block gossip

`broadcast_block()` sends only the block header (~200 B) instead of the full block
(~2 MB). Peers request the full block if they need it.

---

## Security Notice

This release has undergone internal audit only. It has not been formally verified
by a third-party security firm. Do not use for real financial transactions.

Falcon-512 and Kyber-1024 implementations are based on NIST PQC Round 3 finalists.

---

## License

Apache 2.0 — see [LICENSE](LICENSE)

"Quanta" and "QuantaChain" are trademarks. Forks may not use these names without permission.
