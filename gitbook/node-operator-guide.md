# Node Operator Guide

Quanta V2 has removed Proof-of-Work mining. Security is now provided by BFT Validators in a permissionless Testnet, allowing anyone to participate as a full node or stake as a validator.

## Minimum System Requirements

To ensure stable block production and AlephBFT consensus participation, validator nodes must meet the following hardware requirements:

- **CPU:** 4 Cores (modern x86_64 or ARM64)
- **RAM:** 8 GB
- **Storage:** 50 GB SSD (NVMe recommended)
- **Network:** Reliable 100 Mbps connection with static IP

## Node Types

You can run your node in three different storage modes, configured in your `quanta.toml`.

### 1. Archive Node (Default)
`mode = "archive"`
An archive node downloads and stores the entire history of the blockchain forever. This is essential for block explorers and API providers, but it requires the most disk space.

### 2. Pruned Node
`mode = "pruned"`
A pruned node only keeps the last 30 days of blocks (configurable via `prune_days = 30`). It discards old block bodies but keeps the cryptographically proven UTXO state. This is highly recommended for Validators who want to save disk space.

### 3. Light Node
`mode = "light"`
A light node only downloads block headers and validates the BFT signatures. It has an incredibly small footprint but cannot serve block data to other peers over the network.

## Running a Node

To run a standard node (non-validator) to serve the REST API and support the network:

```bash
./quanta start -c quanta.toml --bootstrap node1.quantachain.org:8333
```

## Running a Validator

If you want to participate in AlephBFT consensus and produce blocks, you must run your node as a validator.

1. **Generate a Raw Wallet**
   ```bash
   quanta-wallet new-raw --file validator.qua
   ```

2. **Register your Validator**
   *You must stake a minimum of 100,000 QUA to register your validator on the network.*
   ```bash
   quanta-wallet stake --wallet validator.qua --amount 100000.0
   ```

3. **Start Node as Validator**
   ```bash
   ./quanta start -c quanta.toml --validator-wallet validator.qua --bootstrap node1.quantachain.org:8333
   ```

4. **Unstake and Deregister**
   *If you want to gracefully leave the network and begin the unbonding process to retrieve your stake:*
   ```bash
   quanta-wallet unstake --wallet validator.qua
   ```

## Configuration (`quanta.toml`)

You can override default ports and settings in `quanta.toml`:

```toml
[node]
mode = "pruned"
prune_days = 30
db_path = "./quanta_data"

[network]
network_port = 8333
max_peers = 125
```

*Note: The V3 network magic bytes are `QT35` for the current testnet phase.*
