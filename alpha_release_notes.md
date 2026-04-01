# QuantaChain Testnet — Alpha v0.7.0

Post-quantum secure blockchain using Falcon-512 signatures and SHA3-256 Proof of Work.

> **UPGRADE NOTICE — v0.7.0 (TESTNET RESET REQUIRED)**
> This release includes a major sync architecture change (BID) and security hardening.
> The new cumulative-work peer selection and headers-first sync protocol are incompatible
> with the previous v0.6.0 chain state.
> **All node operators MUST delete their `quanta_data/` directories before starting v0.7.0.**

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
1. Open Docker Desktop and find `xd637/quanta-node:v0.7.0-alpha` (or `:latest`) in your Images.
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
docker pull xd637/quanta-node:v0.7.0-alpha

docker run -d \
  --name quanta-node \
  -p 3000:3000 -p 8333:8333 -p 7782:7782 -p 9090:9090 \
  -v quanta-data:/home/quanta/quanta_data \
  -v quanta-logs:/home/quanta/logs \
  xd637/quanta-node:v0.7.0-alpha
```

### Option 3: Docker Compose (Recommended)

**REQUIRED: Delete old chain data first (testnet reset)**
```bash
docker compose down -v
sudo rm -rf ~/quanta_data/*
```

Then start:
```bash
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

**3. Clean start (REQUIRED for v0.7.0 upgrade):**
```bash
docker stop quanta-node && docker rm quanta-node

# Delete old blockchain data
sudo rm -rf ~/quanta_data/*

docker pull xd637/quanta-node:latest
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
git checkout v0.7.0-alpha
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

## What Changed in Alpha v0.7.0

### Major Architecture — BID (Bitcoin-style Block and Header Download)

The sync engine has been rebuilt around the same two-phase headers-first architecture
that Bitcoin Core uses for Initial Block Download. This was the primary cause of
all previous sync stalls, fork loops, and orphan accumulation on the testnet.

**The old problem:**
Every incoming block triggered a full validation cycle immediately regardless of
ordering. On a fresh sync or after a reorg, blocks arrived out of sequence, were
stored as orphans, and the chain never advanced. The stall counter fired, triggering
another deep reorg, which could fail and leave the node stuck.

**What changed:**

**1. GetHeaders / Headers messages (new P2P protocol messages)**

Two new wire messages — `GetHeaders` and `Headers` — allow a node to download just
the block headers (index, hash, previous_hash, difficulty, cumulative_work) before
requesting any full blocks. A header batch is 500 entries max and is a fraction of
the size of full blocks (which are up to 2 MB each in PQC due to Falcon-512
signatures).

**2. Cumulative work-based peer selection**

The handshake now exchanges `cumulative_work` (sum of all block difficulties on the
chain) alongside `height`. When selecting which peer to sync from, the node picks
the peer with the highest cumulative work — not the highest block height. This
matches Bitcoin's fork selection rule and prevents a malicious peer from getting a
node to follow a low-difficulty long chain.

**3. Headers buffer in the sync engine**

A dedicated `header_buffer` collects incoming headers from `GetHeaders` responses.
The sync loop uses these buffered headers to build a download plan — which height
ranges are missing — then issues targeted `GetBlocks` requests for only those
ranges. This eliminates the scatter-gather pattern that caused blocks to arrive
out of order and be rejected as orphans.

**4. Atomic deep reorg with rollback**

Before this release, a deep reorg that failed partway through (e.g., because the
incoming chain contained a bad block at block 50 of 100) would leave the node at
an inconsistent intermediate height. The node now:

- Saves a snapshot of the current chain's blocks before rolling back
- Applies the new chain blocks one by one
- If any block fails, rolls back the chain pointer and restores the original chain
  from the snapshot before returning an error

The node is never left at a partial reorg state.

**5. Height messages carry cumulative work**

`P2PMessage::Height` now carries `cumulative_work` alongside `height`. Nodes
update both fields on peers during sync, enabling accurate best-peer selection
throughout a long sync rather than only at handshake time.

> This release requires a testnet reset because the cumulative_work field in
> the handshake and Height messages changes the binary wire format. Existing
> v0.6.0 nodes will fail the handshake with v0.7.0 nodes.

---

### Security Fix — Cross-Chain Replay Protection

Added `network_id: u32` to the `Transaction` struct. The field is included in
`get_signing_bytes()` and `hash()`, meaning every Falcon-512 signature is
cryptographically bound to a specific network.

| Network | network_id |
|---|---|
| Testnet (QUA7) | `0` |
| Mainnet | `1` |

A transaction signed on Testnet produces an invalid signature on Mainnet and
vice versa. The field uses `#[serde(default)]` so existing on-chain transactions
deserialize to `network_id = 0` without a genesis change.

---

### Security Fix — State Root Empty-String Bypass Closed

The previous state root check accepted any block with `state_root = ""` as valid,
even when the computed state root did not match. A miner could fabricate account
balances by omitting the state_root field entirely.

Fix: if a block provides a non-empty state_root, it must match the computed value.
Blocks that genuinely omit state_root (pre-feature legacy blocks) continue to pass.

---

### Security Fix — Reorg Path Was Not Verifying Signatures

`validate_block_consensus_reorg()` checked timestamps, PoW, coinbase amounts, and
treasury amounts — but skipped transaction signature verification entirely. An
attacker constructing a longer chain with forged transactions could have them
accepted during a deep reorg.

Fix: the reorg validator now runs the same parallel Rayon signature pass used by
`validate_block_consensus()`. The LRU signature cache is shared between both paths,
so blocks already verified at normal processing time are free to re-apply.

---

### Security Fix — Redundant Serial Signature Verification Removed

`block.is_valid()` ran a serial Falcon-512 verification loop over all transactions,
then `validate_block_consensus()` ran an identical parallel Rayon pass immediately
after. Every block's signatures were being verified twice, adding ~1800 ms of
redundant PQC work per block.

`block.is_valid()` now only checks structural integrity: hash, PoW, Merkle root,
and chain linkage. All signature verification is owned by the parallel Rayon pass.

---

### Security Fix — Inbound Peer Connection Cap

`listen_for_connections()` previously accepted every inbound TCP connection before
`PeerManager.add_peer()` could enforce the `max_peers` limit. A botnet could
exhaust OS connection slots before the limit check ran.

Fix: the node checks `peer_manager.peer_count()` before accepting the TCP stream.
If the node is at capacity, the stream is dropped immediately (TCP RST).

---

### Improvement — network_id Propagated from Node Config

Coinbase and treasury system transactions now read `network_id` from the node's
configured `ChainNetwork` via `self.network.network_id()` instead of a hardcoded
`0`. Mainnet nodes will correctly stamp `network_id = 1` on all system-generated
transactions from launch.

### Improvement — Light Block Gossip

`broadcast_block()` previously sent the full 2 MB block to every connected peer
on every new block found. This is now a header-only announcement (~200 bytes).
Peers that need the full block request it via `GetBlocks`. Per-block broadcast
bandwidth drops from O(peers x 2 MB) to O(peers x 200 B).

---

## What Changed in Alpha v0.6.0

### Critical Fix — Block Template Nonce Sequence (Network Stall Fix)

The mempool block assembler sorted transactions by descending fee without enforcing
nonce ordering. A user submitting multiple transactions could have them included
out of sequence, causing the consensus engine to reject the block with `InvalidNonce`.

Block templates now use a simulated state buffer that enforces absolute sequential
nonce ordering regardless of fee priority.

### Critical Fix — Permanent Nonce Desync on Reorg

The `reorg_to_block` handler reapplied balances but did not call `increment_nonce()`.
Any sender whose transaction landed in a reorg block became permanently unable to
send further transactions (on-chain nonce stuck at 0).

### Critical Fix — Faucet Balance Zero After Sync

Genesis premine transactions were applied in-memory during `Blockchain::new()` but
not stored inside the genesis block struct on disk. `rebuild_account_state_up_to()`
iterated `genesis.transactions` (always empty on disk) and applied no premine, so
all faucet wallets showed 0 QUA after any deep reorg.

### Critical Fix — Sync Stuck at Block 1

`MIN_DIFFICULTY` was higher than the actual difficulty of early testnet blocks,
causing every incoming block at those heights to be rejected immediately.

---

## What Changed in Alpha v0.5.0

- LWMA (Linearly Weighted Moving Average) difficulty algorithm. Adjusts every block
  on a 45-block window (~22.5 min). Replaces 2016-block Bitcoin-style intervals.
- `deep_reorg()` multi-block reorganisation engine.
- Parallel Rayon signature verification with LRU cache (serial ~1800 ms to parallel
  ~300 ms for a full 1200-tx block).
- Bloom filter for O(1) mempool duplicate detection.
- Atomic orphan pool.

---

## What Changed in Alpha v0.3.0 (Testnet V2)

- Testnet V2 genesis reset with realistic difficulty (6,972,889).
- Bitcoin-style DoSMan weighted peer scoring (0-100) replacing flat 3-strike system.
- Block explorer API: address history, tx lookup, latest blocks.
- Subnet Sybil protection (IPv4 /24, IPv6 /48).
- Persistent IP ban list.

---

## What Changed in Alpha v0.2.0

- Mnemonic-based faucet wallet system (10 reserve wallets, BIP-39 derived).
- Security audit: nonce atomicity, coinbase validation, MTP timestamp rule,
  per-sender mempool cap, state root enforcement.
- Block size increased to 2 MB for Falcon-512 transaction sizes.
- License changed to Apache 2.0.

---

## Security Notice

This release has undergone internal audit only. It has not been formally verified
by a third-party security firm. Do not use for real financial transactions.

Falcon-512 and Kyber-1024 implementations are based on NIST PQC Round 3 finalists.

---

## License

Apache 2.0 — see [LICENSE](LICENSE)

"Quanta" and "QuantaChain" are trademarks. Forks may not use these names without permission.
