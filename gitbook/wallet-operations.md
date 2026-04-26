# Wallet Operations

Quanta wallets store Falcon-512 post-quantum keypairs. All wallet files are encrypted with Kyber-1024 and protected by an Argon2id password.

---

## Wallet Types

| Type | Command | Description |
|------|---------|-------------|
| **HD Wallet** (recommended) | `new_hd_wallet` | 24-word BIP39 mnemonic, multiple accounts, single recovery phrase |
| **Standard Wallet** | `new_wallet` | Single Falcon-512 keypair, simpler but no mnemonic recovery |

Use HD wallets for all new accounts. The 24-word mnemonic is the only way to recover an HD wallet.

---

## Create an HD Wallet

```bash
# Docker
docker exec -it quanta-node quanta new_hd_wallet --file hd_wallet.json

# Source build
./target/release/quanta new_hd_wallet --file hd_wallet.json
```

You will be prompted for a password. After creation:
- Write down your **24-word mnemonic** and store it offline
- Note your **address** (starts with `0x`)

Create an HD wallet with multiple accounts:

```bash
docker exec -it quanta-node quanta new_hd_wallet --file hd_wallet.json --accounts 3
```

---

## View Wallet Info

```bash
# Show HD wallet info (all accounts and addresses)
docker exec -it quanta-node quanta hd_wallet --file hd_wallet.json

# Show address only (offline — no node needed)
docker exec -it quanta-node quanta wallet_address --file hd_wallet.json

# Show wallet info with balance (requires running node)
docker exec -it quanta-node quanta wallet --file hd_wallet.json
```

---

## Check Balance

```bash
# Via CLI (requires running node)
docker exec -it quanta-node quanta wallet --file hd_wallet.json

# Via REST API (no wallet file needed)
curl http://localhost:3000/accounts/0xYOUR_ADDRESS/balance
```

Balance is returned in **microunits** — divide by 1,000,000 to get QUA.

```
47500000 microunits = 47.5 QUA
```

---

## Send a Transaction

Amounts are in **microunits** (1 QUA = 1,000,000 microunits).

```bash
# Docker
docker exec -it quanta-node quanta send \
  --wallet hd_wallet.json \
  --to 0xRECIPIENT_ADDRESS \
  --amount 5000000 \
  --db /home/quanta/quanta_data

# Source build
./target/release/quanta send \
  --wallet hd_wallet.json \
  --to 0xRECIPIENT_ADDRESS \
  --amount 5000000 \
  --db ./quanta_data
```

The node deducts the transaction fee automatically (minimum 100 microunits = 0.0001 QUA).

---

## TimeLock Transfer (Escrow / Vaulting)

Lock funds so they cannot be spent by the recipient until a specific block height:

This transaction type is available via the `quanta-sdk`. Specify `tx_type` as `TimeLockTransfer` with an `unlock_height`.

```typescript
import { TransactionBuilder } from 'quanta-sdk';

const tx = TransactionBuilder.createUnsigned(
  wallet.address,
  recipientAddress,
  amount,
  nonce,
  { type: 'TimeLockTransfer', unlock_height: 20000 }
);
```

---

## Multisig Wallets

Multisig addresses require M-of-N Falcon-512 signatures to spend. The treasury uses a **3-of-5 multisig** (`ms69216b1d10425689704d5ae3b2a4aa17049f59b1`).

Multisig address format:
```
"ms" + hex(SHA3-256(sorted_pubkeys))
```

---

## Address Format

Quanta addresses are derived from the Falcon-512 public key:

```
address = "0x" + hex(SHA3-256(public_key)[0:20])
```

Single-key addresses start with `0x`. Multisig addresses start with `ms`.

---

## Security Best Practices

- **Back up your mnemonic** — write it on paper, store offline. If lost, funds cannot be recovered.
- **Never share your wallet file or password** — your wallet file contains your encrypted secret key.
- **Keep wallet files off VPS servers** — sign transactions locally, then broadcast via the API.
- **Use HD wallets** — standard wallets have no mnemonic and are unrecoverable if the file is lost.
- **Test with small amounts** — especially on mainnet, always send a small test transaction first.

---

## Using the SDK for Wallet Operations

For programmatic wallet creation and transactions, use the `quanta-sdk`:

```typescript
import { QuantaWallet, initQuanta } from 'quanta-sdk';

await initQuanta();
const wallet = QuantaWallet.create();
console.log('Address:', wallet.address);
console.log('Mnemonic:', wallet.mnemonic);

// Restore from mnemonic
const restored = QuantaWallet.fromMnemonic("your twenty four words...");
```

See the [SDK Integration Guide](sdk-integration.md) for full documentation.
