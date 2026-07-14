# QuantaChain Testnet — V2 Release (v2.2.12-alpha)

Post-quantum secure blockchain using Falcon-512 signatures and **Asynchronous Byzantine Fault Tolerance (AlephBFT)**.

> **v2.2.12-alpha — TIME WARP RECOVERY FIX (2026-07-14)**
> **NO WIPE REQUIRED.** Hot-fix release addressing the network stall that occurred when trying to recover from the Time Warp DOS.
> * **Time Warp Recovery**: Reverted the 15-second block limit back to 2 hours. Because the network's tip was already 2 hours in the future due to the exploit, enforcing a 15-second limit caused all new (healing) blocks to be rejected. The root cause (using chain-time instead of real-time to trigger block creation) remains fixed in `bft_proposer`.
> * **DAG Cleanup**: Added logic to automatically clear old `alephbft_backup` files at session boundaries, ensuring validators don't get stuck waiting for parents from aborted runs.
> * **Network Magic Updated**: Changed network magic to `Q8TE`. Ensure all nodes in your cluster are updated.

> **v2.2.11-alpha — TIME WARP DOS HOTFIX (2026-07-14)**
> **NO WIPE REQUIRED.** Hot-fix release addressing a critical vulnerability where blocks with future timestamps could completely halt network consensus.
> * **Time Warp Protection**: Reduced maximum allowed future timestamp drift from 2 hours to 15 seconds.
> * **Monotonic Timestamps**: Block validation now strictly enforces that block timestamps must always move forward, preventing DOS loops in AlephBFT data provisioning.
> * **Network Magic Updated**: Changed network magic to `Q7TE`. Ensure all nodes in your cluster are updated.

> **v2.2.10-alpha — SYBIL PROTECTION FIX (2026-07-13)**
> **NO WIPE REQUIRED.** Hot-fix release increasing the Sybil connection limit from 2 to 100 per IP.
> * **Sybil Limit Increased**: Increased the maximum connections allowed from a single IP to allow multiple validators to run on the same VPS without being banned.
> * **Network Magic Updated**: Changed network magic to `Q6TE`. Ensure all nodes in your cluster are updated.

> **v2.2.9-alpha — DEVNET MODE & MERKLE PROOF FIX (2026-07-13)**
> **NO WIPE REQUIRED.** Hot-fix release adding tools for automated cloud deployment and addressing an SPV proof bug.
> * **Devnet Mode**: Added `--devnet <ID>` and `--devnet-nodes <N>` flags to auto-bootstrap private networks on Azure/AWS without manual wallet configuration.
> * **Merkle Proof Bugfix**: Fixed a bug where odd-sized SPV subtrees would fail verification due to a midpoint rounding error. Proofs now correctly mirror the `ceil(n/2)` split.

> **v2.2.8-alpha — BFT CONSENSUS FIX & LOG CLEANUP (2026-07-08)**
> **NO WIPE REQUIRED.** Hot-fix release targeting two regressions introduced in v2.2.6:
> * **Block production freeze**: Fixed a write-lock deadlock in `Peer::send_message` that caused AlephBFT to starve while all nodes appeared Online. Blocks were not produced despite a fully-connected validator set.
> * **Log spam eliminated**: Raw AlephBFT byte-arrays were being printed with `{:?}`, flooding operator logs with unreadable binary blobs. Logs now show compact labels like `AlephBFT(342 bytes)`, `Block(#1234)`, etc.

> **v2.2.0 — MAJOR CONSENSUS & STAKING UPGRADE (2026-07-06)**
> **WIPE REQUIRED.** A critical bug with deterministic BFT payload hashing (`invalid Falcon-512 sig`) has been fully resolved! External nodes can now successfully propose and verify blocks on the AlephBFT network.
> You MUST delete your `quanta_data` folder before starting this version! The network magic has changed to `Q2TE`.
> 
> **How to update and start your node:**
> 
> ```bash
> # 1. Stop and remove the old container
> docker stop quanta-validator && docker rm quanta-validator
> 
> # 2. Delete the old blockchain data (REQUIRED!)
> sudo rm -rf ~/quanta_data_v2
> 
> # 3. Pull the new image and start
> docker pull xd637/quanta-node:latest
> 
> docker run -d \
>   --name quanta-validator \
>   --restart always \
>   --network host \
>   -v ~/quanta_data_v2:/home/quanta/quanta_data \
>   -e QUANTA_WALLET_PASSWORD="YOUR_PASSWORD_HERE" \
>   xd637/quanta-node:latest \
>   quanta start --validator-wallet /home/quanta/quanta_data/validator.qua --bootstrap 34.87.128.33:8333
> ```
> 
> **Staking is fully OPEN!** We have successfully tested dynamic BFT committee rotation with external nodes! `OPEN_VALIDATOR_REGISTRATION_HEIGHT = 0` so new validators can stake and join instantly at the next session boundary!

> **v2.1.1-alpha — SOFT UPDATE (2026-07-06)**
> **NO WIPE REQUIRED.** A bypass was added for block 12615 to resolve the state root consensus bug. The blockchain data format and network magic (`Q2T9`) have not changed. DO NOT delete your `quanta_data` folder!

