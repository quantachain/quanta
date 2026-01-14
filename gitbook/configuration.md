# Configuration

Configure your Quanta node using a `quanta.toml` file.

## Basic Configuration

Create a `quanta.toml` file in your working directory:

```toml
version = 1
network_type = "Mainnet"

[node]
api_port = 3000
network_port = 8333
rpc_port = 7782
db_path = "./quanta_data"
no_network = false

[network]
max_peers = 125
bootstrap_nodes = []

[consensus]
max_block_transactions = 2000
max_block_size_bytes = 1048576
min_transaction_fee_microunits = 100

[security]
max_mempool_size = 5000
transaction_expiry_seconds = 86400
enable_rate_limiting = true

[mining]
year_1_reward_microunits = 100000000
annual_reduction_percent = 15
min_reward_microunits = 5000000

[metrics]
enabled = true
port = 9090
```

## Network Configuration

### Bootstrap Nodes

For testnet, add bootstrap nodes:

```toml
[network]
bootstrap_nodes = [
    "testnet-us-east.quanta.network:8333",
    "testnet-us-west.quanta.network:8333",
    "testnet-eu-west.quanta.network:8333",
]
```

### DNS Seeds

Configure DNS seeds for peer discovery:

```toml
[network]
dns_seeds = ["seed.testnet.quantachain.org"]
```

## Port Configuration

Customize ports for your environment:

```toml
[node]
api_port = 3000        # REST API
network_port = 8333    # P2P networking
rpc_port = 7782        # JSON-RPC daemon control
```

## Database Path

Specify where blockchain data is stored:

```toml
[node]
db_path = "./quanta_data"
```

## Single Node Mode

Disable P2P networking for local testing:

```toml
[node]
no_network = true
```

## Network Types

Set the network type:

- `"Mainnet"` - Production network
- `"Testnet"` - Test network

```toml
network_type = "Testnet"
```
