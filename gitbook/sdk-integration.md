# SDK Integration Guide

The `quanta-sdk` is the official JavaScript/TypeScript toolkit for building on the Quanta Protocol. It wraps the node REST API and integrates the Falcon-512 WASM cryptography engine.

**npm**: [`quanta-sdk`](https://www.npmjs.com/package/quanta-sdk)
**GitHub**: [quantachain/quanta-sdk](https://github.com/quantachain/quanta-sdk)

---

## Installation

```bash
npm install quanta-sdk
```

---

## Initialization

The SDK uses `quanta-wasm` internally for Falcon-512 cryptography. You must call `initQuanta()` once before any cryptographic operations:

```typescript
import { initQuanta } from 'quanta-sdk';

await initQuanta();
// All SDK crypto functions are now available
```

Call `initQuanta()` once at app startup — not before every function call.

---

## Connecting to a Node

```typescript
import { QuantaClient } from 'quanta-sdk';

// Public testnet RPC endpoint
const client = new QuantaClient('https://rpc.quantachain.org');

// Or your own node
const client = new QuantaClient('http://localhost:3000');

// Check node health
const health = await client.getHealth();
console.log(`Block height: ${health.blockchain_height}`);
console.log(`Peers: ${health.peer_count}`);
```

---

## Wallet Management

### Create a New Wallet

```typescript
import { QuantaWallet, initQuanta } from 'quanta-sdk';

await initQuanta();

const wallet = QuantaWallet.create();
console.log('Address:', wallet.address);
console.log('Mnemonic:', wallet.mnemonic);  // 24-word recovery phrase — store offline
```

### Restore from Mnemonic

```typescript
const wallet = QuantaWallet.fromMnemonic(
  "word1 word2 word3 ... word24"
);
console.log('Address:', wallet.address);
```

---

## Checking Balances

```typescript
const client = new QuantaClient('https://rpc.quantachain.org');

const balance = await client.getBalance('0xYOUR_ADDRESS');
console.log(`Balance: ${balance / 1_000_000} QUA`);   // convert from microunits
```

Amounts are always in **microunits** (1 QUA = 1,000,000 microunits).

---

## Sending a Transaction

```typescript
import { QuantaClient, QuantaWallet, TransactionBuilder, initQuanta } from 'quanta-sdk';

await initQuanta();

const client = new QuantaClient('https://rpc.quantachain.org');
const wallet = QuantaWallet.fromMnemonic("your twenty four word mnemonic here...");

// 1. Fetch current nonce
const nonce = await client.getNonce(wallet.address);

// 2. Build the unsigned transaction
const unsignedTx = TransactionBuilder.createUnsigned(
  wallet.address,       // sender
  '0xRECIPIENT',        // recipient
  5_000_000,            // 5 QUA in microunits
  nonce + 1             // nonce must be exactly current + 1
);

// 3. Sign with Falcon-512
const signedTx = TransactionBuilder.sign(unsignedTx, wallet);

// 4. Broadcast
const response = await client.submitTransaction(signedTx);
console.log('Transaction hash:', response.tx_hash);
```

---

## TimeLock Transfers (Escrow / Vaulting)

Lock funds on the recipient's account until a specific block height:

```typescript
const currentHeight = (await client.getLatestBlock()).index;
const unlockAt = currentHeight + 52560;   // ~6 months

const unsignedTx = TransactionBuilder.createUnsigned(
  wallet.address,
  '0xRECIPIENT',
  10_000_000,    // 10 QUA
  nonce + 1,
  { type: 'TimeLockTransfer', unlock_height: unlockAt }
);

const signedTx = TransactionBuilder.sign(unsignedTx, wallet);
const response = await client.submitTransaction(signedTx);
```

The recipient cannot spend these funds until block `unlockAt`.

---

## Fetching Blocks and Transactions

```typescript
// Get latest block
const latestBlock = await client.getLatestBlock();
console.log('Height:', latestBlock.index);
console.log('TXs:', latestBlock.transactions.length);

// Get block by height
const block = await client.getBlock(1000);

// Get transaction by hash
const tx = await client.getTransaction('TX_HASH');
console.log('Sender:', tx.sender);
console.log('Amount:', tx.amount / 1_000_000, 'QUA');
```

---

## Mempool

```typescript
const mempool = await client.getMempool();
console.log(`Pending transactions: ${mempool.count}`);
```

---

## CLI Utility

The SDK ships with a built-in CLI:

```bash
# Generate a new wallet
npx quanta-cli wallet generate

# Check node status
npx quanta-cli node status https://rpc.quantachain.org

# Check address balance
npx quanta-cli balance 0xYOUR_ADDRESS
```

---

## Using `quanta-wasm` Directly

Most developers should use `quanta-sdk` instead of `quanta-wasm` directly. Use `quanta-wasm` only if you need raw Falcon-512 operations without the SDK abstraction layer.

```typescript
import init, { generate_keypair, sign_transaction } from 'quanta-wasm';

await init();    // must be called first in browser environments

const keypair = generate_keypair();
```

See the [`quanta-wasm` README](https://github.com/quantachain/quanta-wasm) for the full WASM API.

---

## Error Handling

```typescript
try {
  const response = await client.submitTransaction(signedTx);
  console.log('Success:', response.tx_hash);
} catch (err) {
  if (err.status === 422) {
    console.error('Transaction rejected:', err.message);
    // Common causes: wrong nonce, insufficient balance, invalid signature
  } else {
    console.error('Network error:', err);
  }
}
```

---

## License

ISC License — see [quanta-sdk/LICENSE](https://github.com/quantachain/quanta-sdk/blob/main/LICENSE).
