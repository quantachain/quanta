# JSON-RPC API Guide

The QUANTA V2 daemon exposes a fast, stateless JSON-RPC API for interacting with the blockchain. Unlike the REST API (which is primarily used for node administration), the JSON-RPC API is designed for building block explorers, wallets, and dApps that need to query chain data efficiently.

By default, the RPC server binds to `127.0.0.1:3030`. You can configure this in your `quanta.toml`:

```toml
[node]
rpc_port = 3030
```

> [!WARNING]
> Do not expose the RPC port directly to the public internet without a reverse proxy (like NGINX or HAProxy) providing TLS termination and rate-limiting.

---

## Making Requests

All requests must be `POST` requests sent to the root path (`/`) with a `Content-Type: application/json`. The payload must follow the standard JSON-RPC 2.0 format.

**Example Request Format:**
```json
{
  "jsonrpc": "2.0",
  "method": "METHOD_NAME",
  "params": { ... },
  "id": 1
}
```

---

## Available Endpoints

### 1. `node_status`
Returns general health and sync information about the node.

**Request:**
```bash
curl -X POST http://127.0.0.1:3030/ -H "Content-Type: application/json" -d '{
  "jsonrpc": "2.0",
  "method": "node_status",
  "params": {},
  "id": 1
}'
```

**Response:**
```json
{
  "jsonrpc": "2.0",
  "result": {
    "running": true,
    "chain_height": 10425,
    "peer_count": 8,
    "mempool_size": 12,
    "api_port": 3000,
    "network_port": 8333,
    "rpc_port": 3030,
    "uptime_seconds": 3600,
    "version": "2.0.2"
  },
  "id": 1
}
```

### 2. `get_block`
Retrieves detailed information about a specific block by its height.

**Request:**
```bash
curl -X POST http://127.0.0.1:3030/ -H "Content-Type: application/json" -d '{
  "jsonrpc": "2.0",
  "method": "get_block",
  "params": {
    "height": 10425
  },
  "id": 1
}'
```

**Response:**
```json
{
  "jsonrpc": "2.0",
  "result": {
    "height": 10425,
    "hash": "0xabc123...",
    "timestamp": 1720084532,
    "transactions": 5,
    "epoch": 2,
    "bft_round": 1,
    "proposer": "0xvalidator...",
    "sig_count": 3
  },
  "id": 1
}
```

### 3. `get_balance`
Retrieves the confirmed balance of a specific address.

**Request:**
```bash
curl -X POST http://127.0.0.1:3030/ -H "Content-Type: application/json" -d '{
  "jsonrpc": "2.0",
  "method": "get_balance",
  "params": {
    "address": "0xuseraddress..."
  },
  "id": 1
}'
```

**Response:**
```json
{
  "jsonrpc": "2.0",
  "result": {
    "address": "0xuseraddress...",
    "balance": 1000000000,
    "balance_qua": 1000.0
  },
  "id": 1
}
```

### 4. `get_mempool`
Returns all unconfirmed transactions currently sitting in the node's mempool awaiting inclusion in the next block.

**Request:**
```bash
curl -X POST http://127.0.0.1:3030/ -H "Content-Type: application/json" -d '{
  "jsonrpc": "2.0",
  "method": "get_mempool",
  "params": {},
  "id": 1
}'
```

**Response:**
```json
{
  "jsonrpc": "2.0",
  "result": {
    "transactions": [
      {
        "sender": "0xalice...",
        "recipient": "0xbob...",
        "amount": 5000000,
        "fee": 1000,
        "nonce": 42,
        "timestamp": 1720084530
      }
    ]
  },
  "id": 1
}
```

### 5. `get_peers`
Returns a list of connected network peers (useful for network explorers or topology mapping).

**Request:**
```bash
curl -X POST http://127.0.0.1:3030/ -H "Content-Type: application/json" -d '{
  "jsonrpc": "2.0",
  "method": "get_peers",
  "params": {},
  "id": 1
}'
```

**Response:**
```json
{
  "jsonrpc": "2.0",
  "result": [
    {
      "address": "104.248.88.232:8333",
      "connected_since": 1720080932,
      "last_seen": 1720084531
    }
  ],
  "id": 1
}
```

### 6. `shutdown`
Gracefully shuts down the QUANTA daemon. This endpoint is typically only accessible if the RPC port is firewalled to `localhost`.

**Request:**
```bash
curl -X POST http://127.0.0.1:3030/ -H "Content-Type: application/json" -d '{
  "jsonrpc": "2.0",
  "method": "shutdown",
  "params": {},
  "id": 1
}'
```

**Response:**
```json
{
  "jsonrpc": "2.0",
  "result": {
    "message": "Shutting down..."
  },
  "id": 1
}
```
