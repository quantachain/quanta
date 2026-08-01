# Quick Start

The V2 AlephBFT testnet requires a completely fresh start. Legacy V1 data (`quanta_data`) is entirely incompatible and must be deleted.

## 1. Wipe Old Data (CRITICAL)
If you ran a node before V2, you must wipe the state:
```bash
rm -rf ./quanta_data
```

## 2. Start the Node (Docker)

To run a regular observer node that simply syncs the chain and serves the REST API:
```bash
docker run -d \
  --name quanta-node \
  --restart always \
  -p 3000:3000 -p 8333:8333 -p 7782:7782 -p 9090:9090 \
  -v quanta-data:/home/quanta/quanta_data \
  xd637/quanta-node:latest \
  quanta start --bootstrap node1.quantachain.org:8333
```

### Starting as a Validator
Quanta V2 is permissionless! Anyone who stakes 100,000 QUA can become a validator. First, you must register your validator by staking:

```bash
quanta-wallet stake --wallet validator.qua --amount 100000.0
```

Then, you must mount your raw wallet file into the container and pass the `--validator-wallet` flag. Set your password as an environment variable so the node can decrypt the wallet on startup.

```bash
docker run -d \
  --name quanta-validator \
  --restart always \
  -e QUANTA_WALLET_PASSWORD="<YOUR_PASSWORD>" \
  -p 3000:3000 -p 8333:8333 -p 7782:7782 -p 9090:9090 \
  -v quanta-data:/home/quanta/quanta_data \
  -v /absolute/path/to/validator.qua:/home/quanta/validator.qua \
  xd637/quanta-node:latest \
  quanta start --validator-wallet /home/quanta/validator.qua --bootstrap node1.quantachain.org:8333
```

## 3. Verify Sync
The V3 network uses magic bytes `QT35` to prevent old nodes from connecting.
```bash
curl http://localhost:3000/api/blocks/latest
```
You should see blocks incrementing reliably thanks to AlephBFT!
