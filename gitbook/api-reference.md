# API Reference

The Quanta node exposes a REST API on port **3000** (default). All endpoints return JSON.

The public testnet RPC endpoint is: `https://rpc.quantachain.org`

---

## Node Health

### `GET /health`

Returns the current node health status.

```bash
curl https://rpc.quantachain.org/health
```

**Response:**
```json
{
  "status": "healthy",
  "blockchain_height": 12345,
  "peer_count": 8,
  "uptime_seconds": 86400
}
```

---

## Blocks

### `GET /blocks/latest`

Returns the most recently confirmed block.

```bash
curl https://rpc.quantachain.org/blocks/latest
```

**Response:**
```json
{
  "index": 12345,
  "timestamp": 1775088600,
  "hash": "00000012d3a2...",
  "previous_hash": "00000009f1c3...",
  "merkle_root": "abc123...",
  "state_root": "def456...",
  "difficulty": 8304130,
  "nonce": 983421,
  "transactions": [...]
}
```

---

### `GET /blocks/:height`

Returns a block by its height (block number).

```bash
curl https://rpc.quantachain.org/blocks/100
```

---

## Transactions

### `GET /transactions/:hash`

Returns a transaction by its hash.

```bash
curl https://rpc.quantachain.org/transactions/TX_HASH_HERE
```

**Response:**
```json
{
  "sender": "0x1683be267318d2ddd8cee8df4a4548dcffb1e088",
  "recipient": "0xd528c18ce7a8844e4a4dcd841975b20ae599b020",
  "amount": 5000000,
  "fee": 100,
  "nonce": 1,
  "timestamp": 1775088000,
  "tx_type": "Transfer",
  "sig_scheme": 0,
  "hash": "..."
}
```

---

### `POST /transactions`

Submit a signed transaction to the network.

```bash
curl -X POST https://rpc.quantachain.org/transactions \
  -H "Content-Type: application/json" \
  -d '{
    "sender": "0xYOUR_ADDRESS",
    "recipient": "0xRECIPIENT",
    "amount": 5000000,
    "fee": 100,
    "nonce": 1,
    "timestamp": 1775088000,
    "lock_time": 0,
    "public_key": "BASE64_ENCODED_PUBKEY",
    "signature": "BASE64_ENCODED_SIGNATURE",
    "sig_scheme": 0,
    "tx_type": "Transfer",
    "network_id": 0
  }'
```

**Success Response:**
```json
{"tx_hash": "abc123..."}
```

**Error Response:**
```json
{"error": "insufficient balance"}
```

> Use the `quanta-sdk` to build and sign transactions — it handles key encoding, nonce fetching, and the Falcon-512 signing contract automatically.

---

## Accounts

### `GET /accounts/:address/balance`

Returns the spendable balance of an address in microunits (1 QUA = 1,000,000 microunits).

```bash
curl https://rpc.quantachain.org/accounts/0x1683be267318d2ddd8cee8df4a4548dcffb1e088/balance
```

**Response:**
```json
{"balance": 47500000}
```

This is 47.5 QUA.

---

### `GET /accounts/:address/nonce`

Returns the current nonce for an address. Use `nonce + 1` as the nonce for your next transaction.

```bash
curl https://rpc.quantachain.org/accounts/0x1683be267318d2ddd8cee8df4a4548dcffb1e088/nonce
```

**Response:**
```json
{"nonce": 5}
```

Your next transaction should use `"nonce": 6`.

---

## Mempool

### `GET /mempool`

Returns all pending (unconfirmed) transactions in the mempool.

```bash
curl https://rpc.quantachain.org/mempool
```

**Response:**
```json
{
  "count": 12,
  "transactions": [...]
}
```

---

## Network

### `GET /peers`

Returns the list of currently connected peer nodes.

```bash
curl https://rpc.quantachain.org/peers
```

---

### `GET /network/stats`

Returns network-wide statistics.

```bash
curl https://rpc.quantachain.org/network/stats
```

**Response:**
```json
{
  "height": 12345,
  "total_transactions": 98201,
  "peer_count": 8,
  "hashrate_estimate": "1.2 MH/s",
  "mempool_size": 12
}
```

---

## Transaction Types

QUANTA supports three transaction types. Set `"tx_type"` in your POST body accordingly:

### Transfer

Standard value transfer between two addresses.

```json
{
  "tx_type": "Transfer"
}
```

### TimeLockTransfer

Locks funds on the recipient's account until a specific block height. Used for escrow and vesting.

```json
{
  "tx_type": {
    "TimeLockTransfer": {
      "unlock_height": 15000
    }
  }
}
```

### MultiSigTransfer

Requires M-of-N independent Falcon-512 signatures to spend.

```json
{
  "tx_type": {
    "MultiSigTransfer": {
      "signers_required": 3
    }
  }
}
```

---

## Amounts and Units

All amounts in the API are in **microunits**:

| QUA | Microunits |
|-----|-----------|
| 1 QUA | 1,000,000 |
| 0.1 QUA | 100,000 |
| 0.001 QUA | 1,000 |
| Min fee | 100 (0.0001 QUA) |

---

## Address Format

Quanta addresses are derived as:

```
address = "0x" + hex(SHA3-256(public_key)[0:20])
```

Multisig addresses use prefix `ms`:

```
multisig_address = "ms" + hex(SHA3-256(sorted_pubkeys))
```

---

## Error Codes

| HTTP Status | Meaning |
|-------------|---------|
| 200 | Success |
| 400 | Invalid request (malformed JSON, missing fields) |
| 404 | Block, transaction, or address not found |
| 422 | Transaction validation failed (invalid signature, wrong nonce, insufficient balance) |
| 500 | Node internal error |

---

## Rate Limits

The public RPC endpoint has a soft rate limit. For high-volume integrations, run your own node. See the [Node Operator Guide](node-operator-guide.md) for deployment instructions.
