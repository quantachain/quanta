# Quantum Resistance

## The Quantum Threat

Current blockchains rely on elliptic curve cryptography (ECDSA, EdDSA) that quantum computers can break using Shor's algorithm. Conservative estimates suggest quantum computers capable of breaking 256-bit ECDSA could exist within 10-15 years.

## Quanta's Solution

Quanta uses NIST-standardized post-quantum cryptography that resists both classical and quantum attacks.

## Post-Quantum Algorithms

### Falcon-512 Signatures

- **Type**: Lattice-based signature scheme (NTRU lattices)
- **Security Level**: NIST Level 1 (equivalent to AES-128)
- **Key Sizes**:
  - Public key: 897 bytes
  - Private key: 1,281 bytes
  - Signature: ~666 bytes
- **Performance**: Fast verification, compact signatures
- **Quantum Security**: No known quantum attacks on lattice problems

### Kyber-1024 Encryption

- **Type**: Module-LWE-based key encapsulation
- **Security Level**: NIST Level 5 (equivalent to AES-256)
- **Key Sizes**:
  - Public key: 1,568 bytes
  - Private key: 3,168 bytes
- **Use Case**: Wallet encryption, secure key storage
- **Quantum Security**: Maximum security for long-term protection

### SHA3-256 Hashing

- **Type**: Keccak-based cryptographic hash
- **Security**: 256-bit collision resistance
- **Quantum Resistance**: Grover's algorithm reduces effective security to 128-bit (still secure)
- **NIST Standardized**: Yes

### Argon2id Key Derivation

- **Type**: Memory-hard password hashing
- **Configuration**: Time cost: 2, Memory: 65536 KB, Parallelism: 4
- **Quantum Resistance**: Memory hardness provides quantum resistance
- **Protection**: Resistant to GPU/ASIC attacks

## Security Analysis

### Classical Attack Resistance

- **Signature forgery**: 2^128 operations (computationally infeasible)
- **Hash collisions**: 2^256 operations
- **Brute force**: Protected by Argon2id memory hardness

### Quantum Attack Resistance

- **Grover's algorithm**: Reduces hash security by half (256-bit to 128-bit, still secure)
- **Shor's algorithm**: Not applicable to lattice-based cryptography
- **Post-quantum cryptanalysis**: No known polynomial-time attacks

## Implementation

### Transaction Signing

```
Signature = Falcon-512.Sign(private_key, SHA3-256(transaction_data))
Verification = Falcon-512.Verify(public_key, signature, SHA3-256(transaction_data))
```

### Wallet Encryption

```
Encrypted_Wallet = Kyber-1024.Encrypt(plaintext_keys, password_via_Argon2)
```

### Address Generation

```
Address = Base58Check(0x00 || SHA3-256(Falcon-512.PublicKey)[:20])
```

## Operational Impact

### Signature Size

Falcon-512 signatures (~666 bytes) are larger than ECDSA (~64 bytes):

- Block with 2,000 transactions: ~1.3 MB in signatures
- Annual signature data: ~4.2 TB
- Full archival node (year 1): ~4.6 TB total

### Mitigation Strategies

1. **Signature Aggregation**: Research into Falcon-compatible batch verification
2. **Pruning**: Remove old signatures (reduces to ~2.5 TB/year)
3. **Compression**: Specialized compression for lattice signatures
4. **SPV Protocol**: Light clients verify only relevant transactions

## Future-Proofing

### Cryptographic Agility

Quanta is designed to support algorithm upgrades via hard fork if needed:

- Hybrid signature schemes can be added
- Migration paths for new NIST standards
- Governance process for cryptographic changes

### Long-Term Security

- **2025-2035**: Classical security sufficient
- **2035-2045**: Quantum computers may emerge
- **2045+**: Quanta remains secure with post-quantum crypto

## Comparison with Other Chains

### Bitcoin/Ethereum

- **Cryptography**: ECDSA (vulnerable to quantum attacks)
- **Quantum Resistance**: None
- **Migration**: Requires hard fork and address migration

### Quanta

- **Cryptography**: Falcon-512 (quantum-resistant)
- **Quantum Resistance**: Full, from genesis
- **Migration**: Not needed

## References

- NIST Post-Quantum Cryptography Standardization (2024)
- Falcon: Fast-Fourier Lattice-based Compact Signatures over NTRU
- CRYSTALS-Kyber: Key Encapsulation Mechanism
- The Keccak SHA-3 Proposal
