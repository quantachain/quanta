# QUANTA TOKENOMICS

**Comprehensive Economic Model Specification**

Version 1.0 - January 2026

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

---

## 1. Overview

### 1.1 Design Goals

The QUANTA economic model is designed to achieve:

1. **Long-term Sustainability**: Rewards that remain attractive for decades
2. **Fair Distribution**: No pre-mine, no ICO, 100% through mining
3. **Deflationary Pressure**: Fee burning creates supply reduction
4. **Early Adopter Incentives**: Reward risk-taking without excessive inequality
5. **Anti-Dump Protection**: Lock mechanisms prevent immediate sell pressure
6. **Development Funding**: Sustainable treasury for ongoing development

### 1.2 Key Parameters

| Parameter | Value | Rationale |
|-----------|-------|-----------|
| Initial Block Reward | 100 QUA | High enough to attract miners, low enough to prevent inflation |
| Annual Reduction | 15% | Gradual decay, not sudden halvings |
| Minimum Reward | 5 QUA | Ensures perpetual mining incentive |
| Block Time | 10 seconds | Fast finality, reasonable propagation time |
| Blocks Per Year | 3,153,600 | 365.25 days * 24 hours * 360 blocks/hour |
| Fee Burn Rate | 70% | Strong deflationary pressure |
| Treasury Allocation | 20% of fees | Sustainable development funding |
| Miner Fee Share | 10% of fees | Additional incentive beyond block reward |

---

## 2. Supply Schedule

### 2.1 Emission Formula

The base block reward follows exponential decay with a floor:

```python
def base_reward(year):
    initial_reward = 100_000_000  # 100 QUA in microunits
    reduction_rate = 0.85         # 15% annual reduction
    minimum_reward = 5_000_000    # 5 QUA floor
    
    reward = initial_reward * (reduction_rate ** year)
    return max(reward, minimum_reward)
```

### 2.2 Emission Schedule Table

| Year | Base Reward (QUA) | Blocks | Annual Emission | Cumulative Supply |
|------|-------------------|--------|-----------------|-------------------|
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

The total supply approaches but never reaches a hard cap:

```
Asymptotic Maximum ≈ 1.5 billion QUA (year 15-20)
True Maximum: Infinite (due to 5 QUA floor)
Practical Maximum: ~2 billion QUA (year 50)
```

This ensures:
- No "final Bitcoin" problem where mining stops
- Perpetual security budget
- Predictable long-term inflation (~0.8% annually after year 20)

---

## 3. Block Rewards

### 3.1 Standard Block Reward

Without modifiers, a block reward is simply:

```
Block Reward = base_reward(current_year)
```

### 3.2 Early Adopter Bonus

**Duration**: First 100,000 blocks (~11.5 days)

**Multiplier**: 1.5x

**Formula**:
```python
if block_height < 100_000:
    reward *= 1.5
```

**Rationale**:
- Attracts initial miners when network hashrate is low
- Short duration prevents excessive early holder advantage
- Creates excitement at launch without long-term distortion

**Total Extra Emission**: ~157,680,000 QUA (50% of first 11.5 days)

### 3.3 Network Usage Multiplier

**Duration**: First 315,360 blocks (~36 days, "bootstrap phase")

**Range**: 1.0x to 2.0x

**Formula**:
```python
def usage_multiplier(block_height):
    if block_height >= 315_360:
        return 1.0  # No multiplier after bootstrap
    
    # Analyze last 1000 blocks
    lookback = min(1000, block_height)
    recent_blocks = blocks[block_height - lookback : block_height]
    
    # Calculate total fees paid (economic activity indicator)
    total_fees = sum(sum(tx.fee for tx in block.transactions) 
                     for block in recent_blocks)
    
    # Normalize to expected minimum activity
    # Assume minimum 10 QUA fees per 1000 blocks
    expected_minimum = 10_000_000  # microunits
    
    # Multiplier scales with fee activity
    multiplier = 1.0 + min(1.0, total_fees / expected_minimum)
    
    return multiplier
```

**Rationale**:
- Rewards genuine network usage, not just mining
- Fee-based (not tx count) prevents spam attacks
- Caps at 2.0x to prevent runaway inflation
- Only during bootstrap when network needs incentive

**Attack Resistance**:
- Miners cannot profitably spam transactions (fees cost more than reward gain)
- Looking at 1000 blocks prevents single-block manipulation
- Fee burning makes sustained fake activity expensive

### 3.4 Combined Reward Calculation

Full block reward formula:
```python
def calculate_block_reward(block_height, block):
    year = block_height / 3_153_600
    reward = base_reward(year)
    
    # Early adopter bonus
    if block_height < 100_000:
        reward *= 1.5
    
    # Network usage multiplier
    if block_height < 315_360:
        reward *= usage_multiplier(block_height)
    
    return reward
```

