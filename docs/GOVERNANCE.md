# QUANTA Governance

**Founder**: Kishore K — [admin@quantachain.org](mailto:admin@quantachain.org)

This document defines how protocol upgrades, treasury spending, and consensus changes are managed in QUANTA.

---

## 1. Current Governance Model (Phase 1 — Founder-Led)

QUANTA is in its early phase. Governance is currently exercised by the founding team through:

- **Treasury multisig** (3-of-5, Falcon-512) for all fund movements
- **Code-based consensus** for protocol rules (no on-chain voting yet)
- **Open GitHub** for proposals, bug reports, and community input

This is deliberate: immature on-chain governance creates attack surfaces. We evolve to decentralized governance as the network matures.

---

## 2. Treasury Multisig

### Policy

| Parameter | Value |
|---|---|
| Type | 3-of-5 Falcon-512 multisig |
| Address | `ms69216b1d10425689704d5ae3b2a4aa17049f59b1` |
| Threshold | Any 3 of 5 keyholders must sign |
| Enforcement | Hardcoded in `src/consensus/blockchain.rs` — consensus-critical |
| Generated | 2026-03-14 |

The treasury address is a **consensus constant**, not a config value. A node that changes it mines blocks the network will reject.

### Where Treasury Funds Come From

Every block automatically credits the treasury:

```
Fee income:      20% of all transaction fees in the block
Block reward:     5% of the block reward
```

At Year 1 rates (100 QUA reward, typical fees):
- ~5 QUA per block from reward allocation
- plus fee share (~0.5–2 QUA per busy block)
- **~7–10 QUA per block to treasury** (~$630/day at $1/QUA, ~$2.3M/year)

### How to Spend Treasury Funds

All spending requires 3-of-5 signatures using the `quanta-wallet` CLI:

```bash
# 1. Any keyholder creates an unsigned proposal
quanta-wallet treasury-propose \
  --setup treasury_keys/treasury_setup.json \
  --to 0xRecipientAddress \
  --amount 1000 \
  --nonce 1 \
  --out proposal.json

# 2. Three separate keyholders sign (each on their own machine)
quanta-wallet treasury-sign --proposal proposal.json --key treasury_keys/treasury_key0.qua --index 0
quanta-wallet treasury-sign --proposal proposal.json --key treasury_keys/treasury_key1.qua --index 1
quanta-wallet treasury-sign --proposal proposal.json --key treasury_keys/treasury_key2.qua --index 2

# 3. Anyone broadcasts the fully-signed transaction
quanta-wallet treasury-broadcast --proposal proposal.json --node http://localhost:3000
```

### Key Security Rules

- Keys are stored in **separate physical locations** (different machines / USB drives)
- Never store more than 2 keys on the same machine in production
- Key files (`treasury_key*.qua`) are Kyber-1024 + Argon2id encrypted
- `treasury_setup.json` contains only **public keys** — safe to back up to cloud
- The private key files must never be committed to version control (`.gitignore` enforces this)

### Approved Treasury Uses

| Category | Description | Approval needed |
|---|---|---|
| Infrastructure | Servers, DNS, bootstrap nodes | 3-of-5 multisig |
| Security audits | External code audits, pentests | 3-of-5 multisig |
| Developer grants | Bounties, core contributor rewards | 3-of-5 multisig + public announcement |
| Exchange listings | Listing fees, market making | 3-of-5 multisig + community notice |
| Marketing | Events, content, community | 3-of-5 multisig |

All treasury transactions are **publicly visible on-chain**. No private spending.

---

## 3. Protocol Upgrade Process

### How Rules Change

Currently, protocol changes are made by the founding team following:

1. **GitHub Issue / Discussion** — propose change, gather feedback (minimum 7 days open)
2. **Implementation PR** — code submitted with tests and documentation
3. **Review** — at least one additional reviewer; security-critical changes require 2 reviewers
4. **Testnet deployment** — minimum 14 days on testnet before mainnet consideration
5. **Mainnet upgrade** — coordinated node upgrade with advance notice (minimum 30 days for consensus changes)

