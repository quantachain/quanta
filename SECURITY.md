# Security Policy

## 🔒 Security Overview

QUANTA implements post-quantum cryptographic algorithms for blockchain security. However, this is an **educational/research implementation** and has NOT undergone formal security audits.

## ⚠️ Known Limitations

### NOT Production-Ready For:
- ❌ Real financial transactions with monetary value
- ❌ Mission-critical applications
- ❌ High-value asset storage
- ❌ Regulatory-compliant systems

### Suitable For:
- ✅ Educational purposes
- ✅ Research projects
- ✅ Testnet deployments
- ✅ Algorithm demonstrations
- ✅ Private/consortium blockchains (with proper review)

## 🛡️ Cryptographic Implementation

### Quantum-Resistant Algorithms Used

| Component | Algorithm | NIST Status | Security Level |
|-----------|-----------|-------------|----------------|
| **Signatures** | Falcon-512 | PQC Round 3 | Level 1 (~128-bit) |
| **Hashing** | SHA3-256 | FIPS 202 | 128-bit quantum |
| **KEM** | Kyber-1024 | PQC Selected | Level 5 (~256-bit) |
| **Cipher** | ChaCha20-Poly1305 | RFC 8439 | 256-bit classical |

### Security Assumptions

1. **Falcon-512 Signatures**
   - Based on NTRU lattices
   - Secure against Shor's algorithm (quantum)
   - Resistant to classical lattice attacks
   - Implementation uses `pqcrypto-falcon` crate

2. **SHA3-256 Hashing**
   - Only ~4x speedup from Grover's algorithm
   - Effectively 128-bit security vs quantum
   - Used for block hashing and addresses

3. **Kyber-1024 Encryption**
   - Module-LWE based
   - Highest NIST security level
   - Used for wallet encryption
   - Paired with ChaCha20-Poly1305

## 🐛 Reporting Security Vulnerabilities

### How to Report

**DO NOT** create public GitHub issues for security vulnerabilities!

**Instead:**

1. **Email**: Open a draft security advisory on GitHub
2. **Include**:
   - Clear description of vulnerability
   - Steps to reproduce
   - Potential impact assessment
   - Affected versions
   - Proof-of-concept (if applicable)
   - Suggested remediation (optional)

### What Happens Next

1. **Acknowledgment**: Within 48 hours
2. **Assessment**: 1-7 days for severity analysis
3. **Fix**: Develop and test patch
4. **Disclosure**: Coordinated disclosure after fix
5. **Credit**: Security researchers credited (if desired)

### Scope

**In Scope:**
- Cryptographic implementation flaws
- Key management vulnerabilities
- Signature verification bypass
- Blockchain consensus attacks
- Database injection attacks
- API authentication issues
- Memory safety issues

**Out of Scope:**
- DoS via resource exhaustion (known limitation)
- Social engineering
- Physical access attacks
- Issues in dependencies (report to upstream)
- Theoretical attacks without PoC

## 🚨 Security Warnings

### Critical Warnings

1. **Private Key Storage**
   ```
   ⚠️  Wallet files (.qua) contain encrypted private keys
   ⚠️  NEVER commit wallet files to version control
   ⚠️  NEVER share wallet files publicly
   ⚠️  Passwords cannot be recovered if lost
   ```

2. **Demo Passwords**
   ```
   ⚠️  Demo code contains INSECURE passwords
   ⚠️  Demo wallets are PUBLIC and INSECURE
   ⚠️  Delete demo wallets after testing
   ⚠️  Always use strong passwords in production
   ```

3. **Network Exposure**
   ```
   ⚠️  API server has NO authentication
   ⚠️  Do not expose to untrusted networks
   ⚠️  Use firewall rules to restrict access
   ⚠️  Consider VPN or SSH tunneling
   ```

### Best Practices

#### Wallet Security
- ✅ Use strong, unique passwords (20+ characters)
- ✅ Store wallet backups securely offline
- ✅ Never share wallet files or passwords
- ✅ Use hardware security modules for high-value keys
- ✅ Test recovery procedures regularly

#### Node Security
- ✅ Keep software updated
- ✅ Restrict API access with firewall rules
- ✅ Monitor logs for suspicious activity
- ✅ Use TLS/SSL for remote connections
- ✅ Implement rate limiting
- ✅ Regular database backups

#### Development Security
- ✅ Never hardcode passwords or keys
- ✅ Use environment variables for secrets
- ✅ Review code for injection vulnerabilities
- ✅ Validate all inputs
- ✅ Use constant-time comparisons for secrets
- ✅ Clear sensitive data from memory after use

## 🔐 Secure Configuration Examples

### Strong Password Generation
```bash
# Generate 32-character password
openssl rand -base64 32

# Or use password manager
```

### Secure Node Deployment
```bash
# 1. Create non-root user
sudo useradd -m -s /bin/bash quanta

# 2. Restrict file permissions
chmod 600 wallet.qua
chmod 700 quanta_data/

# 3. Use systemd with user isolation
# 4. Configure firewall
sudo ufw allow from 192.168.1.0/24 to any port 3000

# 5. Run API behind reverse proxy (nginx/caddy)
```

### Environment Variables
```bash
# .env (NEVER commit this file!)
QUANTA_WALLET_PASSWORD=your_secure_password_here
QUANTA_API_PORT=3000
QUANTA_DB_PATH=/var/lib/quanta/data
```

## 📊 Security Audit Status

| Component | Audit Status | Last Review |
|-----------|-------------|-------------|
| Cryptographic Implementation | ❌ Not Audited | N/A |
| Smart Contract VM | ❌ Not Implemented | N/A |
| API Security | ❌ Not Audited | N/A |
| Database Security | ❌ Not Audited | N/A |
| Network Protocol | ❌ Not Implemented | N/A |

**Status**: 🔴 **Pre-Audit** - Educational use only

## 🎯 Security Roadmap

### Before Production Use:

- [ ] Professional security audit by certified firm
- [ ] Penetration testing
- [ ] Formal verification of cryptographic implementations
- [ ] Memory safety audit (Rust unsafe code review)
- [ ] API authentication and authorization
- [ ] Rate limiting and DoS protection
- [ ] Comprehensive logging and monitoring
- [ ] Incident response plan
- [ ] Bug bounty program
- [ ] Regular security updates

## 📚 Security Resources

### Standards & Documentation
- [NIST Post-Quantum Cryptography](https://csrc.nist.gov/projects/post-quantum-cryptography)
- [Falcon Specification](https://falcon-sign.info/)
- [Kyber Specification](https://pq-crystals.org/kyber/)
- [OWASP Blockchain Security](https://owasp.org/www-project-blockchain/)

### Tools
- [Cargo Audit](https://github.com/RustSec/rustsec) - Dependency vulnerability scanner
- [Clippy](https://github.com/rust-lang/rust-clippy) - Rust linter
- [Miri](https://github.com/rust-lang/miri) - Undefined behavior detector

### Run Security Checks
```bash
# Check for vulnerable dependencies
cargo audit

# Security-focused linting
cargo clippy -- -D warnings

# Check for unsafe code
rg "unsafe" src/
```

## 📝 Version History

- **v1.0.0** - Initial release (Not audited)

## 🙏 Security Researchers

We appreciate responsible disclosure. Security researchers who report valid vulnerabilities will be:
- Credited in release notes (if desired)
- Listed in security acknowledgments
- Provided with CVSS scoring details

---

**Remember: Security is a process, not a product.**

Last Updated: 2025-12-25
