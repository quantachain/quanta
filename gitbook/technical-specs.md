# Technical Specifications

## Cryptography

### Post-Quantum Algorithms

- **Signatures**: Falcon-512 (NIST Level 1)
  - Public key: 897 bytes
  - Private key: 1,281 bytes
  - Signature: ~666 bytes
  
- **Encryption**: Kyber-1024 (NIST Level 5)
  - Public key: 1,568 bytes
  - Private key: 3,168 bytes
  
- **Hashing**: SHA3-256
  - 256-bit collision resistance
  - Quantum-resistant
  
- **Key Derivation**: Argon2id
  - Memory-hard password hashing
  - Resistant to GPU/ASIC attacks

## Consensus

### Proof-of-Work

- Algorithm: SHA3-256 double hash
- Block time: 10 seconds
- Difficulty adjustment: Every 10 blocks
- Maximum increase: 2x per adjustment
- Maximum decrease: 0.5x per adjustment

### Block Structure

- Maximum transactions: 2,000 per block
- Maximum block size: 1 MB
- Merkle tree for transaction verification
- Timestamp validation: Within 2 hours of current time

## Network

### P2P Protocol

- Network magic (Testnet): `QUAX` (0x51554158)
- Network magic (Mainnet): `QUAM` (0x5155414D)
- Maximum message size: 2 MB
- Maximum peers: 125
- Ping interval: 60 seconds
- Peer timeout: 180 seconds

### Ports

- API: 3000
- P2P: 8333
- RPC: 7782
- Metrics: 9090

## Transaction Model

### Account-Based

- Nonce-based replay protection
- 24-hour transaction expiry
- Minimum fee: 100 microunits (0.0001 QUA)
- Amount precision: Microunits (1 QUA = 1,000,000 microunits)

### Fee Distribution

- 70% burned (deflationary)
- 20% to treasury
- 10% to miner

## Storage

### Database

- Engine: Sled (embedded key-value store)
- Transactional: ACID guarantees
- Atomic operations for state changes

### Storage Requirements

- Year 1: ~4.6 TB (full node)
- Year 5: ~23 TB (full node)
- Pruned node: ~500 GB (maintains 6 months)

## Technology Stack

- Language: Rust 2021
- Async runtime: Tokio
- API framework: Axum
- Serialization: Bincode
- Cryptography: pqcrypto-falcon, pqcrypto-kyber, sha3, argon2

## Security Features

- Quantum-resistant cryptography
- Memory-hard key derivation
- DoS protection (message size limits)
- Replay protection (nonces + expiry)
- 51% attack mitigation (checkpoints)
- Rate limiting
- Mempool size limits (5,000 transactions)

## Performance

- Signature generation: ~0.8 ms
- Signature verification: ~0.1 ms
- Block validation (2,000 tx): ~200 ms
- Target block propagation: <5 seconds
