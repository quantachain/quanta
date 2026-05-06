# QuantaChain Testnet — Alpha v0.7.3

Post-quantum secure blockchain using Falcon-512 signatures and SHA3-256 Proof of Work.

> **v0.7.3 — Sync stability patch. No testnet reset required.**
> All nodes SHOULD upgrade. v0.7.2 nodes may get stuck during reorg at high block heights
> due to a timeout-inducing O(n) Sled scan and incorrect LWMA bounds check during replay.
> Existing `quanta_data/` directories are fully compatible — drop-in upgrade.

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
docker pull xd637/quanta-node:v0.7.3-alpha

docker run -d \
  --name quanta-node \
  -p 3000:3000 -p 8333:8333 -p 7782:7782 -p 9090:9090 \
  -v quanta-data:/home/quanta/quanta_data \
  -v quanta-logs:/home/quanta/logs \
  xd637/quanta-node:v0.7.3-alpha
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

**3. Upgrade to v0.7.3 (no data wipe required):**
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

## Manual Build from Source

```bash
git clone https://github.com/quantachain/quanta
cd quanta
git checkout v0.7.3-alpha
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

## What Changed in Alpha v0.7.3

**No testnet reset. No wire format change. Sync stability patch — all nodes should upgrade.**

### Fix — O(n) Sled scan on every reorg (`deep_reorg`)

When performing even a tiny 2–5 block reorg, `deep_reorg` was recalculating `base_work`
by reading **every block from 0 to rollback_to** out of Sled. At height 85k this was
85,000 sequential reads while holding the blockchain write lock, taking 30–60 seconds.
During this time the syncing peer would time out, drop the connection, and the sync loop
logged `"Reorg failed: Invalid block"` then retried — hitting the same scan again.

Fix: replaced with `cumulative_work_at(rollback_to)` which is O(1) — it reads the
in-memory `cumulative_work` value that is updated incrementally after every block.

### Fix — Wrong LWMA bounds check during reorg replay

`validate_block_consensus_reorg()` called `calculate_next_difficulty()` to obtain an
LWMA estimate, then rejected any peer block whose difficulty was outside ±50% of that
estimate. During a deep reorg the chain is partially rebuilt — the LWMA window is
incomplete (wrong timestamps, missing blocks) — so the estimate was meaningless and
valid peer blocks were rejected as "outside LWMA bounds".

Fix: removed the LWMA bounds check from the reorg validation path. The `has_valid_hash()`
PoW check already proves real work was done. `MIN_DIFFICULTY` still guards against trivial
blocks.

### Fix — Snapshot fallback replayed wrong block range (`rebuild_account_state_up_to`)

When a 1000-block snapshot was missing (new nodes have none), the code fell back to
genesis-only state (10 faucet accounts) then set `replay_start = snapshot_height + 1`,
skipping all blocks 1…snapshot_height. The rebuilt state had genesis balances only, so
every subsequent reorg block failed with "Insufficient balance / wrong nonce → Invalid block".

Fix: when no snapshot is loaded, `replay_start` is always set to `1`.

### Added — Checkpoint at block 85,000

Verified live from `scan.quantachain.org` on 2026-05-06. This anchors the
`STATE_ROOT_SORT_FIX_HEIGHT` boundary and prevents deep reorgs into pre-sort-fix territory.

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
