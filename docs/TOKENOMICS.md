# QUANTA TOKENOMICS

**Comprehensive Economic Model Specification — AI Agent Execution Layer Era**

Version 3.0 — May 2026

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

1. **Long-term Sustainability**: Validator rewards remain attractive for decades, never reaching zero
2. **Fair Distribution**: No ICO, no VC allocation — genesis allocation to founding validators, public emission via block rewards
3. **Deflationary Pressure**: 50% fee burning creates permanent supply reduction at scale
4. **Validator Incentive**: 35% of fees + 92% of block rewards flow to the BFT proposer committee
5. **AI Agent Native**: Sub-cent gas fees by design — QUA is execution fuel, not settlement currency
6. **Ecosystem Funding**: 8% block allocation + 15% fee share fund the Quanta Ecosystem Fund (QEF)

### 1.2 Key Parameters (Source: `src/consensus/blockchain.rs` + `bft_proposer.rs`)

| Parameter | Value | Code Constant |
|---|---|---|
| 1 QUA denomination | 1,000,000 microunits | `MICROUNITS_PER_QUA = 1_000_000` |
| Initial Block Reward | **50 QUA** | `YEAR_1_REWARD = 50_000_000` |
| Annual Reduction | 15% | `ANNUAL_REDUCTION_PERCENT = 15` |
| Minimum Reward Floor | **2 QUA** | `MIN_REWARD = 2_000_000` |
| BFT Slot Time | **6 seconds** | `SLOT_SECONDS = 6` (bft_proposer.rs) |
| Blocks Per Year | **5,256,000** | `BLOCKS_PER_YEAR = 5_256_000` |
| Fee Burn Rate | **50%** | `FEE_BURN_PERCENT = 50` |
| Fee to Ecosystem Fund | **15%** | `FEE_TREASURY_PERCENT = 15` |
| Fee to Validator | **35%** | `FEE_VALIDATOR_PERCENT = 35` |
| Block Reward to QEF | **8%** | `TREASURY_ALLOCATION_PERCENT = 8` |
| Validator Reward | 92% of block reward (full, no lock) | BFT proposer receives immediately |
| Coinbase Maturity | **500 blocks (~50 min)** | `COINBASE_MATURITY = 500` |
| Unbonding Period | **60 epochs (~4.2 days)** | `UNBONDING_EPOCHS = 60` |
| Committee (Testnet) | **7 validators** | `MAX_COMMITTEE_SIZE = 7` |
| Committee (Mainnet) | **21 validators** | Change at mainnet genesis |
| Min Transaction Fee | 100 microunits (0.0001 QUA) | `MIN_TRANSACTION_FEE = 100` |
| Mempool Limit | 5,000 transactions | `MAX_MEMPOOL_SIZE = 5000` |
| Max Block Size | 2 MB (2,097,152 bytes) | `MAX_BLOCK_SIZE_BYTES = 2_097_152` |
| Max Block Transactions | **1,200** | `MAX_BLOCK_TRANSACTIONS = 1200` |

> **Note on Block TX Limit**: Falcon-512 transactions average ~1,713 bytes. 1,200 × 1,713 = 2.06 MB — fits within the 2 MB block with minor overhead management.

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