**Example Rewards**:
- Block 1,000: 100 QUA × 1.5 (early bonus) × 1.2 (usage) = 180 QUA
- Block 150,000: 100 QUA × 1.3 (usage) = 130 QUA
- Block 400,000: ~98 QUA (year 1, no bonuses)
- Block 3,153,600: 85 QUA (year 2 base)
- Block 31,536,000: 19.69 QUA (year 10)

---

## 4. Fee Economics

### 4.1 Transaction Fees

**Minimum Fee**: 100 microunits (0.0001 QUA)

**Purpose**:
- Spam prevention
- Network prioritization
- Economic sustainability

**Fee Market**:
- Users can pay higher fees for priority inclusion
- Miners select highest-fee transactions first
- Mempool limits create natural fee market

### 4.2 Fee Distribution

Each block's transaction fees are split:

| Recipient | Percentage | Purpose |
|-----------|------------|---------|
| **Burn Address** | 70% | Permanent supply reduction |
| **Treasury** | 20% | Development funding |
| **Miner** | 10% | Additional validator reward |

**Example**:
```
Block has 1000 transactions, each paying 0.001 QUA fee
Total fees: 1 QUA = 1,000,000 microunits

Distribution:
- Burn: 700,000 microunits (0.7 QUA) - destroyed forever
- Treasury: 200,000 microunits (0.2 QUA) - development fund
- Miner: 100,000 microunits (0.1 QUA) - added to block reward
```

### 4.3 Burn Mechanism

**Implementation**:
```
Burn Address: "QUANTA_BURN_ADDRESS" (special constant)
```

Coins sent to burn address are:
- Tracked in total supply calculations
- Permanently unspendable (no private key exists)
- Visible on-chain for transparency

**Deflationary Effect**:

Year 1 estimates (conservative):
- 10 million transactions
- Average fee: 0.001 QUA
- Total fees: 10,000 QUA
- Burned: 7,000 QUA
- Net inflation: 315,360,000 - 7,000 = 315,353,000 QUA

Year 10 estimates (mature network):
- 100 million transactions
- Average fee: 0.005 QUA (higher due to value appreciation)
- Total fees: 500,000 QUA
- Burned: 350,000 QUA
- New emission: 62,094,634 QUA
- Net inflation rate: (62,094,634 - 350,000) / total_supply ≈ 4.3%

### 4.4 Treasury Accumulation

**Treasury Address**: Multisig controlled by core development team

**Transparency**: All treasury transactions publicly visible

**Usage**:
- Developer salaries
- Security audits
- Infrastructure costs
- Grants program
- Marketing and outreach

**Projected Treasury Growth**:
- Year 1: ~2,000 QUA
- Year 5: ~50,000 QUA
- Year 10: ~200,000 QUA

---

## 5. Anti-Dump Mechanisms

### 5.1 Mining Reward Lock

**Percentage Locked**: 50% of mining rewards

**Lock Duration**: 157,680 blocks (~6 months at 10s blocks)

**Mechanism**:
```python
block_reward = calculate_block_reward(height)
immediate_reward = block_reward * 0.5
locked_reward = block_reward * 0.5

miner_receives_now = immediate_reward + (10% of fees)
miner_locked_until = height + 157_680
```

**Implementation**:
```rust
struct AccountState {
    balance: u64,              // Immediately spendable
    locked_balance: u64,       // Time-locked
    lock_release_height: u64,  // Block when lock expires
}
```

**Unlock Behavior**:
- Locked balance becomes spendable at release height
- Can be tracked per mining event (multiple locks possible)
- Wallet displays both available and locked balances

**Rationale**:
- Prevents miners from immediately dumping rewards
- Aligns incentives: miners benefit from long-term price stability
- 6-month lock is long enough to matter, short enough to be acceptable
- 50% lock preserves operational liquidity for miners

### 5.2 Economic Impact

**Circulating Supply vs Total Supply**:

| Time | Total Mined | Circulating (Unlocked) | Locked | Burned |
|------|-------------|------------------------|--------|--------|
| Day 30 | ~270 million QUA | ~135 million QUA | ~135 million QUA | ~100 QUA |
| Day 180 | ~1.1 billion QUA | ~825 million QUA | ~275 million QUA | ~10,000 QUA |
| Year 1 | ~1.2 billion QUA | ~1.05 billion QUA | ~150 million QUA | ~50,000 QUA |

**Effective Inflation Control**:
- First 6 months: 50% of supply locked
- Reduces immediate sell pressure
- Stabilizes price during critical launch period

---

## 6. Treasury Model

### 6.1 Treasury Accumulation

**Source**: 20% of all transaction fees

**Control**: 3-of-5 multisig (core team + community representatives)

**Transparency**: All transactions visible on-chain

### 6.2 Allocation Guidelines

