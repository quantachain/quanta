# API Reference

Quanta provides both REST API and JSON-RPC interfaces.

## REST API

The REST API runs on port 3000 by default.

### Health Check

```bash
curl http://localhost:3000/health
```

Response:
```json
{
  "status": "healthy",
  "blockchain_height": 12345,
  "peer_count": 8,
  "uptime_seconds": 86400
}
```

### Get Blockchain Statistics

```bash
curl http://localhost:3000/api/stats
```

### Check Address Balance

```bash
curl -X POST http://localhost:3000/api/balance \
  -H "Content-Type: application/json" \
  -d '{"address": "your_address_here"}'
```

## JSON-RPC API

The JSON-RPC daemon control interface runs on port 7782 by default.

### Get Node Status

```bash
curl -X POST http://localhost:7782 \
  -H "Content-Type: application/json" \
  -d '{"jsonrpc":"2.0","method":"node_status","params":[],"id":1}'
```

### Start Mining

```bash
curl -X POST http://localhost:7782 \
  -H "Content-Type: application/json" \
  -d '{"jsonrpc":"2.0","method":"start_mining","params":["YOUR_ADDRESS"],"id":1}'
```

### Stop Mining

```bash
curl -X POST http://localhost:7782 \
  -H "Content-Type: application/json" \
  -d '{"jsonrpc":"2.0","method":"stop_mining","params":[],"id":1}'
```

### Get Mining Status

```bash
curl -X POST http://localhost:7782 \
  -H "Content-Type: application/json" \
  -d '{"jsonrpc":"2.0","method":"mining_status","params":[],"id":1}'
```

### Get Block Information

```bash
curl -X POST http://localhost:7782 \
  -H "Content-Type: application/json" \
  -d '{"jsonrpc":"2.0","method":"get_block","params":[HEIGHT],"id":1}'
```

## Port Configuration

Default ports:
- REST API: 3000
- P2P Network: 8333
- JSON-RPC: 7782
- Prometheus Metrics: 9090

Configure custom ports in `quanta.toml`.
