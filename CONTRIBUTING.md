# Contributing

Contributions welcome!

## Setup

```bash
git clone https://github.com/YOUR_USERNAME/quanta.git
cd quanta
cargo build
cargo test
```

## Development

```bash
cargo fmt          # Format code
cargo clippy       # Linting
cargo test         # Run tests
```

## Commit Format

```
<type>: <description>

Types: feat, fix, docs, test, refactor, security, chore
```

Examples:
```
feat: add continuous mining API
fix: correct difficulty adjustment
docs: update API documentation
security: patch signature verification
```

## Pull Requests

Before submitting:
- Code compiles without warnings
- All tests pass
- Code is formatted
- Clippy checks pass
- Documentation updated

## Areas to Contribute

- P2P networking improvements
- Mining optimizations
- Web interface enhancements
- Additional PQC algorithms
- Performance optimization
- Documentation
- Tests

## Security

Do not open public issues for vulnerabilities. Create draft security advisory on GitHub.

## License

MIT License - contributions will be licensed under same terms.
