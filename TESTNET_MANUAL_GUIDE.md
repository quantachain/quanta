# Quanta Node - Manual Verification Guide

This guide provides instructions to run and verify a single Quanta node using Docker or CLI.

## 1. Run via Docker (Recommended)

Start the node and monitoring stack (Prometheus/Grafana):

```bash
docker-compose up --build -d
```

### Check Logs
```bash
docker logs -f quanta-node
```

### Check Status
```bash
curl -s http://localhost:3000/api/stats | jq
```

## 2. Wallet Management

You need a wallet to mine. Create one inside the container:

```bash
# Create wallet (password: test123)
docker exec -it quanta-node quanta new-wallet --file /home/quanta/quanta_data/wallet.qua --password test123

# View Address
docker exec -it quanta-node quanta wallet --file /home/quanta/quanta_data/wallet.qua --password test123
```

## 3. Start Mining

Start the built-in miner using your wallet address:

```bash
# Replace <ADDRESS> with your wallet address
docker exec -d quanta-node quanta start_mining <ADDRESS> --rpc-port 7782
```

## 4. Monitor Mining

Check stats again to see block height increasing:

```bash
watch -n 5 "curl -s http://localhost:3000/api/stats"
```

You can also view metrics in Grafana:
- URL: http://localhost:3030
- Login: admin / quanta2026

## 5. Stop Mining & Shutdown

Stop miner:
```bash
docker exec quanta-node quanta stop_mining --rpc-port 7782
```

Shutdown node:
```bash
docker-compose down
```

## 6. Run via CLI (Alternative)

If you prefer running without Docker:

```bash
# Build
cargo build --release

# Start Node
./target/release/quanta start --port 3000 --network-port 8333 --rpc-port 7782

# (In new terminal) Create Wallet
./target/release/quanta new-wallet --file wallet.qua

# Mine
./target/release/quanta start_mining <ADDRESS> --rpc-port 7782
```
