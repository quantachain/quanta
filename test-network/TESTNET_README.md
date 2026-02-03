# QUANTA Local Testnet Guide

This directory contains a configured local testnet with 3 nodes running in Docker.

## Configuration
- **Node 1 (Bootstrap)**
  - API: `http://localhost:3001`
  - P2P: 8331
  - Logs: `docker logs quanta-node-1`
- **Node 2**
  - API: `http://localhost:3002`
  - P2P: 8332
  - Logs: `docker logs quanta-node-2`
- **Node 3**
  - API: `http://localhost:3003`
  - P2P: 8333
  - Logs: `docker logs quanta-node-3`

## Quick Commands

### Start Network
```powershell
cd d:\tempp\qua\test-network
docker-compose up -d
```

### Stop Network
```powershell
docker-compose down
```

### Check Connectivity (Peer Count)
```powershell
# Node 1 Peer Count
(Invoke-RestMethod http://localhost:3001/api/peers).peer_count

# Node 2 Peer Count
(Invoke-RestMethod http://localhost:3002/api/peers).peer_count

# Node 3 Peer Count
(Invoke-RestMethod http://localhost:3003/api/peers).peer_count
```

### Mine a Block (Node 1)
```powershell
Invoke-RestMethod -Uri "http://localhost:3001/api/mine" -Method Post -ContentType "application/json" -Body '{"miner_address": "test_miner"}'
```

### Check Blockchain Height
```powershell
(Invoke-RestMethod http://localhost:3001/api/blockchain/info).chain_height
(Invoke-RestMethod http://localhost:3002/api/blockchain/info).chain_height
(Invoke-RestMethod http://localhost:3003/api/blockchain/info).chain_height
```

## Logs & Debugging
View logs for a specific node:
```bash
docker logs -f quanta-node-1
```

## Data Persistence
Blockchain data is stored in the `data/` subdirectory for each node (e.g., `node1/data`). To reset the network completely:
```powershell
docker-compose down
Remove-Item -Recurse -Force node1/data, node2/data, node3/data
docker-compose up -d
```