Recommended distribution:
- **40%**: Core development (salaries, infrastructure)
- **25%**: Security (audits, bug bounties)
- **20%**: Ecosystem grants (DApps, tools, integrations)
- **10%**: Marketing and community
- **5%**: Reserve/emergency fund

### 6.3 Governance

**Current Model** (Year 1):
- Core team proposals
- Community feedback
- Quarterly transparency reports

**Future Model** (Year 2+):
- On-chain treasury voting
- Token-weighted voting on proposals
- Time-locked spending with veto period

---

## 7. Economic Security

### 7.1 51% Attack Cost

**Cost to attack**:
```
Attack Cost = (Network Hashrate * Attack Duration * Energy Cost) + Equipment Cost
```

Year 1 estimates:
- Network hashrate: ~10 TH/s (assumed)
- Equipment: $1,000,000 (ASIC miners)
- Energy: $50,000/hour
- 1-hour attack cost: ~$1,050,000

**Defense**:
- Checkpoint system prevents deep reorgs
- High block reward attracts honest miners
- Social layer: exchanges require many confirmations

### 7.2 Miner Profitability

**Revenue Sources**:
1. Block reward (declining over time)
2. 10% of transaction fees (growing over time)
3. Locked rewards (vest over 6 months)

**Break-even Analysis** (Year 1):
```
Block reward: 100 QUA
Estimated QUA price: $0.10 (conservative)
Block value: $10

Average fee per block: 0.1 QUA
Miner fee share: 0.01 QUA
Additional fee revenue: $0.001

Total revenue per block: $10.001

Cost per block (energy): ~$5
Profit per block: ~$5
Daily profit (8,640 blocks): ~$43,200
```

Remains profitable as long as:
```
(Block Reward + Fee Share) * QUA Price > Mining Cost
```

### 7.3 Fee Market Dynamics

**Low Activity** (Early network):
- Minimum fees only (0.0001 QUA)
- All transactions included
- No priority bidding needed

**High Activity** (Mature network):
- Mempool fills up (5,000 tx limit)
- Users bid higher fees for inclusion
- Fee market emerges naturally
- Miner revenue shifts from block reward to fees (Bitcoin-like transition)

---

## 8. Simulation Results

### 8.1 Supply Growth Projection

**Total Supply Over Time**:
```
Year 1:  315.4 million QUA
Year 2:  583.4 million QUA
Year 3:  811.3 million QUA
Year 5:  1,169.6 million QUA
Year 10: 1,417.6 million QUA
Year 20: 1,503.8 million QUA
Year 50: 1,977.1 million QUA
```

**Inflation Rate**:
```
Year 1:  N/A (genesis)
Year 2:  84.9% (high early growth)
Year 3:  39.0%
Year 5:  14.1%
Year 10: 4.4%
Year 20: 1.0%
Year 50: 0.8%
```

Approaches Bitcoin-like ~2% inflation, but never reaches zero (perpetual security budget).

### 8.2 Fee Burn Impact

**Conservative Scenario** (Low activity):
- Year 5: 50,000 QUA burned
- Year 10: 200,000 QUA burned
- Year 20: 1,000,000 QUA burned

**Optimistic Scenario** (High activity):
- Year 5: 500,000 QUA burned
- Year 10: 5,000,000 QUA burned
- Year 20: 50,000,000 QUA burned

At high adoption, fee burning could make QUANTA deflationary by year 15-20.

### 8.3 Circulating Supply vs Locked Supply

**Month 1**: 50% circulating, 50% locked
**Month 6**: 75% circulating, 25% locked (first locks expire)
**Month 12**: 87.5% circulating, 12.5% locked (steady state)

After first 6 months, locked supply stabilizes at ~12-13% of total supply (6-month rolling lock).

---

## 9. Comparison with Other Chains

### 9.1 vs Bitcoin

| Feature | Bitcoin | QUANTA |
|---------|---------|--------|
| Initial Reward | 50 BTC | 100 QUA |
| Reduction Method | 50% halving every 4 years | 15% decay annually |
| Final Supply | 21 million (hard cap) | ~1.5 billion (soft cap, 5 QUA floor) |
| Security Budget | Ends ~2140 | Perpetual (5 QUA/block minimum) |
| Fee Burning | None | 70% of fees |
| Pre-mine | None | None |

**Advantages**:
- Smoother emission curve (no halving shocks)
- Perpetual mining incentive
- Deflationary pressure through burn

**Tradeoffs**:
- Higher initial inflation
- Larger total supply

### 9.2 vs Ethereum

| Feature | Ethereum (PoS) | QUANTA |
|---------|----------------|--------|
| Consensus | Proof-of-Stake | Proof-of-Work |
| Issuance | ~0.5% annual | 15% → 1% over 20 years |
| Fee Burning | EIP-1559 (variable) | 70% (fixed) |
| Staking Lock | Variable | 50% of mining rewards |
| Initial Distribution | ICO + pre-mine | Fair launch mining |

