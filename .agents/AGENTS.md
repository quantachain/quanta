# QuantaChain Coding Conventions

When working on the Quanta project, you MUST follow these guidelines:

1. **Code Documentation**: When modifying code, add comments explaining WHY it was changed, the DATE of the change, and the VERSION.
2. **Release Artifacts**: For every release/fix, update the release files:
   - `docs/alpha_release_notes.md`: Keep only the current fix/release highlighted, and remove or summarize old fixes.
   - `docs/CHANGELOG.md`: Append the new version entry at the top, detailing the fixes.
3. **Version Bumps**: Bump the protocol versions and network magics in necessary files (e.g., `Cargo.toml`, `Dockerfile`, `src/network/protocol.rs`) when breaking network compatibility.
4. **Documentation**: If there are any architectural changes or command-line interface changes, update the documentation in the `gitbook/` directory.
