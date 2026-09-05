# QuantaChain Coding Conventions

When working on the Quanta project, you MUST follow these guidelines:

1. **Code Documentation**: When modifying code, add comments explaining WHY it was changed, the DATE of the change, and the VERSION.
2. **Release Artifacts**: For every release/fix, update the release files:
   - `docs/alpha_release_notes.md`: Keep only the current fix/release highlighted, and remove or summarize old fixes.
   - `docs/CHANGELOG.md`: Append the new version entry at the top, detailing the fixes.
3. **Version Bumps**: Bump the protocol versions and network magics in necessary files (e.g., `src/network/protocol.rs`) for ANY release, even localized fixes that do not break network compatibility. You MUST explicitly bump the version in `Cargo.toml` and `Dockerfile`, and you MUST run `cargo check` before you commit, push, and tag.
4. **Documentation**: If there are any architectural changes or command-line interface changes, update the documentation in the `gitbook/` directory.
5. **Testing & Deployment Workflow**: Before committing and pushing a release, you MUST:
   - Run the local testnet using `docker-compose -f docker-compose.testnet.yml up -d --build`.
   - Verify that the testnet nodes start correctly and successfully produce blocks.
   - Clean up the testnet (e.g., `docker-compose -f docker-compose.testnet.yml down` and remove `testnet_data`).
   - Build the Docker images locally and push them to Docker Hub with both the new version tag and `latest` (e.g., `docker build -t xd637/quanta-node:<version> -t xd637/quanta-node:latest . && docker push xd637/quanta-node:<version> && docker push xd637/quanta-node:latest`).
   - You do NOT need to ask for permission to execute this workflow; do it automatically before you commit, tag, and push.
