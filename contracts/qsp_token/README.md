# QSP-Token Standard

The Quantum Safe Program Token standard for the Quanta blockchain.

## Features

- **Quantum-resistant**: All signatures use Falcon-512
- **Account-based**: Similar to SPL-Token design
- **Gas-metered**: All operations consume gas
- **Frozen accounts**: Support for freezing/thawing

## Building

```bash
cd contracts/qsp_token
cargo build --release --target wasm32-unknown-unknown
```

The compiled WASM will be in `target/wasm32-unknown-unknown/release/qsp_token.wasm`

## Instructions

### InitializeMint
Create a new token mint with supply and decimals.

### InitializeAccount  
Create a new token account for holding tokens.

### Transfer
Transfer tokens between accounts.

### MintTo
Mint new tokens (requires mint authority).

### Burn
Burn tokens from an account.

### FreezeAccount / ThawAccount
Freeze or unfreeze a token account.

## Example Usage

```rust
// Deploy the contract
let code = std::fs::read("qsp_token.wasm")?;
let tx = Transaction::new_deploy_contract(
    deployer_address,
    code,
    timestamp,
    0.1, // fee
);

// Initialize a mint
let init_mint_ix = ContractInstruction {
    program_id: token_program_id,
    accounts: vec![
        AccountMeta::new(mint_account, false, true),
        AccountMeta::new(authority, true, false),
    ],
    data: vec![0, 9, 0, 0, 0, 0, 0, 0, 0], // instruction + decimals
};
```
