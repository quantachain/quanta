# API Reference

Quanta V2 exposes a standard REST API on port `3000` for frontend and AI agent interaction, and an internal JSON-RPC server on port `7782` for CLI tooling.

## REST API (Port 3000)

| Endpoint | Method | Description |
|----------|--------|-------------|
| `/health` | `GET` | Node health and version |
| `/api/stats` | `GET` | General blockchain stats (height, epoch) |
| `/api/validate` | `GET` | Validates local chain |
| `/api/peers` | `GET` | Connected network peers |
| `/api/validators` | `GET` | List of active BFT validators |
| `/api/metrics` | `GET` | Node metrics (mempool size, active connections) |
| `/api/blocks/latest` | `GET` | Latest committed AlephBFT block |
| `/api/block/:height` | `GET` | Retrieve block by height |
| `/api/mempool` | `GET` | View pending transactions |
| `/api/transactions/submit` | `POST` | Submit a signed transaction |
| `/api/tx/:hash` | `GET` | Retrieve transaction by hash |
| `/api/balance/:address` | `GET` | Get QUA balance for an address |
| `/api/address/:address` | `GET` | Get full address info (balance, nonce) |
| `/api/address/:address/txs` | `GET` | Get transaction history for address |
| `/api/contracts/:address` | `GET` | Retrieve contract state (e.g. Escrow) |
| `/api/contracts/:address/events` | `GET` | Retrieve contract events |
| `/api/agents` | `GET` | List active AI agents on the network |

*(Note: The legacy Proof-of-Work mining endpoints have been completely removed in V2. Block production is handled internally by the BFT proposer.)*

## RPC Server (Port 7782)

This port uses a custom JSON-RPC protocol over TCP. It is strictly used by the `quanta` and `quanta-wallet` CLI tools. **It is heavily recommended to firewall this port from public access.**

Available Methods:
- `node_status`
- `get_block`
- `get_balance`
- `get_peers`
- `get_mempool`
- `shutdown`
