# Quanta GitBook Documentation

This folder contains GitBook-ready documentation for the Quanta blockchain project.

## Documentation Structure

The documentation is organized into the following sections:

1. **installation.md** - Installation guide for building Quanta from source
2. **quick-start.md** - Quick start guide for running a node
3. **wallet-operations.md** - Wallet creation and management
4. **api-reference.md** - REST API and JSON-RPC reference
5. **configuration.md** - Node configuration guide
6. **node-operator-guide.md** - Complete guide for node operators
7. **mining-guide.md** - Mining setup and optimization
8. **technical-specs.md** - Technical specifications and architecture
9. **p2p-networking.md** - P2P networking and peer discovery
10. **quantum-resistance.md** - Quantum resistance and cryptography
11. **security.md** - Security best practices and threat model
12. **contributing.md** - Contributing guidelines

## Importing to GitBook

### Option 1: Manual Import

1. Create a new GitBook space
2. For each markdown file, create a new page in GitBook
3. Copy the content from each .md file
4. Organize pages according to the structure above

### Option 2: GitHub Integration

1. Push this folder to your GitHub repository
2. In GitBook, go to Integrations > GitHub
3. Connect your repository
4. Select the `gitbook` folder as the documentation source
5. GitBook will automatically sync changes

## Suggested GitBook Structure

```
Documentation
├── Getting Started
│   ├── Installation
│   └── Quick Start
├── User Guides
│   ├── Wallet Operations
│   └── Configuration
├── Node Operators
│   ├── Node Operator Guide
│   └── Mining Guide
├── Technical Reference
│   ├── API Reference
│   ├── Technical Specs
│   ├── P2P Networking
│   └── Quantum Resistance
└── Community
    ├── Security
    └── Contributing
```

## Customization

Feel free to:
- Adjust content to match your specific needs
- Add more sections as the project evolves
- Include screenshots and diagrams
- Add code examples and tutorials

## Notes

- All files are written in standard Markdown format
- No emojis are used (as requested)
- Content is based on the latest workspace files
- Documentation is concise and focused on essential information
