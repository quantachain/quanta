# Contributing

We welcome contributions from the community.

## Ways to Contribute

- Code improvements and bug fixes
- Documentation enhancements
- Test coverage expansion
- Performance optimization
- Translation and localization
- Community support and education

## Development Workflow

### 1. Fork and Clone

```bash
git clone https://github.com/YOUR_USERNAME/quanta.git
cd quanta
```

### 2. Create Feature Branch

```bash
git checkout -b feature/your-feature-name
```

### 3. Make Changes

Follow Rust best practices:

```bash
# Format code
cargo fmt

# Run linter
cargo clippy

# Run tests
cargo test
```

### 4. Commit Changes

Use conventional commit messages:

```bash
git commit -m "feat: add new feature"
git commit -m "fix: resolve bug"
git commit -m "docs: update documentation"
```

### 5. Push and Create Pull Request

```bash
git push origin feature/your-feature-name
```

Then create a pull request on GitHub.

## Code Standards

- Follow Rust style guidelines
- Write comprehensive tests
- Document public APIs
- Keep commits atomic and focused
- Write clear commit messages

## Testing

Run the full test suite:

```bash
cargo test
```

Run specific tests:

```bash
cargo test test_name
```

## Documentation

Update documentation for:
- New features
- API changes
- Configuration options
- Breaking changes

## Pull Request Process

1. Ensure all tests pass
2. Update documentation
3. Add entry to CHANGELOG.md
4. Request review from maintainers
5. Address review feedback
6. Merge after approval

## Code Review

All submissions require review. We use GitHub pull requests for this purpose.

## Community

- GitHub Discussions: Ask questions and discuss ideas
- Discord: Real-time chat (coming Q2 2026)
- GitHub Issues: Report bugs and request features

## License

By contributing, you agree that your contributions will be licensed under the MIT License.
