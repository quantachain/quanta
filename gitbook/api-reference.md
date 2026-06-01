# API Reference

Quanta V2 exposes a standard REST API on port `3000` for frontend and agent interaction, and an internal RPC server on port `7782` for CLI tooling.

## REST API (Port 3000)

| Endpoint | Method | Description |
|----------|--------|-------------|
| `/health` | `GET` | Node health and version |
| `/blocks/latest` | `GET` | Latest committed AlephBFT block |
| `/blocks/:height` | `GET` | Retrieve block by height |
| `/accounts/:address/balance` | `GET` | Get QUA balance for address |
| `/accounts/:address/nonce` | `GET` | Get current transaction nonce |
| `/mempool` | `GET` | View pending transactions |
| `/network/stats` | `GET` | General network hashrate (deprecated) / validator stats |
| `/transactions` | `POST` | Submit a signed transaction |

## RPC Server (Port 7782)
This port uses a custom JSON-RPC protocol over TCP. It is strictly used by the `quanta` and `quanta-wallet` CLI tools. It is heavily recommended to firewall this port from public access.
