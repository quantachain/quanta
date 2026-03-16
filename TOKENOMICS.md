# QUANTA TOKENOMICS

**Comprehensive Economic Model Specification**

Version 2.0 — March 2026

**Founder**: Kishore K — [admin@quantachain.org](mailto:admin@quantachain.org) — [quantachain.org](https://quantachain.org)

---

## Table of Contents

1. [Overview](#1-overview)
2. [Supply Schedule](#2-supply-schedule)
3. [Block Rewards](#3-block-rewards)
4. [Fee Economics](#4-fee-economics)
5. [Anti-Dump Mechanisms](#5-anti-dump-mechanisms)
6. [Treasury Model](#6-treasury-model)
7. [Economic Security](#7-economic-security)
8. [Simulation Results](#8-simulation-results)
9. [Comparison with Other Chains](#9-comparison-with-other-chains)
10. [Future Economic Considerations](#10-future-economic-considerations)
11. [Economic Attack Vectors](#11-economic-attack-vectors)

---

## 1. Overview

### 1.1 Design Goals

The QUANTA economic model achieves:

1. **Long-term Sustainability**: Mining rewards remain attractive for decades, never reaching zero
2. **Fair Distribution**: No pre-mine, no ICO, no team allocation — 100% through proof-of-work mining
3. **Deflationary Pressure**: Fee burning creates permanent supply reduction
4. **Anti-Dump Protection**: 50% of mining rewards locked for 6 months — prevents launch-day sell cascades
5. **Development Funding**: Two transparent treasury streams (block reward allocation + fee share) for ongoing development

### 1.2 Key Parameters (Source: `src/consensus/blockchain.rs`)

| Parameter | Value | Code Constant |
|---|---|---|
| 1 QUA denomination | 1,000,000 microunits | `MICROUNITS_PER_QUA = 1_000_000` |
| Initial Block Reward | 100 QUA | `YEAR_1_REWARD = 100_000_000` |
| Annual Reduction | 15% | `ANNUAL_REDUCTION_PERCENT = 15` |
| Minimum Reward Floor | 5 QUA | `MIN_REWARD = 5_000_000` |
| Block Time Target | 30 seconds | `TARGET_BLOCK_TIME = 30` |
| Blocks Per Year | 1,051,200 | `BLOCKS_PER_YEAR = 1_051_200` |
| Fee Burn Rate | **70%** | `FEE_BURN_PERCENT = 70` |
| Fee to Treasury | **20%** | `FEE_TREASURY_PERCENT = 20` |
| Fee to Miner | **10%** | `FEE_VALIDATOR_PERCENT = 10` |
| Block Reward to Treasury | **5%** | `TREASURY_ALLOCATION_PERCENT = 5` |
| Mining Reward Lock | **50% for 6 months** | `MINING_REWARD_LOCK_PERCENT = 50` |
| Lock Duration | 52,560 blocks | `MINING_REWARD_LOCK_BLOCKS = 52_560` |
| Coinbase Maturity | 100 blocks | `COINBASE_MATURITY = 100` |
| Min Transaction Fee | 100 microunits (0.0001 QUA) | `MIN_TRANSACTION_FEE = 100` |
| Mempool Limit | 5,000 transactions | `MAX_MEMPOOL_SIZE = 5000` |
| Max Block Size | 2 MB (2,097,152 bytes) | `MAX_BLOCK_SIZE_BYTES = 2_097_152` |
| Max Block Transactions | **1,200** | `MAX_BLOCK_TRANSACTIONS = 1200` |

> **Note on Block TX Limit**: The limit was corrected from 2,000 to 1,200 in the production codebase. Falcon-512 transactions average ~1,713 bytes (666-byte signature + 897-byte public key + overhead). 1,200 × 1,713 = 2.06 MB, which fits within the 2 MB block size with minor overhead management.

---

## 2. Supply Schedule

### 2.1 Emission Formula (Integer Math — Consensus Critical)

The block reward uses **pure integer math** — no floating point. Floating-point divergence between CPU architectures would cause consensus forks. The formula applied iteratively ensures determinism on every platform:

```rust
fn apply_annual_reduction(start: u64, years: u64) -> u64 {
    let mut reward = start;
    let keep_pct = 85; // 100 - 15 (annual reduction)
    for _ in 0..years {
        reward = reward * keep_pct / 100;
        if reward <= MIN_REWARD {
            return MIN_REWARD; // 5 QUA floor
        }
    }
    reward
}

fn get_mining_reward(block_height: u64) -> u64 {
    let years_elapsed = block_height / BLOCKS_PER_YEAR;
    apply_annual_reduction(YEAR_1_REWARD, years_elapsed).max(MIN_REWARD)
}
```

Note: Integer division truncates. Over 20 years, the accumulated rounding error vs. an ideal `f64` computation is < 0.01 QUA — well within the 5 QUA floor.

### 2.2 Emission Schedule Table

| Year | Base Reward (QUA/block) | Blocks/Year | Annual Emission | Cumulative Supply |
|---|---|---|---|---|
| 1 | 100.00 | 3,153,600 | 315,360,000 | 315,360,000 |
| 2 | 85.00 | 3,153,600 | 268,056,000 | 583,416,000 |
| 3 | 72.25 | 3,153,600 | 227,847,600 | 811,263,600 |
| 4 | 61.41 | 3,153,600 | 193,670,460 | 1,004,934,060 |
| 5 | 52.20 | 3,153,600 | 164,619,891 | 1,169,553,951 |
| 10 | 19.69 | 3,153,600 | 62,094,634 | 1,417,612,585 |
| 15 | 7.43 | 3,153,600 | 23,431,238 | 1,482,043,823 |
| 20 | 5.00 (floor) | 3,153,600 | 15,768,000 | 1,503,811,823 |
| 50 | 5.00 (floor) | 3,153,600 | 15,768,000 | 1,977,051,823 |

### 2.3 Asymptotic Maximum Supply

```
Soft Maximum:   ~1.5 billion QUA (year 15–20, before floor kicks in)
Practical Max:  ~2 billion QUA (year 50)
True Maximum:   Infinite (due to 5 QUA perpetual floor)
```

This ensures:
- No "final Bitcoin" problem — mining never stops
- Perpetual security budget for miners
- Predictable long-term inflation (~0.8% annually after year 20)

### 2.4 Effective Circulating Supply

The anti-dump vesting mechanism means **not all mined QUA is immediately circulating**:

| Phase | Immediate (Circulating) | Locked (Vesting) |
|---|---|---|
| Per block | 47.5% of reward | 47.5% of reward (6-month lock) |
| Treasury | 5% of reward | Controlled by multisig |

Of 100 QUA mined per block in Year 1:
- 47.5 QUA → miner wallet immediately spendable
- 47.5 QUA → miner wallet locked for 52,560 blocks (~6 months)
- 5.0 QUA → treasury (immediately spendable for operations)

---

## 3. Block Rewards

### 3.1 Standard Block Reward

```
Block Reward = apply_annual_reduction(100 QUA, years_since_genesis)
             = max(100 × (85/100)^year, 5 QUA)
```

### 3.2 Reward Distribution (Per Block)

The total block reward `R` is distributed as follows:

```
Treasury Allocation:  R × 5%            → sent to treasury address
Miner Base Reward:    R × 95%           → available to miner

  Of Miner Base Reward:
    Immediate:        (R × 95%) × 50%   → credited to miner immediately
    Locked:           (R × 95%) × 50%   → locked until height + 52,560

Example at R = 100 QUA:
  Treasury:       5.0 QUA  (hardcoded address)
  Miner gets now: 47.5 QUA (immediately spendable)
  Miner locked:   47.5 QUA (vests after ~6 months)
```

> ⚠️ **Note**: Earlier documentation described a 50%/50% miner split as if the full 100 QUA were split. The correct formula first subtracts the 5% treasury allocation, then applies 50%/50% to the remaining 95%. This yields 47.5% immediate / 47.5% locked of the total block reward — matching the production `blockchain.rs` implementation exactly.

### 3.3 Network Usage Multiplier (Bootstrap Phase)

**Duration**: First 315,360 blocks (~36 days — the bootstrap phase)

**Range**: 1.0× to 2.0×

During the bootstrap phase, block rewards are boosted based on real fee activity to reward early miners and network contributors. Fee-based measurement (not transaction count) resists spam manipulation.

**Formula**:
```python
def usage_multiplier(block_height):
    if block_height >= 315_360:
        return 1.0  # No multiplier after bootstrap
    
    lookback = min(1000, block_height)
    recent_blocks = blocks[block_height - lookback : block_height]
    total_fees = sum(sum(tx.fee for tx in b.transactions) for b in recent_blocks)
    
    expected_minimum = 10_000_000  # 10 QUA in microunits (baseline activity)
    multiplier = 1.0 + min(1.0, total_fees / expected_minimum)
    return multiplier  # Range: [1.0, 2.0]
```

**Attack Resistance**:
- Miners cannot profitably spam: fees cost 100% but reward gain is at most 100% (net zero)
- 1000-block lookback averages out single-block manipulation
- Fee burning makes sustained fake activity expensive (70% of fee cost is permanently destroyed)

---

## 4. Fee Economics

### 4.1 Transaction Fees

**Minimum Fee**: 100 microunits (0.0001 QUA) — prevents network spam

**Fee Types by Transaction**:
| Transaction Type | Recommended Fee |
|---|---|
| Transfer | 1,000 microunits (0.001 QUA) |
| TimeLockTransfer | 5,000 microunits (0.005 QUA) |

**Fee Market**: Transactions are sorted highest-fee-first for block inclusion. A natural fee market emerges as mempool fills (5,000 TX cap).

### 4.2 Fee Distribution

Each block's total transaction fees (`F`) are split in fixed proportions:

| Recipient | Percentage | Destination |
|---|---|---|
| **Burn (destroyed)** | **70%** | Sent to unspendable address — permanent deflation |
| **Treasury** | **20%** | `ms69216b1d10425689704d5ae3b2a4aa17049f59b1` (3-of-5 multisig) |
| **Block Miner** | **10%** | Miner's coinbase address (added to immediate reward) |

> **Rounding**: `fee_burned + fee_to_treasury + fee_to_miner = total_fees` is guaranteed arithmetically. Any rounding remainder goes to the miner (preventing loss of microunits).

**Example (1,000 transactions × 0.001 QUA each)**:
```
Total fees:     1,000,000 microunits (1 QUA)
Burned:           700,000 microunits (0.70 QUA) — destroyed forever
Treasury:         200,000 microunits (0.20 QUA) — development fund
Miner:            100,000 microunits (0.10 QUA) — added to coinbase
```

### 4.3 Burn Mechanism

**Burn Implementation**: Fees tagged as "burned" are accounted for in token supply tracking but are never credited to any spendable address. The 70% burn fraction directly reduces circulating supply.

**Deflationary Effect Estimates**:

| Scenario | Annual TX | Avg Fee | Annual Burned | Annual Emission |
|---|---|---|---|---|
| Year 1, Low | 10M | 0.001 QUA | 7,000 QUA | 315,360,000 QUA |
| Year 5, Medium | 50M | 0.002 QUA | 70,000 QUA | 164,619,891 QUA |
| Year 10, High | 200M | 0.005 QUA | 700,000 QUA | 62,094,634 QUA |
| Year 20, Mature | 1B | 0.01 QUA | 7,000,000 QUA | 15,768,000 QUA |

At high adoption (Year 20+), fee burning could **exceed new emission**, making QUANTA net deflationary.

### 4.4 Treasury Accumulation from Fees

Treasury receives 20% of all transaction fees, creating a sustainable independent funding stream:

| Year | Est. Annual TX | Fee Revenue (20%) | Cumulative Treasury |
|---|---|---|---|
| 1 | 10M | ~2,000 QUA | ~2,000 QUA |
| 5 | 50M | ~20,000 QUA | ~62,000 QUA |
| 10 | 200M | ~140,000 QUA | ~500,000 QUA |

---

## 5. Anti-Dump Mechanisms

### 5.1 Mining Reward Vesting

**Purpose**: Prevent miners from immediately dumping all rewards on launch day, which would crash the price and undermine network confidence.

**Parameters**:
```
MINING_REWARD_LOCK_PERCENT  = 50  (% of miner's share that is locked)
MINING_REWARD_LOCK_BLOCKS   = 52,560  (≈ 6 months at 30-second blocks)
```

**Mechanism** (as implemented in `blockchain.rs`):

```rust
// Of the 95% miner reward:
let immediate_reward = (miner_reward * (100 - MINING_REWARD_LOCK_PERCENT)) / 100;
// = 95% × 50% = 47.5% of total block reward → credited immediately

let locked_reward    = miner_reward - immediate_reward;
// = 95% × 50% = 47.5% of total block reward → locked balance

// Locked balance is stored in account_state as LockedBalance {
//     amount: locked_reward,
//     unlock_height: current_height + MINING_REWARD_LOCK_BLOCKS
// }
```

**Unlock Behavior**:
- Locked balance automatically becomes spendable at `block_height >= unlock_height`
- Multiple concurrent locks possible (each mining event creates a new lock entry)
- Wallet displays: Available Balance / Locked Balance / Total Balance

**Coinbase Maturity**: In addition to the 6-month vesting, all mining rewards (including the immediate 47.5%) are subject to a **100-block coinbase maturity** before they can be included in a spend. This prevents spending mining rewards from orphaned blocks.

### 5.2 Economic Impact of Vesting

| Phase | Total Mined | Circulating (Unlocked) | Locked | Burned |
|---|---|---|---|---|
| Day 1 | 720,000 QUA | 342,000 QUA | 342,000 QUA | 0 QUA |
| Month 1 | ~21.6M QUA | ~10.3M QUA | ~10.3M QUA | ~100 QUA |
| Month 6 | ~130M QUA | ~97M QUA | ~33M QUA | ~7,000 QUA |
| Year 1 | ~315M QUA | ~275M QUA | ~40M QUA | ~50,000 QUA |

After 6 months, the 6-month rolling lock stabilizes at ~12–13% of total mined supply.

---

## 6. Treasury Model

### 6.1 Treasury Funding Streams

The treasury receives two distinct income streams:

| Source | Amount | Frequency |
|---|---|---|
| Block Reward Allocation | 5% of each block reward | Every block (~30 seconds) |
| Fee Share | 20% of block's total transaction fees | Every block (when fees > 0) |

**Year 1 projections**:
- Block allocation: 1,051,200 blocks × 5 QUA = **~5,256,000 QUA/year** from blocks
- Fee share: ~10M TX × 0.001 QUA × 20% = **~2,000 QUA/year** from fees (initially modest)

### 6.2 Treasury Address

```
Treasury Address: ms69216b1d10425689704d5ae3b2a4aa17049f59b1
Type:             3-of-5 Falcon-512 multisig (generated 2026-03-14)
Threshold:        Any 3 of 5 keyholders must sign to spend
```

This is a **consensus constant** hardcoded in `src/consensus/blockchain.rs`. Every node enforces that the treasury transaction in each block targets exactly this address. Tampering with the address causes instant block rejection. The address cannot be changed via `quanta.toml` — only a coordinated network upgrade (hard fork) can change it.

See [GOVERNANCE.md](GOVERNANCE.md) for spending procedures and keyholder policy.

### 6.3 Allocation Guidelines

Recommended distribution of treasury funds:

| Category | Allocation | Purpose |
|---|---|---|
| Core Development | 40% | Developer salaries, infrastructure |
| Security | 25% | Audits, bug bounties, penetration testing |
| Ecosystem Grants | 20% | DApps, tools, integrations, SDK |
| Marketing & Community | 10% | Exchange listings, awareness |
| Reserve | 5% | Emergency fund |

### 6.4 Governance

**Current Model (Year 1 — Off-Chain)**:
- Kishore K (Founder) and core team proposals
- Community feedback on GitHub Discussions
- Quarterly transparency reports with full transaction history

**Future Model (Year 2+ — On-Chain)**:
- Token-weighted voting on treasury proposals
- Time-locked spending with community veto period
- On-chain proposal submission and execution

### 6.5 Treasury Multisig

Treasury is controlled by a **3-of-5 multisig** Falcon-512 threshold scheme:

| Signers | Requirement |
|---|---|
| Kishore K (Founder) | Required for major decisions |
| Core Developer 2 | |
| Core Developer 3 | |
| Community Representative 1 | |
| Community Representative 2 | |

**Signing Policy**:
- Routine expenses (< 10,000 QUA): Any 3 of 5 signers
- Major expenses (> 10,000 QUA): All 5 signers + public announcement
- Emergency expenses: 3 signers + post-facto public disclosure

---

## 7. Economic Security

### 7.1 51% Attack Cost

```
Attack Cost = (Hashrate_needed × Duration × Energy_cost) + Hardware_cost
```

Year 1 estimates:
- Network hashrate: ~10 TH/s
- Hardware (ASIC): ~$1,000,000
- Energy: ~$50,000/hour
- 1-hour attack cost: **~$1,050,000**

**Defense Layers**:
1. Checkpoint system prevents deep reorgs below checkpoint heights
2. High block reward attracts honest miners (stronger network hashrate)
3. Exchange social consensus — require 20+ confirmations for large deposits

### 7.2 Miner Profitability (Year 1 Estimates)

**Revenue Per Block**:
```
Block reward (immediate): 47.5 QUA
Miner fee share (10%):     ~0.01 QUA (typical)
Total immediate revenue:  ~47.51 QUA per block

At $0.10/QUA:  ~$4.75 per block immediate
               ~$4.75 per block locked (vests over 6 months)
               ~$9.50 total value per block
```

**Break-Even**:
```
Energy cost per block:     ~$5 (estimated)
Immediate revenue:         $4.75
Locked revenue (6-month):  $4.75 (deferred)
Net daily profit (immediate only): variable
Long-term ROI (including locked): positive at $0.10+ per QUA
```

Miners who HODL their locked rewards benefit from both network security and appreciation.

### 7.3 Fee Market Dynamics

**Low Activity (Early Network)**:
- Minimum fees only (0.0001 QUA)
- All transactions included
- No priority bidding needed

**High Activity (Mature Network)**:
- Mempool fills (5,000 TX limit)
- Users bid higher fees for inclusion
- Fee market emerges naturally
- Miner revenue transitions from block reward to fees (Bitcoin's long-term model)

---

## 8. Simulation Results

### 8.1 Supply Growth Projection

```
Year 1:  315.4 million QUA   (100 QUA/block, 3.15M blocks)
Year 2:  583.4 million QUA   (85 QUA/block, cumulative)
Year 3:  811.3 million QUA
Year 5:  1,169.6 million QUA
Year 10: 1,417.6 million QUA
Year 20: 1,503.8 million QUA  (floor reached ~year 18)
Year 50: 1,977.1 million QUA
```

**Inflation Rate** (excluding fee burn):
```
Year 2:  84.9%   (early growth phase)
Year 3:  39.0%
Year 5:  14.1%
Year 10:  4.4%
Year 20:  1.0%
Year 50:  0.8%   (Bitcoin-like long-term inflation)
```

### 8.2 Circulating vs. Locked Supply

Due to 6-month vesting, effective circulating supply is significantly lower than total mined:

| Month | Total Mined | Circulating | Locked | Ratio Circulating |
|---|---|---|---|---|
| 1 | 21.6M QUA | 10.3M QUA | 10.3M QUA | 47.5% |
| 6 | 130M QUA | 97M QUA | 33M QUA | 75% |
| 12 | 315M QUA | 275M QUA | 40M QUA | 87% |
| Steady State | — | ~87% | ~13% | 87% |

### 8.3 Fee Burn Impact

**Conservative Scenario** (10M TX/year):
- Year 5: ~50,000 QUA burned
- Year 10: ~200,000 QUA burned
- Year 20: ~1,000,000 QUA burned

**Optimistic Scenario** (100M+ TX/year at maturity):
- Year 5: ~500,000 QUA burned
- Year 10: ~5,000,000 QUA burned
- Year 20: ~50,000,000 QUA burned — net deflationary by year 20

---

## 9. Comparison with Other Chains

### 9.1 vs Bitcoin

| Feature | Bitcoin | QUANTA |
|---|---|---|
| Initial Reward | 50 BTC | 100 QUA |
| Reduction Method | 50% halving every 4 years | 15% smooth annual decay |
| Final Supply | 21M (hard cap, ~2140) | ~1.5B soft cap (5 QUA floor) |
| Security Budget | Ends ~2140 | Perpetual (never zero) |
| Fee Burning | None | 70% of all fees |
| Pre-mine | None | None |
| Anti-dump | None | 50% locked 6 months |
| Treasury | None | 5% block + 20% fees |
| Quantum Resistance | ❌ ECDSA | ✅ Falcon-512 (NIST) |

**QUANTA Advantages over Bitcoin**:
- Smoother emission (no halving shocks)
- Perpetual mining incentive
- Strong deflationary pressure via burn
- Quantum-resistant by design

### 9.2 vs Ethereum

| Feature | Ethereum (PoS) | QUANTA |
|---|---|---|
| Consensus | Proof-of-Stake | Proof-of-Work (ASIC-resistant) |
| Issuance | ~0.5% annual | 15% → 0.8% over 20 years |
| Fee Burning | EIP-1559 (variable) | 70% (fixed, predictable) |
| Mining Lock | N/A | 50% of rewards, 6 months |
| Initial Distribution | ICO + pre-mine | Fair launch mining |
| Quantum Resistance | ❌ ECDSA | ✅ Falcon-512 (NIST) |

### 9.3 vs Monero

| Feature | Monero | QUANTA |
|---|---|---|
| Initial Emission | Fast (18.4M in 4 years) | Gradual (1.5B over 15 years) |
| Tail Emission | 0.6 XMR/block | 5 QUA/block |
| Privacy | Native (Ring Signatures) | Planned (future) |
| Quantum Resistance | ❌ None | ✅ Full PQC |

---

## 10. Future Economic Considerations

### 10.1 Transition to Fee-Based Security

As block rewards decline, network security transitions to fee-based model (as Bitcoin is designed to do by ~2140, QUANTA achieves this organically by ~year 20):

| Year | Block Reward | Expected Fees (10% miner) | Total Miner Revenue |
|---|---|---|---|
| 5 | 52 QUA | ~0.5 QUA | 52.5 QUA |
| 10 | 20 QUA | ~5 QUA | 25 QUA |
| 20 | 5 QUA | ~50 QUA | 55 QUA |

By year 20, **fees exceed block rewards** — a fully sustainable fee economy.

### 10.2 Potential Governance-Driven Adjustments

All changes require hard fork + community consensus:
- Fee burn rate: 70% ± 10% (range 60–80%)
- Lock percentage: 50% ± 20% (range 30–70%)
- Lock duration: 6 months ± 3 months (range 3–12 months)
- Treasury allocation: 5% ± 2% (range 3–7%)

### 10.3 PoW → PoS Transition (Planned)

Consensus engine is configurable via `quanta.toml`:

```toml
consensus_engine = "proof_of_work"   # current (live)
# consensus_engine = "proof_of_stake"  # planned — node will refuse to start until implemented
```

When PoS is implemented, validator staking rewards will supplement (then replace) PoW mining rewards. The treasury model (5% allocation + 20% fee share) remains unchanged across both consensus engines.

See [GOVERNANCE.md §4](GOVERNANCE.md) for the full PoS transition roadmap and validator economics.

---

## 11. Economic Attack Vectors

### 11.1 Fee Market Manipulation

**Attack**: Miner includes own transactions to inflate usage multiplier during bootstrap  
**Mitigation**: 70% burn cost — spending 10 QUA gains ≤ 10 QUA bonus (net zero or negative)  
**Scope**: Only affects first 315,360 blocks (~36 days)

### 11.2 Selfish Mining

**Attack**: Withhold valid blocks to gain advantage over competing miners  
**Mitigation**: 30-second block time minimizes orphan risk; checkpoint system prevents deep reorgs  
**Economics**: Selfish mining requires >25% hashrate to be profitable — very high barrier

### 11.3 Long-Range Attack

**Attack**: Rewrite chain from genesis with a quantum computer  
**Mitigation**: Falcon-512 signatures resist quantum forgery; hardcoded checkpoints in all node binaries; social consensus at exchanges and wallets

### 11.4 Treasury Drain

**Attack**: Compromise treasury multisig signers  
**Mitigation**: 3-of-5 Falcon-512 multisig; major expenses require all 5 signers + public announcement; on-chain visibility of all treasury transactions

---

## Appendix A: Economic Formulas (Code-Accurate)

### Block Reward (Integer Math)
```rust
// From blockchain.rs — consensus-critical, no f64
fn get_mining_reward(&self) -> u64 {
    let years_elapsed = self.get_height() / BLOCKS_PER_YEAR;
    let mut reward = YEAR_1_REWARD; // 100 QUA = 100_000_000 microunits
    let keep_pct = 100 - ANNUAL_REDUCTION_PERCENT; // = 85
    for _ in 0..years_elapsed {
        reward = reward * keep_pct / 100;
        if reward <= MIN_REWARD { return MIN_REWARD; } // 5 QUA floor
    }
    reward
}
```

### Block Reward Distribution
```rust
// 5% treasury allocation
let treasury_allocation = (reward * TREASURY_ALLOCATION_PERCENT) / 100;
// = reward × 5 / 100

// 95% to miner (split 50/50)
let miner_reward = reward - treasury_allocation;
let immediate_reward = (miner_reward * (100 - MINING_REWARD_LOCK_PERCENT)) / 100;
// = miner_reward × 50 / 100
// = (reward × 95 / 100) × 50 / 100
// ≈ 47.5% of total block reward

let locked_reward = miner_reward - immediate_reward;
// ≈ 47.5% of total block reward, locked for 52,560 blocks
```

### Fee Distribution
```rust
// Integers only — rounding remainder goes to miner
let fee_burned      = (total_fees * FEE_BURN_PERCENT) / 100;      // 70%
let fee_to_treasury = (total_fees * FEE_TREASURY_PERCENT) / 100;  // 20%
let fee_to_miner    = total_fees - fee_burned - fee_to_treasury;   // 10% + remainder
```

### Mining Lock
```rust
// Account state stores locked balances
account_state.add_locked_balance(
    miner_address,
    locked_reward,
    current_height + MINING_REWARD_LOCK_BLOCKS  // current + 52,560
);
```

---

## Appendix B: Treasury Multisig Configuration

**Treasury Address**: `ms69216b1d10425689704d5ae3b2a4aa17049f59b1`  
**Type**: 3-of-5 Falcon-512 multisig — generated 2026-03-14

**Keyholder Keys** (from `treasury_keys/treasury_setup.json`):
1. `treasury_key0.qua` — address `0x5372c47e617180f95c6e8a957b3e3c3a7c17ec7a`
2. `treasury_key1.qua` — address `0x9430dc395f9be6d76873dc6fa703f1ebb4acb4e5`
3. `treasury_key2.qua` — address `0x6f64731ab168a114ed1a39aa6beeb4b59202239e`
4. `treasury_key3.qua` — address `0x9e5995fab9d6246e37d9e9bb30c10a1dfeff17f7`
5. `treasury_key4.qua` — address `0x1160d8504f9cb2b4b3e621114c90c7a8a0bc41d8`

**Signing Thresholds**:
| Expense Level | Required Signatures | Public Notice |
|---|---|---|
| < 10,000 QUA | Any 3 of 5 | Optional |
| > 10,000 QUA | All 5 signers | Required (7-day public notice) |
| Emergency | Any 3 of 5 | Post-facto public disclosure |

Full spending procedures: [GOVERNANCE.md](GOVERNANCE.md)

---

**Document Version**: 2.1  
**Last Updated**: 2026-03-14  
**Founder**: Kishore K (admin@quantachain.org)  
**License**: CC BY 4.0
