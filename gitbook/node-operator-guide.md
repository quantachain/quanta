# Node Operator Guide

Quanta V2 removes PoW mining. Security is now provided by BFT Validators in a strictly permissioned testnet. 

> **Note:** Currently, only the validators hardcoded into the Genesis set can run a node. In future releases, we will implement full DPoS, allowing anyone to stake and participate in consensus.

## Running a Validator
If you are part of the Genesis set, you must inject your raw wallet key into your node to produce blocks.

1. **Generate a Wallet**
   ```bash
   quanta-wallet new-raw --file validator.qua
   ```
   *(Provide the address and public key to the Quanta core team to be included in the Genesis set)*

2. **Start Node as Validator**
   ```bash
   ./quanta start --validator-wallet validator.qua
   ```

## Configuration (`quanta.toml`)
You can override default ports and settings in `quanta.toml`.

```toml
[network]
p2p_port = 8333
rpc_port = 7782
api_port = 3000
max_peers = 50
```

*Note: The V2 network magic bytes are `Q2T2`. Do not attempt to connect to older nodes.*
