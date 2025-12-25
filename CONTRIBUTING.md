# Contributing to QUANTA

Thank you for your interest in contributing to QUANTA, a quantum-resistant blockchain implementation! We welcome contributions from the community.

## 🎯 How to Contribute

### Types of Contributions

- 🐛 **Bug Reports** - Found an issue? Let us know!
- ✨ **Feature Requests** - Have ideas for improvements?
- 🔧 **Code Contributions** - Pull requests welcome!
- 📚 **Documentation** - Help improve our docs
- 🧪 **Testing** - Write tests, find edge cases
- 🔒 **Security** - Report vulnerabilities responsibly

## 🚀 Getting Started

### 1. Fork & Clone

```bash
# Fork on GitHub, then clone your fork
git clone https://github.com/YOUR_USERNAME/quanta.git
cd quanta
```

### 2. Set Up Development Environment

```bash
# Install Rust (if not installed)
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Build the project
cargo build

# Run tests
cargo test

# Run with debug logging
RUST_LOG=debug cargo run -- demo
```

### 3. Create a Branch

```bash
git checkout -b feature/your-feature-name
# or
git checkout -b fix/bug-description
```

## 📋 Development Guidelines

### Code Style

- **Rust Style**: Follow official Rust style guidelines
- **Formatting**: Use `cargo fmt` before committing
- **Linting**: Run `cargo clippy` and fix warnings
- **Comments**: Document complex cryptographic operations

```bash
# Format code
cargo fmt

# Check for issues
cargo clippy -- -D warnings
```

### Commit Messages

Follow conventional commits format:

```
<type>(<scope>): <description>

[optional body]

[optional footer]
```

**Types:**
- `feat`: New feature
- `fix`: Bug fix
- `docs`: Documentation changes
- `test`: Adding tests
- `refactor`: Code refactoring
- `perf`: Performance improvements
- `security`: Security fixes
- `chore`: Maintenance tasks

**Examples:**
```bash
git commit -m "feat(wallet): add Kyber-1024 quantum-safe encryption"
git commit -m "fix(mining): correct difficulty adjustment algorithm"
git commit -m "docs(readme): update API endpoint documentation"
git commit -m "security(crypto): patch Falcon signature verification"
```

### Testing Requirements

All contributions must include tests:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_your_feature() {
        // Test implementation
        assert!(true);
    }
}
```

Run tests:
```bash
# Run all tests
cargo test

# Run specific test
cargo test test_quantum_wallet

# Run with output
cargo test -- --nocapture
```

## 🔒 Security Contributions

### Reporting Vulnerabilities

**DO NOT** open public issues for security vulnerabilities!

Instead:
1. Email: security@quanta-blockchain.org (if available)
2. Or create a draft security advisory on GitHub
3. Include:
   - Description of vulnerability
   - Steps to reproduce
   - Potential impact
   - Suggested fix (if any)

### Security Review Checklist

When contributing crypto code:
- [ ] Use established libraries (pqcrypto, sha3)
- [ ] Never implement custom crypto primitives
- [ ] Verify constant-time operations where needed
- [ ] Check for side-channel vulnerabilities
- [ ] Add comprehensive tests
- [ ] Document security assumptions

## 🎨 Areas for Contribution

### High Priority

1. **P2P Networking Layer**
   - Node discovery
   - Block propagation
   - Transaction broadcasting
   - Gossip protocol

2. **Advanced Mining**
   - Multi-threaded mining
   - Mining pool support
   - Stratum protocol
   - GPU acceleration research

3. **Web Interface**
   - Block explorer
   - Wallet UI
   - Node dashboard
   - Transaction viewer

4. **Additional PQC Algorithms**
   - Dilithium signatures (alternative to Falcon)
   - SPHINCS+ (hash-based signatures)
   - Comparison benchmarks

### Medium Priority

5. **Smart Contracts**
   - Virtual machine design
   - Quantum-safe contract language
   - Gas mechanism

6. **Performance Optimization**
   - Parallel signature verification
   - Database indexing
   - UTXO set optimization
   - Memory profiling

7. **Developer Tools**
   - SDK/Libraries
   - Block explorer API
   - Testnet faucet
   - Documentation examples

### Documentation

8. **Educational Content**
   - Post-quantum cryptography explainers
   - Blockchain architecture guides
   - Tutorial videos
   - Interactive demos

## 📝 Pull Request Process

### Before Submitting

- [ ] Code compiles without warnings
- [ ] All tests pass (`cargo test`)
- [ ] Code is formatted (`cargo fmt`)
- [ ] Clippy checks pass (`cargo clippy`)
- [ ] Documentation is updated
- [ ] CHANGELOG.md is updated (if applicable)

### PR Template

```markdown
## Description
Brief description of changes