### Consensus-Critical vs. Non-Critical Changes

| Type | Examples | Process |
|---|---|---|
| **Consensus-critical** | Treasury address, block reward, TREASURY_ADDRESS, genesis hash | Hard fork — all nodes must upgrade simultaneously |
| **Node-local** | Log verbosity, pruning mode, cache sizes | Soft change — each node can update independently |
| **Config-driven** | `quanta.toml` parameters | Backward-compatible — no upgrade coordination needed |

---

## 4. PoW → PoS Transition (Planned)

### Why PoS in the Future?

Proof-of-Work provides ideal fair-launch properties: no pre-existing token distribution, proven Sybil resistance. However, long-term QUANTA plans to transition to Proof-of-Stake for:

- **Energy efficiency**: PoS uses ~99.9% less energy than PoW
- **Faster finality**: BFT-style PoS can achieve 2-3 second finality
- **Validator decentralization**: Lower barrier to participation than mining hardware

### Transition Architecture (Design Phase)

The codebase already has the infrastructure stub:

```toml
# quanta.toml — future activation
consensus_engine = "proof_of_stake"  # currently: node refuses to start with clear error
```

```rust
// src/config/types.rs
pub enum ConsensusEngine {
    ProofOfWork,    // live
    ProofOfStake,   // planned — stub exists, implementation in Phase 6
}
```

### Transition Phases

| Phase | Timeline | Description |
|---|---|---|
| **PoW only** | Now → Q4 2026 | Current state — fair launch, proven security |
| **PoW + PoS hybrid** | Q1 2027 (tentative) | Miners + validators both produce blocks; test staking |
| **PoS primary** | Q3 2027 (tentative) | PoS produces majority of blocks, PoW as fallback |
| **PoW sunset** | Q1 2028 (tentative) | Pure PoS once staking is battle-tested |

> **Note**: All timeline estimates are subject to security audit results and community readiness. No transition will happen without extensive testnet validation and community notice.

### Validator Requirements (PoS — Planned)

| Parameter | Target Value |
|---|---|
| Minimum stake | TBD (community vote) |
| Slashing condition | Double-signing, downtime |
| Unbonding period | 21 days |
| Signature scheme | Falcon-512 (post-quantum) |
| Validator key | Separate from spending key |

---

## 5. Emergency Procedures

### Consensus Bug / Critical Vulnerability

If a critical vulnerability is discovered:

1. **Immediate**: Private disclosure to [admin@quantachain.org](mailto:admin@quantachain.org)
2. **Within 24h**: Patch developed and tested
3. **Within 48h**: Emergency upgrade notice to all known node operators
4. **Within 72h**: Patched binary released with migration guide

See [SECURITY.md](SECURITY.md) for full vulnerability reporting procedures.

### Treasury Key Compromise

If one treasury key is believed compromised:

1. Immediately use the remaining 4 keys to sign a transaction moving ALL treasury funds to a new 3-of-5 multisig
2. Generate a new `treasury-init --signers 5` setup
3. Update `TREASURY_ADDRESS` in `blockchain.rs` and coordinate a network upgrade
4. Publicly disclose the incident

The 3-of-5 threshold means **one compromised key cannot move funds** — an attacker needs 3.

---

## 6. Roadmap to Decentralized Governance

| Phase | Governance Model |
|---|---|
| **Phase 1–2 (Now)** | Founder-led, 3-of-5 treasury multisig, open GitHub |
| **Phase 3 (Q3 2026)** | Community multisig: expand to 5-of-9, include external trusted contributors |
| **Phase 4 (Q1 2027)** | On-chain signaling: token-weighted "soft votes" for protocol parameter changes |
| **Phase 5 (2028+)** | Full on-chain governance: binding validator votes for protocol upgrades |

---

*Last updated: 2026-03-14 — Kishore K, Founder*
