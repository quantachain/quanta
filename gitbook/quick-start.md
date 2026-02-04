# Quick Start

Get started with Quanta in minutes.

## Start a Node
 
### Run with Docker (Recommended)
 
The fastest way to get started is with Docker:
 
```bash
docker run -d --name quanta-node -p 3000:3000 -p 8333:8333 -p 7782:7782 -p 9090:9090 xd637/quanta-node:v0.1
```
 
### Build from Source
 
Alternatively, build and start the node in daemon mode:

```bash
cargo build --release
./target/release/quanta start --detach
```

## Check Node Status

Verify your node is running:

```bash
./target/release/quanta status
```

## View Blockchain Height

Check the current blockchain height:

```bash
./target/release/quanta print_height
```

## Check Connected Peers

View your connected peers:

```bash
./target/release/quanta peers
```

## Stop the Node

To stop a running daemon:

```bash
./target/release/quanta stop
```

## Configuration

Create a `quanta.toml` file in your working directory to customize node settings. See the Configuration section for details.