fn get_block_reward(block_height: u64) -> u64 {
    let years_elapsed = block_height / BLOCKS_PER_YEAR; // 5,256,000 blocks/yr at 6s
    apply_annual_reduction(YEAR_1_REWARD, years_elapsed).max(MIN_REWARD)
}
```

Note: Integer division truncates. Over 20 years, accumulated rounding error vs `f64` is < 0.01 QUA — well within the 2 QUA floor.

### 2.2 Emission Schedule Table (v3 — 6s BFT slots, 50 QUA start)

| Year | Base Reward (QUA/block) | Blocks/Year | Annual Emission | Cumulative Supply |
|---|---|---|---|---|
| 1 | 50.00 | 5,256,000 | 263,800,000 | 263,800,000 |
| 2 | 42.50 | 5,256,000 | 223,380,000 | 487,180,000 |
| 3 | 36.13 | 5,256,000 | 189,979,000 | 677,159,000 |
| 4 | 30.70 | 5,256,000 | 161,413,200 | 838,572,200 |
| 5 | 26.10 | 5,256,000 | 137,181,600 | 975,753,800 |
| 10 | 9.85 | 5,256,000 | 51,771,600 | 1,242,000,000 |
| 15 | 3.72 | 5,256,000 | 19,551,320 | 1,380,000,000 |
| 20 | 2.00 (floor) | 5,256,000 | 10,512,000 | 1,430,000,000 |
| 50 | 2.00 (floor) | 5,256,000 | 10,512,000 | 1,745,000,000 |

### 2.3 Asymptotic Maximum Supply

```
Soft Maximum:   ~1.38 billion QUA (year 15, before floor kicks in)
Practical Max:  ~1.75 billion QUA (year 50)
True Maximum:   Infinite (due to 2 QUA perpetual floor — perpetual validator incentive)
```

This ensures:
- No "final reward" problem — validators always earn block rewards
- Perpetual security budget for the BFT committee
- Predictable long-term inflation (~0.6% annually after year 20)
- Tighter supply than v2 (~1.75B vs ~2B) — better per-unit value dynamics

### 2.4 Effective Circulating Supply

In DPoS, the anti-dump mechanism is **validator staking + unbonding**, not a vesting lock:

| Allocation | Status | Lock Mechanism |
|---|---|---|
| Validator staked QUA | Non-circulating while active | Staked — locked until unstake tx |
| QEF multisig | Non-circulating until spent | 3-of-5 Falcon-512 multisig |
| Block reward (proposer) | Spendable after maturity | 500-block coinbase maturity (~50 min) |
| Unstaked QUA | Locked during unbonding | 60 epochs (~4.2 days) then released |

Of 50 QUA produced per block in Year 1:
- 46 QUA → block proposer (spendable after 500-block maturity)
- 4 QUA → Quanta Ecosystem Fund (QEF multisig)

---

## 3. Block Rewards

### 3.1 Standard Block Reward

```
Block Reward = apply_annual_reduction(50 QUA, years_since_genesis)
             = max(50 × (85/100)^year, 2 QUA)
```

### 3.2 Reward Distribution (Per Block)

The total block reward `R` is distributed as follows:

```
QEF Allocation:       R × 8%   → Quanta Ecosystem Fund (multisig)
Proposer Reward:      R × 92%  → active BFT block proposer (full, no lock)

Example at R = 50 QUA (Year 1):
  QEF:            4.0 QUA  (hardcoded QEF address)
  Proposer now:  46.0 QUA  (spendable after 500-block coinbase maturity ~50 min)
```

In DPoS+BFT, **there is no mining lock**. The proposer receives the full 92% immediately after coinbase maturity (500 blocks). Anti-dump is achieved through:
- **Staking lock** — validator QUA locked while in active committee
- **Unbonding** — 60 epochs (~4.2 days) after `Unstake` transaction

### 3.3 Validator Committee Earnings (Year 1 — 7 validators)

| Period | Per-Validator Block Rewards | At $0.001/QUA | At $0.01/QUA |
|---|---|---|---|
| Per epoch (1,000 blocks) | ~6,571 QUA | ~$6.57 | ~$65.71 |
| Per day (~14.4 epochs) | ~94,622 QUA | ~$94.62 | ~$946.22 |
| Per year | ~34,537,000 QUA | ~$34,537 | ~$345,370 |

> **Note**: With 21 mainnet validators, per-validator rewards are ÷3 (~$11,512/yr at $0.001/QUA). Still economically viable given low 4GB-RAM node operating costs (~$480/yr).

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

### 4.2 Fee Distribution (v3)

Each block's total transaction fees (`F`) are split in fixed proportions:

| Recipient | Percentage | Destination |
|---|---|---|
| **Burn (destroyed)** | **50%** | Unspendable — permanent deflation |
| **Block Proposer** | **35%** | BFT proposer's coinbase address |
| **Ecosystem Fund (QEF)** | **15%** | `ms69216b1d10425689704d5ae3b2a4aa17049f59b1` (3-of-5 multisig) |

> **Rounding**: `fee_burned + fee_to_proposer + fee_to_treasury = total_fees` — remainder goes to proposer.

**Example (1,000 AI agent transactions × 0.001 QUA each)**:
```
Total fees:     1,000,000 microunits (1 QUA)
Burned:           500,000 microunits (0.50 QUA) — destroyed forever
Proposer:         350,000 microunits (0.35 QUA) — validator income
QEF:              150,000 microunits (0.15 QUA) — ecosystem fund
```

### 4.3 Burn Mechanism

**Burn Implementation**: 50% of all fees are never credited to any spendable address. As AI agent transaction volume grows, the burn rate accelerates supply reduction.

**Break-even burn** (net-zero inflation in Year 1):
```
New emission Year 1:  263,800,000 QUA
Burn per tx (50%):    500 μQUA at 1,000 μQUA avg fee
TX needed to offset:  263,800,000 ÷ 0.000500 = ~527,600,000 tx/yr
                    = ~1.44 million transactions per day
