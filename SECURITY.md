# Security Policy

## Status

Educational/research implementation. NOT audited for production use.

## Algorithms

| Component | Algorithm | NIST Status |
|-----------|-----------|-------------|
| Signatures | Falcon-512 | PQC Round 3 |
| Hashing | SHA3-256 | FIPS 202 |
| Encryption | Kyber-1024 | PQC Selected |
| Cipher | ChaCha20-Poly1305 | RFC 8439 |

## Reporting Vulnerabilities

**CRITICAL:** Critical vulnerabilities MUST be emailed directly to **admin@quantachain.org**. 

Do NOT open public issues or share vulnerability details openly.

Please include:
- Description
- Reproduction steps
- Impact assessment
- Affected versions

## Warnings

- Wallet files contain encrypted private keys
- Never commit wallet files to version control
- Demo passwords are insecure
- API has no authentication - use firewall rules
- Keep software updated

## Best Practices

- Use strong passwords (20+ characters)
- Store wallet backups offline
- Restrict API access with firewall
- Never hardcode passwords
- Regular database backups

## Audit Status

This is an educational/research implementation. It is **NOT** audited for production use. 
The current testnet is strictly for experimental and research use only.

## Resources

- [NIST Post-Quantum Cryptography](https://csrc.nist.gov/projects/post-quantum-cryptography)
- [Falcon Specification](https://falcon-sign.info/)
- [Kyber Specification](https://pq-crystals.org/kyber/)

Last Updated: 2026-06-02
