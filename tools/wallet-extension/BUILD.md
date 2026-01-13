# Quanta Wallet Extension - Build Instructions

## Prerequisites (WSL/Linux)

To build the WASM component with real Falcon-512 cryptography, you need a Rust environment with `clang` installed (required for compiling the C bindings in `pqcrypto-falcon`).

### 1. Install System Dependencies
```bash
sudo apt-get update
sudo apt-get install -y clang llvm build-essential
```

### 2. Install Rust
If you haven't installed Rust yet:
```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source $HOME/.cargo/env
```

### 3. Install wasm-pack
```bash
curl https://rustwasm.github.io/wasm-pack/installer/init.sh | sh
```

## Building

Navigate to the `wasm` directory and build:

```bash
cd tools/wallet-extension/wasm
wasm-pack build --target web --out-dir ../public/pkg_v2
```

## Running the Extension

1. Open Chrome/Brave.
2. Go to `chrome://extensions`.
3. Enable "Developer mode".
4. Click "Load unpacked" and select the `tools/wallet-extension/public` directory.