This is a pre-release testnet build. Do not use real funds. APIs and chain parameters may change between alpha releases.

---


## Genesis Block

| Parameter | Value |
|---|---|
| Network | Testnet (QUA7) |
| Timestamp | `1780704001` (2026-06-06 00:00:01 UTC) |
| Testnet Genesis Hash | `ae37fe2f40a7e7dbe6d2d1337f260d57185ef5fb169008e2600f245809fd1fbf` |
| Faucet 0 (API sender) | `0xec4f49553e31f22b27a83036a044aff7d697f524` |
| Block Time | Exactly 6 seconds |
| TPS Limit | ~250 - 300 TPS (assuming 2MB block limit) |

---

## 🔄 Clean Start Guide (Wipe Data & Resync from Genesis)

> **You MUST perform a clean start to join the V2 Testnet!**
> The V2 BFT consensus engine uses a different block structure and state machine. It will crash if it reads old V1 PoW blocks.

### Bare Metal / VPS (no Docker)

```bash
pkill -f "quanta start"    # stop the old node
rm -rf ./quanta_testnet_data  # WIPE THE OLD POW CHAIN DATA!
cargo build --release      # compile the new V2 binary
./target/release/quanta start -c quanta.toml
```

### Docker Validators (Safe Wipe)

If you are an existing validator using Docker, you must wipe the old chain data but **KEEP your validator wallet file**. Since you may have named your wallet file something other than `validator.qua`, you must be extremely careful.

> [!CAUTION]
> If you run the `rm -rf` command below without successfully moving your wallet out first, you will delete your validator wallet forever!

**Step 1:** Stop your node and move your wallet out of the data folder. (Replace `validator.qua` with your actual wallet filename if it is different!)
```bash
docker stop quanta-validator
mv ~/quanta_data_v2/validator.qua ~/validator.qua.backup
```

**Step 2:** Verify your wallet is safe in your home directory, then wipe the old data.
```bash
ls ~/validator.qua.backup     # Verify it exists!
sudo rm -rf ~/quanta_data_v2/*
```

**Step 3:** Move your wallet back in (again, replacing `validator.qua` with your actual filename if different), and restart the node.
```bash
mv ~/validator.qua.backup ~/quanta_data_v2/validator.qua
docker start quanta-validator
```

---

## Validator Setup (Docker)

> **🟢 STAKING IS NOW OPEN!** The network has successfully transitioned to full DPoS mechanics. Anyone who stakes at least **100,000 QUA** can run a validator node, propose blocks, and earn rewards! 
> 
> If you have the required QUA, you can follow these exact instructions to spin up your validator node and connect to the core network using Docker:

**1. Create a Wallet and Get Your Key**
You must generate a raw wallet and provide the public key to the core team to be whitelisted in the Genesis block.
```bash
docker run --rm -it xd637/quanta-node:latest quanta-wallet new-raw --file /tmp/validator.qua
```

**2. Directory Setup**
Create the directory where your blockchain data will live. We recommend adding `_v2` to avoid mixing it with any old testnet data:
```bash
mkdir -p ~/quanta_data_v2
```

**3. Place Your Wallet File**
Move your validator wallet file (e.g., `validator.qua` or whatever you named it) directly into the `~/quanta_data_v2` directory you just created.

Your folder should look exactly like this:
```text
~/quanta_data_v2/
└── validator.qua
```
*(Note: You do NOT need the `genesis.json` or `quanta.toml` files! The latest network configuration is securely baked directly into the V2 Docker image.)*

**4. Pull the Latest Image & Start the Node**
Run the following Docker commands to pull the latest V2 build, launch your node, connect to the Bootstrap node, and begin proposing blocks. 

> [!IMPORTANT]
> **Before running the second command below, you MUST change two things:**
> 1. Change `"YOUR_PASSWORD_HERE"` to your actual wallet password.
> 2. Change `validator.qua` at the very end of the command to match the exact name of your wallet file.

```bash
docker pull xd637/quanta-node:latest

docker run -d \
  --name quanta-validator \
  --restart always \
  --network host \
  -v ~/quanta_data_v2:/home/quanta/quanta_data \
  -e QUANTA_WALLET_PASSWORD="YOUR_PASSWORD_HERE" \
  xd637/quanta-node:latest \
  quanta start --validator-wallet /home/quanta/quanta_data/validator.qua --bootstrap 34.87.128.33:8333
```

---

## Wallet Management

```bash
# New HD Wallet (Recommended)
quanta-wallet new --file my_wallet.json

# New Raw Wallet
quanta-wallet new-raw --file my_raw.qua

# AI Headless Mode (Set env var to skip password prompts)
export QUANTA_WALLET_PASSWORD="your_password"
quanta-wallet info --file my_wallet.json
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

## License

QUANTACHAIN operates under an **Open-Core Dual License** model:

| Component | License |
|---|---|
| Core Protocol | [GNU AGPLv3](https://github.com/quantachain/quanta/blob/main/LICENSE) |
| Native Templates & APIs | [QuantaLabs Commercial License](https://github.com/quantachain/quanta/blob/main/COMMERCIAL_LICENSE.md) |

For commercial licensing: **contact@quantachain.org**
