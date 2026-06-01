# AlephBFT Consensus

Quanta V2 has abandoned Proof-of-Work to solve the single biggest problem with legacy blockchains: **Probabilistic Finality**. AI Agents cannot wait 30 minutes for a chain to be considered secure. They need absolute mathematical finality instantly.

To achieve this, Quanta utilizes **AlephBFT**, an asynchronous Byzantine Fault Tolerance protocol.

## Deterministic 6-Second Blocks
Quanta V2 is configured with a strict `SLOT_SECONDS = 6` timing model. 
A designated proposer collects mempool transactions, builds a block template, and broadcasts it. Once `> 2/3` of the validator committee signs the block, it achieves instant finality.

There are no reorgs. There are no orphans.

## Network Isolation (`Q2T2`)
To prevent broadcast storms and cross-contamination with old V1 PoW nodes, the network magic bytes have been permanently changed to `Q2T2`. 

Furthermore, AlephBFT consensus messages are wrapped in a 1-byte envelope by the `QuantaNetworkBridge` to differentiate between broadcast and unicast traffic at the network layer.
