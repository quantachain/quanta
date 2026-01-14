# P2P Networking

## Overview

Quanta uses a custom peer-to-peer protocol for blockchain synchronization and transaction propagation.

## Network Identification

- **Testnet Magic**: `QUAX` (0x51554158)
- **Mainnet Magic**: `QUAM` (0x5155414D)

## Message Types

### Handshake
- `Version`: Exchange protocol version and capabilities
- `VerAck`: Acknowledge version message

### Discovery
- `GetAddr`: Request peer addresses
- `Addr`: Share known peer addresses

### Synchronization
- `GetBlocks`: Request blocks by height range
- `Block`: Send block data
- `GetHeaders`: Request block headers
- `Headers`: Send block headers
- `GetHeight`: Request current blockchain height
- `Height`: Send current height

### Transactions
- `NewTx`: Broadcast new transaction
- `GetMempool`: Request pending transactions
- `Mempool`: Send mempool contents

### Maintenance
- `Ping`: Keep-alive message
- `Pong`: Ping response
- `Disconnect`: Graceful disconnection

## Peer Discovery

### Bootstrap Nodes

Hardcoded bootstrap nodes for initial connection:

**Testnet**:
- testnet-us-east.quanta.network:8333
- testnet-us-west.quanta.network:8333
- testnet-eu-west.quanta.network:8333
- testnet-eu-central.quanta.network:8333
- testnet-ap-southeast.quanta.network:8333
- testnet-ap-northeast.quanta.network:8333

### DNS Seeds

DNS-based peer discovery:
- seed.testnet.quantachain.org

DNS seeds return multiple IP addresses for decentralized discovery.

### Peer Exchange

Nodes share known peer addresses via `Addr` messages, enabling organic network growth.

## Connection Management

### Limits
- Maximum peers: 125
- Maximum message size: 2 MB
- Ping interval: 60 seconds
- Peer timeout: 180 seconds

### Connection Flow

1. Connect to bootstrap node or DNS seed
2. Send `Version` message
3. Receive `VerAck` acknowledgment
4. Exchange `GetAddr` to discover more peers
5. Maintain connection with periodic `Ping`/`Pong`

## Block Propagation

1. Miner mines valid block
2. Broadcast to all connected peers
3. Peers validate block
4. Valid blocks added to chain
5. Peers rebroadcast to their connections
6. Full network propagation target: <5 seconds

## Security

### DoS Protection
- 2 MB message size limit
- Connection limits per IP range
- Invalid message handling: Automatic peer disconnection

### Sybil Resistance
- Proof-of-work for block production
- Connection limits
- Peer reputation (future enhancement)

## Configuration

Configure networking in `quanta.toml`:

```toml
[network]
max_peers = 125
bootstrap_nodes = [
    "testnet-us-east.quanta.network:8333",
]
dns_seeds = ["seed.testnet.quantachain.org"]
```

## Port Forwarding

For optimal connectivity, forward port 8333 (or your custom P2P port) in your router settings.

## Firewall Configuration

Allow incoming and outgoing connections on:
- P2P port (default 8333)
- API port (default 3000) - optional, for public API
- RPC port (default 7782) - localhost only recommended

## Monitoring Peers

View connected peers:

```bash
./target/release/quanta peers
```

Or via JSON-RPC:

```bash
curl -X POST http://localhost:7782 \
  -H "Content-Type: application/json" \
  -d '{"jsonrpc":"2.0","method":"get_peers","params":[],"id":1}'
```
