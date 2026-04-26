# Quick Start

Get a Quanta node running, create a wallet, and start mining in under 10 minutes using Docker.

---

## Step 1: Start the Node

```bash
docker pull xd637/quanta-node:latest

docker run -d \
  --name quanta-node \
  --restart always \
  -p 3000:3000 -p 8333:8333 -p 7782:7782 -p 9090:9090 \
  -v quanta-data:/home/quanta/quanta_data \
  -v quanta-logs:/home/quanta/logs \
  xd637/quanta-node:latest
```

Wait 10–20 seconds, then verify the node is syncing:

```bash
docker logs quanta-node --tail 20
curl http://localhost:3000/health
```

---

## Step 2: Create a Wallet

Create an HD wallet with a 24-word recovery phrase. This is the recommended wallet type.

```bash
docker exec -it quanta-node quanta new_hd_wallet --file hd_wallet.json
```

You will be prompted to set a password. After creation, note your:
- **24-word mnemonic** — store this offline; it is the only way to recover your wallet
- **Address** — starts with `0x`

Show your wallet address at any time (no node required):

```bash
docker exec -it quanta-node quanta wallet_address --file hd_wallet.json
```

---

## Step 3: Check Node Status

```bash
# Node health and sync status
docker exec -it quanta-node quanta status --rpc-port 7782

# Current block height
docker exec -it quanta-node quanta print_height --rpc-port 7782

# Connected peers
docker exec -it quanta-node quanta peers --rpc-port 7782
```

Let the node sync with the network before mining. This may take a few minutes on the testnet.

---

## Step 4: Start Mining

Replace `YOUR_ADDRESS` with your wallet address from Step 2:

```bash
docker exec -d quanta-node quanta start_mining YOUR_ADDRESS --rpc-port 7782
```

Monitor mining:

```bash
docker exec -it quanta-node quanta mining_status --rpc-port 7782
```

Stop mining:

```bash
docker exec -it quanta-node quanta stop_mining --rpc-port 7782
```

---

## Step 5: Send a Transaction

Amounts are in **microunits** — 1 QUA = 1,000,000 microunits.

```bash
docker exec -it quanta-node quanta send \
  --wallet hd_wallet.json \
  --to 0xRECIPIENT_ADDRESS \
  --amount 5000000 \
  --db /home/quanta/quanta_data
```

This sends 5 QUA. Check the balance of any address:

```bash
curl http://localhost:3000/accounts/0xYOUR_ADDRESS/balance
```

---

## Testnet Faucet

Need testnet QUA? Request funds from the faucet at [quantachain.org/faucet](https://www.quantachain.org/faucet).

---

## Common Commands Reference

```bash
# Node
docker exec -it quanta-node quanta status          --rpc-port 7782
docker exec -it quanta-node quanta print_height    --rpc-port 7782
docker exec -it quanta-node quanta peers           --rpc-port 7782
docker exec -it quanta-node quanta stop            --rpc-port 7782

# Wallet
docker exec -it quanta-node quanta new_hd_wallet   --file hd_wallet.json
docker exec -it quanta-node quanta wallet_address  --file hd_wallet.json
docker exec -it quanta-node quanta hd_wallet       --file hd_wallet.json

# Mining
docker exec -d  quanta-node quanta start_mining  ADDR  --rpc-port 7782
docker exec -it quanta-node quanta mining_status       --rpc-port 7782
docker exec -it quanta-node quanta stop_mining         --rpc-port 7782
```

---

## Upgrade the Node

```bash
docker pull xd637/quanta-node:latest
docker stop quanta-node && docker rm quanta-node

docker run -d \
  --name quanta-node \
  --restart always \
  -p 3000:3000 -p 8333:8333 -p 7782:7782 -p 9090:9090 \
  -v quanta-data:/home/quanta/quanta_data \
  -v quanta-logs:/home/quanta/logs \
  xd637/quanta-node:latest
```

Your `quanta-data` volume persists across upgrades.

---

## Next Steps

- [Mining Guide](mining-guide.md) — reward structure, optimization tips
- [Node Operator Guide](node-operator-guide.md) — VPS, NGINX, SSL setup
- [API Reference](api-reference.md) — REST endpoints for integrations
