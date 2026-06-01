# Quick Start

The V2 AlephBFT testnet requires a completely fresh start. Legacy V1 data (`quanta_data`) is entirely incompatible and must be deleted.

## 1. Wipe Old Data (CRITICAL)
If you ran a node before V2, you must wipe the state:
```bash
rm -rf ./quanta_data
```

## 2. Start the Node (Docker)
```bash
docker run -d \
  --name quanta-node \
  --restart always \
  -p 3000:3000 -p 8333:8333 -p 7782:7782 -p 9090:9090 \
  -v quanta-data:/home/quanta/quanta_data \
  xd637/quanta-node:latest
```

## 3. Verify Sync
The V2 network uses magic bytes `Q2T2` to prevent old nodes from connecting.
```bash
curl http://localhost:3000/blocks/latest
```
You should see blocks incrementing exactly every 6 seconds!
