# Mining Guide

Complete guide for mining Quanta (QUA) cryptocurrency.

## Overview

Quanta uses CPU-based Proof-of-Work with SHA3-256 hashing. Mining rewards start at 100 QUA per block and decrease by 15% annually.

## System Requirements

- CPU: 4 cores at 2.0 GHz minimum (more cores = better)
- RAM: 8 GB minimum
- Storage: 1 TB SSD
- Bandwidth: 50 Mbps down, 20 Mbps up
- Stable internet connection

## Setup

### 1. Install and Build

```bash
git clone https://github.com/quantachain/quanta.git
cd quanta
cargo build --release
```

### 2. Create Mining Wallet

```bash
./target/release/quanta new_hd_wallet --file miner.qua
```

Save your mnemonic phrase securely.

### 3. Get Your Address

```bash
./target/release/quanta hd_wallet --file miner.qua
```

Note your wallet address for mining rewards.

### 4. Start Node

```bash
./target/release/quanta start --detach
```

## Start Mining

### Using CLI

```bash
./target/release/quanta start_mining YOUR_WALLET_ADDRESS
```

### Using JSON-RPC

```bash
curl -X POST http://localhost:7782 \
  -H "Content-Type: application/json" \
  -d '{"jsonrpc":"2.0","method":"start_mining","params":["YOUR_ADDRESS"],"id":1}'
```

## Monitor Mining

### Check Mining Status

```bash
./target/release/quanta mining_status
```

### View Blockchain Height

```bash
./target/release/quanta print_height
```

### Check Your Balance

```bash
curl -X POST http://localhost:3000/api/balance \
  -H "Content-Type: application/json" \
  -d '{"address": "YOUR_ADDRESS"}'
```

## Stop Mining

```bash
./target/release/quanta stop_mining
```

## Mining Rewards

### Year 1 Economics

- Base reward: 100 QUA per block
- Block time: 10 seconds
- Daily blocks: ~8,640
- Daily emission: ~864,000 QUA
- Plus 10% of transaction fees

### Early Adopter Bonus

First 100,000 blocks (~11.5 days):
- Multiplier: 1.5x
- Reward: 150 QUA per block

### Reward Lock

50% of mining rewards are locked for 6 months (157,680 blocks) to prevent dumping and encourage long-term participation.

## Profitability

### Revenue Calculation

```
Block Reward: 100 QUA
Block Time: 10 seconds
Blocks per Day: 8,640
Daily Potential: 864,000 QUA (network-wide)
Your Share: (Your Hashrate / Network Hashrate) * Daily Potential
```

### Costs

- Electricity: Varies by location
- Hardware: Initial investment + depreciation
- Internet: Bandwidth costs

## Optimization

### CPU Optimization

- Use high-performance CPUs with many cores
- Enable CPU governor for maximum performance
- Ensure adequate cooling

### Network Optimization

- Use wired ethernet connection
- Minimize network latency
- Connect to nearby peers

### System Optimization

- Disable unnecessary background processes
- Use SSD for blockchain data
- Allocate sufficient RAM

## Testnet Mining

### Start Testnet

```bash
docker-compose -f docker-compose.testnet.yml up --build -d
```

### Get Testnet Wallet Address

```bash
docker exec -e QUANTA_WALLET_PASSWORD=testnet_insecure_password \
  quanta-testnet-node1 \
  /usr/local/bin/quanta wallet_address --file wallet.qua
```

### Start Testnet Mining

```bash
docker exec quanta-testnet-node1 \
  /usr/local/bin/quanta start_mining YOUR_ADDRESS --rpc-port 17782
```

### Monitor Testnet

```bash
curl -s http://localhost:13000/api/stats
```

### Stop Testnet Mining

```bash
docker exec quanta-testnet-node1 \
  /usr/local/bin/quanta stop_mining --rpc-port 17782
```

## Troubleshooting

### Mining Not Starting

Check node is fully synced:
```bash
./target/release/quanta status
```

### Low Hashrate

- Check CPU usage is at 100%
- Verify no thermal throttling
- Close other applications

### No Rewards

- Verify mining address is correct
- Check network hashrate vs your hashrate
- Mining is probabilistic - rewards take time

## Best Practices

- Keep node software updated
- Monitor system temperature
- Backup wallet regularly
- Join community for updates
- Calculate profitability before investing in hardware
