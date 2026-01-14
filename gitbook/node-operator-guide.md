# Node Operator Guide

Complete guide for running and maintaining a Quanta node.

## System Requirements

### Full Node
- CPU: 4 cores at 2.0 GHz or higher
- RAM: 8 GB minimum, 16 GB recommended
- Storage: 1 TB SSD (year 1), plan for 5 TB over 5 years
- Bandwidth: 50 Mbps down, 20 Mbps up
- OS: Linux (Ubuntu 20.04+), macOS (10.15+), Windows 10+

### Pruned Node
- CPU: 2 cores
- RAM: 4 GB
- Storage: 100 GB SSD
- Bandwidth: 25 Mbps down, 10 Mbps up

## Installation

1. Install Rust 1.70+:
```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

2. Clone and build:
```bash
git clone https://github.com/quantachain/quanta.git
cd quanta
cargo build --release
```

## Running a Node

### Start Node in Daemon Mode

```bash
./target/release/quanta start --detach
```

### Check Node Status

```bash
./target/release/quanta status
```

### View Logs

Logs are written to stdout. For daemon mode, redirect to a file:

```bash
./target/release/quanta start --detach > node.log 2>&1
```

## Network Configuration

### Running Multiple Nodes

Node 1 (Bootstrap):
```bash
./target/release/quanta start --detach \
  --network-port 8333 \
  --port 3000 \
  --rpc-port 7782 \
  --db ./node1_data
```

Node 2 (Connect to Node 1):
```bash
./target/release/quanta start --detach \
  --network-port 8334 \
  --port 3001 \
  --rpc-port 7783 \
  --db ./node2_data \
  --bootstrap 127.0.0.1:8333
```

### Check Peer Connections

```bash
./target/release/quanta peers
```

## Monitoring

### Prometheus Metrics

Quanta exports metrics on port 9090. Configure Prometheus:

```yaml
scrape_configs:
  - job_name: 'quanta'
    static_configs:
      - targets: ['localhost:9090']
```

Available metrics:
- Blockchain height
- Transaction throughput
- Peer count
- Mining hashrate
- Mempool size
- Block validation time

### Health Checks

```bash
curl http://localhost:3000/health
```

## Maintenance

### Validate Blockchain

```bash
./target/release/quanta validate --db ./quanta_data
```

### Backup

Backup your blockchain data:
```bash
tar -czf quanta_backup.tar.gz quanta_data/
```

### Update Node

1. Stop the node:
```bash
./target/release/quanta stop
```

2. Pull latest changes:
```bash
git pull origin main
cargo build --release
```

3. Restart:
```bash
./target/release/quanta start --detach
```

## Security

- Keep your node software updated
- Use firewall rules to restrict access
- Monitor logs for suspicious activity
- Backup wallet files securely
- Use strong passwords for RPC access

## Troubleshooting

### Node Won't Start

Check if ports are already in use:
```bash
netstat -an | grep 8333
```

### Sync Issues

Verify bootstrap nodes are reachable:
```bash
telnet testnet-us-east.quanta.network 8333
```

### High Memory Usage

Reduce mempool size in `quanta.toml`:
```toml
[security]
max_mempool_size = 2500
```