```
Above ~1.44M tx/day the network becomes net-deflationary.

**Deflationary Effect Estimates (v3)**:

| Scenario | Annual TX | Avg Fee | Annual Burned | Annual Emission |
|---|---|---|---|---|
| Year 1, Low | 10M | 0.001 QUA | 5,000 QUA | 263,800,000 QUA |
| Year 5, Medium | 100M | 0.001 QUA | 50,000 QUA | 137,181,600 QUA |
| Year 10, High | 500M | 0.002 QUA | 500,000 QUA | 51,771,600 QUA |
| Year 15, Mature | 2B | 0.002 QUA | 2,000,000 QUA | 19,551,320 QUA |

At 2B tx/year (Year 15+), fee burning could **exceed new emission**, making QUANTA net deflationary.

### 4.4 QEF Accumulation from Fees

QEF receives 15% of all fees — sustainable, independent funding:

| Year | Est. Annual TX | Fee Revenue (15%) | Annual QEF Block Alloc (8%) | Total QEF Inflow |
|---|---|---|---|---|
| 1 | 10M | ~1,500 QUA | ~21,024,000 QUA | ~21,025,500 QUA |
| 5 | 100M | ~15,000 QUA | ~10,975,000 QUA | ~10,990,000 QUA |
| 10 | 500M | ~75,000 QUA | ~4,142,000 QUA | ~4,217,000 QUA |

---

## 5. Anti-Dump Mechanisms (DPoS Era)

### 5.1 Validator Staking Lock (Replaces PoW Mining Lock)

In DPoS+BFT, there is no mining reward lock. Anti-dump is achieved through the validator lifecycle:

| Lock Type | Mechanism | Duration |
|---|---|---|
| **Staking lock** | Validator's staked QUA is frozen while in the active committee | Entire validator lifetime |
| **Coinbase maturity** | Block rewards unspendable for 500 blocks after production | ~50 minutes |
| **Unbonding period** | After `Unstake` tx, staked QUA locked for 60 epochs | ~4.2 days |
| **QEF multisig** | Ecosystem fund behind 3-of-5 Falcon-512 threshold | Until governance vote to spend |

**Unbonding implementation** (`authorities.rs`):
```rust
pub const UNBONDING_EPOCHS: u64 = 60; // 60,000 blocks ≈ 4.2 days at 6s
```

This means a validator who wishes to exit cannot immediately dump their staked QUA — they must wait ~4.2 days from the `Unstake` transaction before the staked amount is released.

### 5.2 Circulating Supply Dynamics (v3)

| Phase | Total Emitted | Validator Staked | QEF Locked | Circulating |
|---|---|---|---|---|
| Day 1 | 0 QUA | ~21M QUA (genesis) | ~50M QUA | ~0 QUA |
| Week 1 | ~2.45M QUA | ~21M QUA | ~50M QUA | ~2.45M QUA |
| Month 1 | ~21.9M QUA | ~21M QUA | ~50M QUA | ~21.9M QUA |
| Year 1 | ~263.8M QUA | ~21M QUA | ~71M QUA | ~171.8M QUA |

Very low circulating supply at launch (validators' staked QUA locked, QEF multisig controlled) means minimal dump risk.

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
    current_height + 157_680  // ~54.75 days
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

**Document Version**: 3.0  
**Last Updated**: May 2026  
**Founder**: Kishore K (admin@quantachain.org)  
**License**: CC BY 4.0