## Type of Change
- [ ] Bug fix
- [ ] New feature
- [ ] Breaking change
- [ ] Documentation update
- [ ] Security fix

## Testing
How was this tested?

## Checklist
- [ ] Code follows style guidelines
- [ ] Self-review completed
- [ ] Comments added for complex code
- [ ] Documentation updated
- [ ] Tests added/updated
- [ ] No new warnings
```

### Review Process

1. Submit PR with clear description
2. Wait for automated CI checks
3. Address reviewer feedback
4. Maintainer approval required
5. Squash and merge

## 🏗️ Architecture Guidelines

### Module Organization

```
src/
├── crypto.rs          # Falcon signatures, SHA3
├── quantum_wallet.rs  # Kyber encryption, wallet management
├── transaction.rs     # UTXO model, tx validation
├── block.rs          # Block structure, mining
├── blockchain.rs     # Core chain logic
├── storage.rs        # Sled database persistence
├── api.rs            # REST API (Axum)
└── main.rs           # CLI interface
```

### Adding New Features

1. **Design Document**: For major features, write a design doc first
2. **Interface First**: Define public APIs before implementation
3. **Tests First**: Consider TDD approach
4. **Incremental**: Break into smaller PRs when possible

## 🧪 Testing Standards

### Coverage Requirements

- Minimum 70% code coverage
- 100% coverage for cryptographic functions
- Integration tests for all CLI commands
- API endpoint tests

### Test Categories

```bash
# Unit tests
cargo test --lib

# Integration tests
cargo test --test '*'

# Specific module
cargo test crypto::

# Ignored/slow tests
cargo test -- --ignored
```

## 📚 Documentation Standards

### Code Documentation

```rust
/// Mines a new block using Proof-of-Work
/// 
/// # Arguments
/// * `difficulty` - Number of leading zeros required
/// 
/// # Returns
/// * `Block` - The mined block with valid nonce
/// 
/// # Quantum Resistance
/// Uses SHA3-256 which provides ~4x slowdown for quantum computers
/// (Grover's algorithm), but exponential security vs classical attacks.
pub fn mine(&mut self) {
    // Implementation
}
```

### README Updates

When adding features, update:
- Feature list
- Usage examples
- API documentation
- Performance notes

## 🤝 Community

### Communication Channels

- **GitHub Issues**: Bug reports, feature requests
- **GitHub Discussions**: General questions, ideas
- **Pull Requests**: Code contributions

### Code of Conduct

- Be respectful and inclusive
- Focus on constructive feedback
- Help newcomers
- No harassment or discrimination
- Follow GitHub's Community Guidelines

## 🎓 Learning Resources

### Cryptography

- [NIST PQC Project](https://csrc.nist.gov/projects/post-quantum-cryptography)
- [Falcon Specification](https://falcon-sign.info/)
- [Kyber Documentation](https://pq-crystals.org/kyber/)

### Rust

- [The Rust Book](https://doc.rust-lang.org/book/)
- [Rust by Example](https://doc.rust-lang.org/rust-by-example/)
- [Async Book](https://rust-lang.github.io/async-book/)

### Blockchain

- [Bitcoin Whitepaper](https://bitcoin.org/bitcoin.pdf)
- [Mastering Bitcoin](https://github.com/bitcoinbook/bitcoinbook)
- [Ethereum Yellow Paper](https://ethereum.github.io/yellowpaper/paper.pdf)

## 📄 License

By contributing, you agree that your contributions will be licensed under the MIT License.

## 🙏 Recognition

Contributors will be:
- Listed in CONTRIBUTORS.md
- Mentioned in release notes
- Given credit in documentation

## ❓ Questions?

- Open a GitHub Discussion
- Check existing issues
- Read the documentation

---

**Thank you for making QUANTA better! 🚀**

*Together, we're building a quantum-safe future for blockchain technology.*
