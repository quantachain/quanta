# Wallet Operations

Manage your Quanta wallets with quantum-resistant cryptography.

## Create a New Wallet

Create a standard quantum-safe wallet:

```bash
./target/release/quanta new_wallet --file my_wallet.qua
```

## Create HD Wallet

Create a hierarchical deterministic wallet with BIP39 24-word mnemonic:

```bash
./target/release/quanta new_hd_wallet --file my_hd_wallet.qua
```

Save your mnemonic phrase securely. It cannot be recovered if lost.

## View Wallet Information

Display wallet details including your address:

```bash
./target/release/quanta wallet --file my_wallet.qua
```

For HD wallets:

```bash
./target/release/quanta hd_wallet --file my_hd_wallet.qua
```

## Check Balance

Check your wallet balance via the API:

```bash
curl -X POST http://localhost:3000/api/balance \
  -H "Content-Type: application/json" \
  -d '{"address": "YOUR_ADDRESS_HERE"}'
```

## Send Transactions

Send QUA to another address:

```bash
./target/release/quanta send \
  --wallet my_wallet.qua \
  --to RECIPIENT_ADDRESS \
  --amount 10000000 \
  --db ./quanta_data
```

Note: Amount is in microunits (1 QUA = 1,000,000 microunits)

## Security Best Practices

- Always backup your wallet file and mnemonic phrase
- Store wallet files in encrypted storage
- Never share your private keys or mnemonic
- Use strong passwords for wallet encryption
