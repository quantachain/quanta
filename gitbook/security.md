# Security

## Security Model

### Threat Assumptions

- Adversary has bounded computational power
- Adversary does not control >50% of mining power
- Adversary may control network nodes but not all
- Quantum computers with 10^6+ qubits may exist in future

### Not Protected Against

- 51% attacks (inherent to Proof-of-Work)
- Eclipse attacks on network-isolated nodes
- Physical key extraction from compromised devices

## Attack Resistance

### Double-Spend Attack

**Mitigation**:
- Confirmation depth: 6 blocks recommended for high-value transactions
- Probabilistic finality: 99.9% certainty after 6 blocks with 40% attacker hashpower

### Transaction Replay Attack

**Mitigation**:
- Monotonic nonce requirement per account
- 24-hour transaction expiry
- Unique transaction hash per signature

### Timestamp Manipulation

**Mitigation**:
- Blocks must be within 2 hours of current time
- Network time averaging across peers
- Rejection of blocks with timestamps before previous block

### Memory Exhaustion (DoS)

**Mitigation**:
- Orphan block limit: 100 blocks maximum
- Mempool size cap: 5,000 transactions
- Maximum message size: 2 MB
- Per-peer memory limits

### Sybil Attack

**Mitigation**:
- Proof-of-work requirement for block production
- Connection limits per IP range
- Peer reputation system (planned)

## Cryptographic Security

### Classical Security

- SHA3-256: 2^256 operations for collision
- Falcon-512: 2^128 operations for forgery
- Argon2id: Memory-hard, prevents brute-force

### Quantum Security

- Falcon-512: Lattice-based, quantum-resistant
- Kyber-1024: 256-bit quantum security
- SHA3-256: Grover-resistant (128-bit effective security)

## Network Security

### DoS Protection

- 2 MB message size limit
- 5,000 transaction mempool cap
- Rate limiting enabled by default
- Invalid message handling: automatic peer disconnection

### Replay Protection

- Monotonic nonces per account
- 24-hour transaction expiry
- Unique transaction signatures

### 51% Attack Mitigation

- Checkpoint system prevents deep reorganizations
- High block rewards attract honest miners
- Social layer: exchanges require many confirmations

## Operational Security

### Node Security

- Graceful shutdown handling (SIGINT/SIGTERM)
- Persistent state across restarts
- Health check endpoints
- Localhost-only RPC binding by default

### Wallet Security

- Encrypted wallet storage with Kyber-1024
- Memory-hard key derivation (Argon2id)
- Secure key generation
- Mnemonic phrase backup (BIP39)

## Best Practices

### For Node Operators

- Keep software updated
- Use firewall rules to restrict access
- Monitor logs for suspicious activity
- Backup blockchain data regularly
- Use strong passwords for RPC access
- Bind RPC to localhost only

### For Wallet Users

- Backup wallet files and mnemonic phrases
- Store backups in encrypted storage
- Never share private keys or mnemonics
- Use strong passwords for wallet encryption
- Verify addresses before sending transactions
- Use hardware wallets when available

### For Miners

- Secure mining wallet separately
- Use dedicated mining address
- Monitor for unusual activity
- Keep mining software updated
- Implement proper cooling and power backup

## Vulnerability Reporting

### Responsible Disclosure

If you discover a security vulnerability:

1. **Do NOT** open a public GitHub issue
2. Email security details to: security@quantachain.org
3. Include:
   - Description of vulnerability
   - Steps to reproduce
   - Potential impact
   - Suggested fix (if any)

### Bug Bounty Program

Planned for Q2 2026 testnet launch:
- Critical vulnerabilities: $10,000+
- High severity: $5,000+
- Medium severity: $1,000+
- Low severity: $500+

## Security Audits

### Planned Audits

- Q2 2026: External security audit (testnet)
- Q3 2026: Comprehensive security audit
- Q4 2026: Final pre-mainnet audit

### Audit Scope

- Cryptographic implementation
- Consensus mechanism
- Network protocol
- Wallet security
- Smart contract layer (future)

## Incident Response

### Emergency Procedures

1. Identify and assess the threat
2. Notify core development team
3. Prepare emergency patch if needed
4. Coordinate with node operators
5. Deploy fix via emergency hard fork if critical
6. Post-mortem analysis and disclosure

### Communication Channels

- GitHub Security Advisories
- Discord announcements
- Email notifications to registered operators
- Website security bulletin
