# Quasar Smart Contract Framework

**Quasar** is Quanta's smart contract platform - a Solana-inspired, quantum-resistant framework for building decentralized applications.

## 🌟 Key Features

### Quantum-Safe by Default
- **Falcon-512 signatures** - Post-quantum secure
- **Kyber-1024 encryption** - Future-proof privacy
- **Quantum random oracle** - True randomness
- **Lattice-based crypto** - Resistant to quantum attacks

### Solana-Like Architecture
- **Account model** - Everything is an account
- **WASM runtime** - Fast, sandboxed execution
- **Gas metering** - Resource management
- **Parallel execution** - High throughput (future)

### Developer Experience
- **Rust-first** - Type-safe contract development
- **Clean APIs** - Familiar patterns for Solana devs
- **Built-in primitives** - Quantum functions out of the box
- **Standard library** - QSP tokens, NFTs, DeFi

## 📦 Architecture

```
┌─────────────────────────────────────────────┐
│         Smart Contract Layer                │
│  (Account-based, Rust contracts → WASM)     │
├─────────────────────────────────────────────┤
│      Quantum Security Middleware            │
│  • Falcon signatures                        │
│  • Kyber encryption                         │
│  • Quantum RNG                              │
│  • Program Derived Addresses (PDAs)         │
├─────────────────────────────────────────────┤
│       Execution Engine (Wasmer)             │
│  • Gas metering                             │
│  • Host functions                           │
│  • Memory isolation                         │
├─────────────────────────────────────────────┤
│        Quanta Blockchain                    │
│  • UTXO backend                             │
│  • PoW consensus                            │
│  • Falcon signatures                        │
└─────────────────────────────────────────────┘
```

## 🚀 Quick Start

### 1. Create a Contract

```rust
// contracts/my_token/src/lib.rs
#[repr(C)]
pub struct TokenAccount {
    pub owner: [u8; 32],
    pub amount: u64,
}

#[no_mangle]
pub extern "C" fn process_instruction() -> i32 {
    // Your contract logic
    0 // Success
}
```

### 2. Build to WASM

```bash
cd contracts/my_token
cargo build --release --target wasm32-unknown-unknown
```

### 3. Deploy Contract

```bash
quanta deploy-contract \
  --wasm target/wasm32-unknown-unknown/release/my_token.wasm \
  --fee 0.1
```

### 4. Call Contract

```bash
quanta call-contract \
  --contract <contract_address> \
  --function transfer \
  --args '{"to": "0xabc...", "amount": 1000}' \
  --fee 0.01
```

## 🔧 Host Functions

Contracts can call these functions provided by the runtime:

### Gas & Logging
- `consume_gas(amount: u64)` - Charge gas for operations
- `log(ptr: *const u8, len: u32)` - Emit log messages

### Account Access
- `get_account_balance(index: u32) -> u64` - Read account balance
- `set_account_data(index: u32, ptr: *const u8, len: u32)` - Write account data
- `get_account_data(index: u32, ptr: *mut u8, max_len: u32) -> i32` - Read account data

### Quantum Primitives
- `quantum_random(max: u32) -> u32` - Get quantum random number
- `sha3_hash(ptr: *const u8, len: u32, out_ptr: *mut u8)` - SHA3-256 hash
- `falcon_verify(msg, sig, pk)` - Verify Falcon signature
- `get_block_height() -> u64` - Get current block height

## 📚 Standard Programs (QSP)

### QSP-Token
Quantum-safe fungible tokens (like SPL-Token).

**Instructions:**
- `InitializeMint` - Create token mint
- `InitializeAccount` - Create token account
- `Transfer` - Transfer tokens
- `MintTo` - Mint new tokens
- `Burn` - Burn tokens
- `FreezeAccount` - Freeze/thaw accounts

### QSP-NFT (Future)
Quantum-safe non-fungible tokens.

### QSP-AMM (Future)
Automated market maker for token swaps.

## 🛠️ Development

### Gas Costs

```rust
pub mod gas_costs {
    pub const BASE_INSTRUCTION: u64 = 1;
    pub const MEMORY_READ: u64 = 2;
    pub const MEMORY_WRITE: u64 = 3;
    pub const STORAGE_READ: u64 = 100;
    pub const STORAGE_WRITE: u64 = 200;
    pub const CRYPTO_HASH: u64 = 50;
    pub const CRYPTO_VERIFY: u64 = 1000;
}
```

### Transaction Types

**Deploy Contract:**
```rust
Transaction::new_deploy_contract(
    deployer: String,
    code: Vec<u8>,  // WASM bytecode
    timestamp: i64,
    fee: f64,
)
```

**Call Contract:**
```rust
Transaction::new_call_contract(
    sender: String,
    contract: String,
    function: String,
    args: Vec<u8>,
    amount: f64,     // QUA to send
    timestamp: i64,
    fee: f64,
)
```

## 🔐 Security Features

### Quantum-Safe Signatures
All contract calls must be signed with Falcon-512:
```rust
// Verify caller identity
if !falcon_verify(msg, sig, pk) {
    return Err("Invalid signature");
}
```

### Program Derived Addresses
Deterministic addresses derived from seeds:
```rust
let (pda, bump) = QuantumPDA::find_program_address(
    &[b"token-account", user_key, mint_key],
    program_id,
);
```

### Gas Limits
Prevents infinite loops:
```rust
// Max 10M gas per transaction
pub const MAX_GAS_PER_TX: u64 = 10_000_000;
```

## 📖 Examples

### Simple Counter Contract

```rust
#[repr(C)]
struct Counter {
    value: u64,
}

#[no_mangle]
pub extern "C" fn process_instruction() -> i32 {
    charge_gas(100);
    
    // Read current value
    let mut data = [0u8; 8];
    get_account_data(0, data.as_mut_ptr(), 8);
    let mut counter = u64::from_le_bytes(data);
    
    // Increment
    counter += 1;
    
    // Write back
    let bytes = counter.to_le_bytes();
    set_account_data(0, bytes.as_ptr(), 8);
    
    log_message("Counter incremented");
    0
}
```

### Quantum Lottery

```rust
#[no_mangle]
pub extern "C" fn process_instruction() -> i32 {
    charge_gas(500);
    
    // Pick random winner from 100 participants
    let winner_index = quantum_random(100);
    
    // Award prize to winner
    // ... transfer logic ...
    
    0
}
```

## 🎯 Roadmap

- [x] Account model and storage
- [x] WASM runtime with gas metering
- [x] Quantum primitives
- [x] QSP-Token standard
- [ ] CLI integration
- [ ] Blockchain execution
- [ ] API endpoints
- [ ] QSP-NFT standard
- [ ] QSP-AMM (DEX)
- [ ] Cross-program invocation (CPI)
- [ ] Proper WASM memory access
- [ ] Contract upgrade mechanism
- [ ] Formal verification tools

## 🤝 Contributing

Quasar is part of the Quanta blockchain project. To contribute:

1. Fork the repository
2. Create a feature branch
3. Write tests
4. Submit a pull request

## 📄 License

Same as Quanta blockchain - see LICENSE file.

---

**Built with ❤️ for a quantum-safe future**