**Advantages**:
- No pre-mine
- More aggressive burning
- Simpler economic model

**Tradeoffs**:
- Higher energy cost (PoW)
- Slower initial distribution

### 9.3 vs Monero

| Feature | Monero | QUANTA |
|---------|--------|--------|
| Initial Emission | Fast (18.4M in 4 years) | Gradual (1.5B over 15 years) |
| Tail Emission | 0.6 XMR/block | 5 QUA/block |
| Privacy | Native | Planned (future) |
| Quantum Resistance | None | Full (PQC) |

**Advantages**:
- Quantum-resistant today
- Lower tail inflation rate

**Tradeoffs**:
- Slower initial distribution
- No native privacy (yet)

---

## 10. Future Economic Considerations

### 10.1 Transition to Fee-Based Security

As block rewards decline, network security must transition to fee-based model:

**Year 10**:
- Block reward: ~20 QUA
- Expected fees: ~5 QUA (10% to miner = 0.5 QUA)
- Total miner revenue: 20.5 QUA

**Year 20**:
- Block reward: 5 QUA (floor)
- Expected fees: ~50 QUA (10% to miner = 5 QUA)
- Total miner revenue: 10 QUA

By year 20, half of miner revenue comes from fees (Bitcoin is targeting this by 2140).

### 10.2 Potential Adjustments

If network conditions change significantly, governance may propose:

**Fee Structure Changes**:
- Adjust burn rate (70% → 60-80%)
- Modify treasury allocation
- Implement fee tiers

**Reward Schedule Changes**:
- Adjust floor (5 QUA → higher if needed)
- Modify decay rate (15% → faster/slower)

**Lock Mechanism Changes**:
- Adjust lock percentage (50% → 30-70%)
- Modify lock duration (6 months → 3-12 months)

All changes require hard fork and community consensus.

---

## 11. Economic Attack Vectors

### 11.1 Fee Market Manipulation

**Attack**: Miner includes own zero-fee transactions to manipulate usage multiplier

**Mitigation**: 
- Fees are burned (attacker pays 70% cost)
- 1000-block lookback averages out single-block manipulation
- Only affects bootstrap phase (first 36 days)

**Cost**: Spending 100 QUA in fees gains at most 100 QUA in bonus (net zero)

### 11.2 Selfish Mining

**Attack**: Miner withholds blocks to gain advantage

**Mitigation**:
- Standard PoW mitigations apply
- Fast block time (10s) reduces impact
- Checkpoint system prevents deep reorgs

### 11.3 Long-Range Attack

**Attack**: Rewrite chain from genesis with quantum computer in future

**Mitigation**:
- Quantum-resistant signatures prevent forgery
- Checkpoints embedded in software
- Social consensus (exchanges/wallets reject fake chains)

---

## Appendix A: Economic Formulas

### Block Reward Calculation
```python
def calculate_block_reward(height):
    blocks_per_year = 3_153_600
    year = height / blocks_per_year
    
    # Base reward
    base = 100_000_000 * (0.85 ** year)
    base = max(base, 5_000_000)
    
    # Bonuses
    if height < 100_000:
        base *= 1.5
    if height < 315_360:
        base *= usage_multiplier(height)
    
    return base

def usage_multiplier(height):
    lookback = min(1000, height)
    blocks = get_blocks(height - lookback, height)
    total_fees = sum(sum(tx.fee for tx in b.transactions) for b in blocks)
    
    expected = 10_000_000
    multiplier = 1.0 + min(1.0, total_fees / expected)
    return multiplier
```

### Fee Distribution
```python
def distribute_fees(total_fees):
    burn_amount = total_fees * 0.70
    treasury_amount = total_fees * 0.20
    miner_amount = total_fees * 0.10
    
    burn(burn_amount)
    send_to_treasury(treasury_amount)
    return miner_amount  # Added to miner's reward
```

### Lock Mechanism
```python
def process_mining_reward(miner_address, reward, height):
    immediate = reward * 0.5
    locked = reward * 0.5
    
    add_balance(miner_address, immediate)
    add_locked_balance(miner_address, locked, height + 157_680)
```

---

## Appendix B: Treasury Multisig

**Current Signers** (3-of-5):
1. Core Developer 1
2. Core Developer 2
3. Core Developer 3
4. Community Representative 1
5. Community Representative 2

**Signing Policy**:
- Routine expenses (<10,000 QUA): Any 3 signers
- Major expenses (>10,000 QUA): All 5 signers + public announcement
- Emergency expenses: 3 signers + post-facto disclosure

---

**Document Version**: 1.0  
**Last Updated**: January 6, 2026  
**License**: CC BY 4.0
