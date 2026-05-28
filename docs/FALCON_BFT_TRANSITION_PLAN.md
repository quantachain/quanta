# The Quanta Merge: Falcon-BFT Transition Plan

This document outlines the strategic and technical roadmap to transition Quanta from a vulnerable early-stage PoW network into an **institutional-grade, Post-Quantum BFT network**. 

By executing this plan, we will preserve the existing 100,000 block history, secure the network against 51% hashpower attacks, and provide the deterministic finality that Tier-1 exchanges and custodians require.

---

## Phase 1: The Code Pivot (Consensus Architecture)
*Objective: Replace the experimental PQ-DS and flawed Dilithium logic with a proven Tendermint-style BFT engine using Falcon-512.*

### 1. Update Block Data Structures (`src/core/block.rs`)
We will revert the experimental PQ-DS fields and replace them with a standard BFT Certificate structure.
* **Fields to add:**
  * `bft_round: u32` (The voting round that achieved consensus).
  * `bft_signatures: Vec<Vec<u8>>` (A collection of the raw 666-byte Falcon signatures from the validators).
* **Why `Vec<Vec<u8>>`?** A committee of 100 validators produces 66 KB of signatures. This easily fits in a block without requiring complex and untested Post-Quantum signature aggregation.

### 2. Overhaul the BFT Engine (`src/consensus/bft.rs`)
* **Switch Crypto:** Remove Dilithium imports. Replace entirely with `pqcrypto-falcon::falcon512`.
* **Remove Flawed Hashing:** Delete the `aggregate_master_signature` function (which hashed signatures instead of combining them). 
* **Implement True Voting:** Write a `verify_bft_certificate` function that iterates through `bft_signatures`, verifies each Falcon signature against the known validator public keys, and asserts that $> \frac{2}{3}$ of the voting power has signed the block hash.

### 3. Define the Genesis Validator Set
Hardcode a list of 21 initial Falcon-512 public keys in the consensus engine. These will act as the initial "Authority" nodes controlling the BFT consensus until an on-chain staking system is fully audited and deployed.

---

## Phase 2: "The Merge" Hard Fork (Block 100,000)
*Objective: Transition the live network seamlessly from PoW to BFT without resetting the chain.*

We will program a strict consensus fork directly into the block validation logic.

```rust
pub fn is_valid(&self) -> bool {
    if self.height < 100_000 {
        // Legacy Protocol: Validate Proof of Work Hash Target
        return self.has_valid_pow();
    } else {
        // V2 Protocol: Validate Falcon-512 BFT Certificate
        return self.verify_bft_certificate();
    }
}
```

**The Result at Block 100,000:**
* Miners will stop hashing. 
* The 21 Authority nodes will begin proposing and signing blocks via BFT.
* The block time becomes perfectly predictable (e.g., exactly 5 seconds).
* Finality becomes **instant and deterministic**. Deep reorgs and tip-overrides become mathematically impossible unless 15 of the 21 nodes collude.

---

## Phase 3: The Business Pivot (Go-To-Market)
*Objective: Use the secured, instantly-final network to attract institutional capital and Top-Tier exchange listings.*

1. **The Pitch:** *"Quanta has successfully executed 'The Merge'. We are now the world's first live, Post-Quantum BFT network with instant finality."*
2. **Target Custodians:** With BFT finality, reaching out to custody providers (Fireblocks, BitGo) becomes viable. They reject probabilistic PoW chains with low hash rate, but they deeply understand and trust BFT.
3. **Raise Capital:** Use the live, stable network to raise a Seed/Series A round from Crypto VCs. 
4. **Expand the Set:** Use the capital to build out the on-chain staking contracts, allowing the network to safely decentralize from 21 validators to 100+ institutional partners.

---

## Next Immediate Steps
1. Discard `pq_ds.rs` (move it to a `/research` folder).
2. Rewrite `block.rs` to include the `bft_signatures` array.
3. Fix the signature verification loop in `bft.rs` using Falcon-512.


