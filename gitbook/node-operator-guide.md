# Node Operator Guide

Quanta V2 removes PoW mining. Security is now provided by BFT Validators who stake QUA to participate in the consensus committee.

## Running a Validator
To run a validator, you must generate a key and inject it into your node.

1. **Generate a Wallet**
   ```bash
   quanta-wallet new-raw --file validator.qua
   ```
2. **Stake QUA**
   ```bash
   quanta-wallet stake --wallet validator.qua --amount 10000
   ```
3. **Start Node as Validator**
   ```bash
   ./quanta start --validator validator.qua
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
