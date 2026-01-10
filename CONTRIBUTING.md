# Contributing to QUANTA

Thank you for your interest in contributing to QUANTA! We welcome contributions from developers, researchers, designers, and community members.

This document provides guidelines and instructions for contributing to the QUANTA blockchain project.

---

## Table of Contents

1. [Code of Conduct](#code-of-conduct)
2. [Getting Started](#getting-started)
3. [Development Environment](#development-environment)
4. [Development Workflow](#development-workflow)
5. [Coding Standards](#coding-standards)
6. [Testing Requirements](#testing-requirements)
7. [Pull Request Process](#pull-request-process)
8. [Areas to Contribute](#areas-to-contribute)
9. [Community Guidelines](#community-guidelines)

---

## Code of Conduct

### Our Standards

We are committed to providing a welcoming and inclusive environment. All contributors are expected to:

- Use welcoming and inclusive language
- Respect differing viewpoints and experiences
- Accept constructive criticism gracefully
- Focus on what is best for the community
- Show empathy towards other community members

### Unacceptable Behavior

- Harassment, trolling, or discriminatory comments
- Publishing others' private information
- Inappropriate sexual attention or advances
- Other conduct that could be considered unprofessional

### Enforcement

Violations of the code of conduct should be reported to the project maintainers. All complaints will be reviewed and investigated promptly and fairly.

---

## Getting Started

### Prerequisites

Before contributing, ensure you have:

- **Rust 1.70+**: Install from [rustup.rs](https://rustup.rs/)
- **Git**: For version control
- **Code Editor**: VS Code, IntelliJ IDEA, or your preferred editor
- **GitHub Account**: For submitting pull requests

### Fork and Clone

```bash
# Fork the repository on GitHub, then clone your fork
git clone https://github.com/YOUR_USERNAME/quanta.git
cd quanta

# Add upstream remote
git remote add upstream https://github.com/quantachain/quanta.git

# Verify remotes
git remote -v
```

---

## Development Environment

### Initial Setup

```bash
# Install Rust toolchain (if not already installed)
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Install development dependencies
rustup component add rustfmt clippy

# Build the project
cargo build

# Run tests to verify setup
cargo test
```

### Recommended Tools

- **rust-analyzer**: IDE support for Rust
- **cargo-watch**: Automatically rebuild on file changes
- **cargo-audit**: Check for security vulnerabilities
- **cargo-outdated**: Check for outdated dependencies

```bash
# Install optional tools
cargo install cargo-watch cargo-audit cargo-outdated
```

### Project Structure

```
quanta/
 src/
    main.rs                 # Entry point
    lib.rs                  # Library root
    api/                    # REST API handlers
    consensus/              # Blockchain and consensus logic
    core/                   # Block and transaction structures
    crypto/                 # Cryptography (Falcon, Kyber, wallets)
    network/                # P2P networking
    rpc/                    # JSON-RPC server
    storage/                # Database layer
 tests/                      # Integration tests
 Cargo.toml                  # Dependencies
 WHITEPAPER.md              # Technical specification
 TOKENOMICS.md              # Economic model
 CONTRIBUTING.md            # This file
```

---

## Development Workflow

### 1. Sync Your Fork

```bash
# Fetch upstream changes
git fetch upstream

# Merge upstream main into your local main
git checkout main
git merge upstream/main

# Push to your fork
git push origin main
```

### 2. Create a Feature Branch

```bash
# Create and switch to a new branch
git checkout -b feature/your-feature-name

# Or for bug fixes
git checkout -b fix/bug-description
```

### 3. Make Changes

- Write clean, idiomatic Rust code
- Follow the coding standards (see below)
- Add tests for new functionality
- Update documentation as needed

### 4. Test Your Changes

```bash
# Format code
cargo fmt

# Run linter
cargo clippy -- -D warnings

# Run all tests
cargo test

# Run specific test
cargo test test_name

# Run with output
cargo test -- --nocapture
```

### 5. Commit Your Changes

Use conventional commit messages:

```bash
# Format: <type>: <description>
#
# Types:
#   feat:     New feature
#   fix:      Bug fix
#   docs:     Documentation changes
#   test:     Adding or updating tests
#   refactor: Code refactoring
#   perf:     Performance improvements
#   security: Security fix or improvement
#   chore:    Build process or auxiliary tool changes

git add .
git commit -m "feat: add continuous mining API endpoint"
```

**Commit Message Examples:**
```
feat: implement Merkle proof verification
fix: correct difficulty adjustment calculation
docs: update API documentation with new endpoints
test: add integration tests for P2P networking
refactor: simplify wallet encryption logic
security: patch signature verification vulnerability
perf: optimize block validation performance
chore: update dependencies to latest versions
```

### 6. Push to Your Fork

```bash
git push origin feature/your-feature-name
```

### 7. Create Pull Request

1. Go to your fork on GitHub
2. Click "Compare & pull request"
3. Fill out the PR template (see below)
4. Submit the pull request

---

## Coding Standards

### Rust Style Guidelines

Follow the [Rust API Guidelines](https://rust-lang.github.io/api-guidelines/) and official Rust style guide.

**Key Points:**

- Use `rustfmt` for consistent formatting
- Pass `clippy` lints without warnings
- Prefer explicit error handling over `unwrap()`
- Use meaningful variable and function names
- Add documentation comments for public APIs

### Code Formatting

```bash
# Format all code
cargo fmt

# Check formatting without modifying files
cargo fmt -- --check
```

### Linting

```bash
# Run clippy
cargo clippy -- -D warnings

# Fix clippy suggestions automatically (when safe)
cargo clippy --fix
```

### Documentation

- Add doc comments (`///`) for all public functions, structs, and modules
- Include examples in doc comments where helpful
- Keep documentation concise but complete

```rust
/// Validates a block against consensus rules.
///
/// # Arguments
///
/// * `block` - The block to validate
/// * `chain` - The current blockchain state
///
/// # Returns
///
/// Returns `Ok(())` if valid, or an error describing why validation failed.
///
/// # Example
///
/// ```
/// let is_valid = validate_block(&block, &chain)?;
/// ```
pub fn validate_block(block: &Block, chain: &Blockchain) -> Result<()> {
    // Implementation
}
```

### Error Handling

- Use `Result<T, Error>` for fallible operations
- Define custom error types using `thiserror` or `anyhow`
- Provide context for errors using `.context()` or `.with_context()`
- Avoid `unwrap()` in production code; use `?` or `expect()` with clear messages

```rust
// Good
let config = load_config()
    .context("Failed to load configuration file")?;

// Avoid
let config = load_config().unwrap();
```

### Naming Conventions

- `snake_case` for functions, variables, modules
- `CamelCase` for types, structs, enums
- `SCREAMING_SNAKE_CASE` for constants
- Clear, descriptive names (avoid abbreviations unless standard)

---

## Testing Requirements

### Test Coverage

All new features and bug fixes must include tests:

- **Unit Tests**: Test individual functions and modules
- **Integration Tests**: Test interactions between components
- **End-to-End Tests**: Test complete workflows

### Writing Tests

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_block_validation() {
        let block = create_test_block();
        let chain = create_test_blockchain();
        
        assert!(validate_block(&block, &chain).is_ok());
    }

    #[test]
    fn test_invalid_signature() {
        let tx = create_invalid_transaction();
        
        assert!(verify_signature(&tx).is_err());
    }
}
```

### Running Tests

```bash
# Run all tests
cargo test

# Run tests with output
cargo test -- --nocapture

# Run specific test
cargo test test_block_validation

# Run tests in specific module
cargo test consensus::

# Run tests with backtrace
RUST_BACKTRACE=1 cargo test
```

### Test Quality Standards

- Tests must be deterministic (no flakiness)
- Use descriptive test names that explain what is being tested
- Include both positive and negative test cases
- Mock external dependencies
- Clean up test resources (temporary files, etc.)

---

## Pull Request Process

### Before Submitting

Ensure your PR meets these requirements:

- [ ] Code compiles without errors or warnings
- [ ] All tests pass (`cargo test`)
- [ ] Code is formatted (`cargo fmt`)
- [ ] Clippy checks pass (`cargo clippy -- -D warnings`)
- [ ] Documentation is updated (if applicable)
- [ ] Commit messages follow conventional format
- [ ] PR description is clear and complete

### PR Description Template

```markdown
## Description

Brief description of the changes.

## Type of Change

- [ ] Bug fix (non-breaking change that fixes an issue)
- [ ] New feature (non-breaking change that adds functionality)
- [ ] Breaking change (fix or feature that would cause existing functionality to change)
- [ ] Documentation update

## Motivation and Context

Why is this change required? What problem does it solve?

## How Has This Been Tested?

Describe the tests you ran to verify your changes.

## Screenshots (if applicable)

Add screenshots for UI changes.

## Checklist

- [ ] My code follows the project's style guidelines
- [ ] I have performed a self-review of my own code
- [ ] I have commented my code, particularly in hard-to-understand areas
- [ ] I have made corresponding changes to the documentation
- [ ] My changes generate no new warnings
- [ ] I have added tests that prove my fix is effective or that my feature works
- [ ] New and existing unit tests pass locally with my changes
```

### Review Process

1. Maintainers will review your PR within 1-2 weeks
2. Address any requested changes
3. Once approved, maintainers will merge your PR
4. Your contribution will be included in the next release

### Continuous Integration

All PRs trigger automated CI checks:

- Build verification
- Test suite execution
- Code formatting check
- Clippy linting
- Security audit (cargo-audit)

PRs must pass all CI checks before merging.

---

## Areas to Contribute

### High Priority

1. **Testing**: Expand test coverage, especially integration tests
2. **Documentation**: Improve API docs, add usage examples
3. **Performance**: Profile and optimize critical paths
4. **Security**: Code review, fuzzing, vulnerability research

### Medium Priority

1. **P2P Networking**: Connection stability, peer discovery improvements
2. **Mining**: Algorithm optimization, GPU mining support research
3. **API**: Additional endpoints, better error messages
4. **Wallet**: UI improvements, additional features

### Long-Term Projects

1. **Light Client**: SPV protocol implementation
2. **Mobile Support**: iOS/Android wallet development
3. **Smart Contracts**: Post-quantum VM research
4. **Privacy**: Confidential transaction research

### Good First Issues

Look for issues labeled `good first issue` on GitHub. These are beginner-friendly tasks designed for new contributors.

---

## Community Guidelines

### Communication Channels

- **GitHub Issues**: Bug reports, feature requests, technical discussions
- **GitHub Discussions**: General questions, ideas, community support
- **Discord** (Coming Q2 2026): Real-time chat, community events
- **Email**: For sensitive security issues only

### Asking Questions

Before asking a question:

1. Check the documentation ([WHITEPAPER.md](WHITEPAPER.md), [README.md](README.md))
2. Search existing GitHub issues
3. Review closed issues for similar problems

When asking questions:

- Provide context and details
- Include error messages and logs
- Describe what you've already tried
- Use code formatting for code snippets

### Reporting Bugs

Use the bug report template on GitHub Issues:

- Describe the bug clearly
- Provide steps to reproduce
- Include expected vs actual behavior
- Add system information (OS, Rust version, etc.)
- Attach relevant logs or screenshots

### Requesting Features

Use the feature request template on GitHub Issues:

- Explain the use case and motivation
- Describe the proposed solution
- Consider alternative approaches
- Assess impact on existing functionality

---

## Security Vulnerabilities

**Do NOT open public GitHub issues for security vulnerabilities.**

See [SECURITY.md](SECURITY.md) for responsible disclosure process.

Security reports are eligible for bug bounty rewards (program launches Q2 2026).

---

## License

By contributing to QUANTA, you agree that your contributions will be licensed under the MIT License, the same license as the project.

---

## Recognition

Contributors will be recognized in:

- Repository contributors page
- Release notes (for significant contributions)
- Project website (major contributors)

---

## Questions?

If you have questions about contributing, please:

1. Check this document thoroughly
2. Search existing GitHub issues
3. Create a new discussion on GitHub Discussions
4. Reach out to maintainers if needed

---

**Thank you for contributing to QUANTA!**

Together, we're building the future of quantum-resistant blockchain technology.


