// ---------------------------------------------------------------------------
// Copyright (c) 2026 QuantaLabs Pvt Ltd. All Rights Reserved.
//
// Quanta Native AI Contract Layer v3
// ====================================
// 5 production templates for PQC-native M2M and AI agent economies:
//   1 = TEMPLATE_ESCROW         -- HTLC hash-time locked escrow (+ refund)
//   2 = TEMPLATE_AGENT_JOB      -- Single-worker AI job (+ deadline + refund)
//   3 = TEMPLATE_AGENT_BID      -- Multi-agent auction: employer picks best result
//   4 = TEMPLATE_STREAM         -- Streaming payment (pay-per-block subscription)
//   5 = TEMPLATE_AGENT_REGISTRY -- On-chain AI service discovery registry
// ---------------------------------------------------------------------------

use crate::core::transaction::{AccountState, ContractEvent, ContractState, Transaction};
use serde::{Deserialize, Serialize};
use sha3::{Digest, Sha3_256};
use std::collections::HashMap;

pub const TEMPLATE_ESCROW: u8 = 1;
pub const TEMPLATE_AGENT_JOB: u8 = 2;
pub const TEMPLATE_AGENT_BID: u8 = 3;
pub const TEMPLATE_STREAM: u8 = 4;
pub const TEMPLATE_AGENT_REGISTRY: u8 = 5;

#[derive(Serialize, Deserialize)]
pub struct EscrowInitArgs {
    pub beneficiary: String,
    pub secret_hash: String,
    pub refund_height: u64,
}
#[derive(Serialize, Deserialize)]
pub struct EscrowClaimArgs {
    pub preimage: String,
}
#[derive(Serialize, Deserialize)]
pub struct AgentJobInitArgs {
    pub worker: String,
    pub task_hash: String,
    pub deadline_height: u64,
}
#[derive(Serialize, Deserialize)]
pub struct AgentJobClaimArgs {
    pub result_hash: String,
}
#[derive(Serialize, Deserialize)]
pub struct AgentBidInitArgs {
    pub task_hash: String,
    pub bid_close_height: u64,
    pub refund_height: u64,
}
#[derive(Serialize, Deserialize)]
pub struct AgentBidSubmitArgs {
    pub result_hash: String,
    pub price: u64,
}
#[derive(Serialize, Deserialize)]
pub struct AgentBidSelectArgs {
    pub winner: String,
}
#[derive(Serialize, Deserialize)]
pub struct StreamInitArgs {
    pub recipient: String,
    pub rate_per_block: u64,
}
#[derive(Serialize, Deserialize)]
pub struct AgentRegisterArgs {
    pub agent_address: String,
    pub name: String,
    pub endpoint_hash: String,
    pub service_type: String,
    pub price_per_call: u64,
}
#[derive(Serialize, Deserialize)]
pub struct AgentUpdateArgs {
    pub endpoint_hash: Option<String>,
    pub price_per_call: Option<u64>,
    pub active: Option<bool>,
}

fn emit(contract: &mut ContractState, height: u64, name: &str, data: Vec<(&str, String)>) {
    let map = data.into_iter().map(|(k, v)| (k.to_string(), v)).collect();
    contract.events.push(ContractEvent {
        height,
        name: name.to_string(),
        data: map,
    });
}

pub struct NativeContracts;

impl NativeContracts {
    pub fn execute(state: &mut AccountState, tx: &Transaction, current_height: u64) {
        match &tx.tx_type {
            crate::core::transaction::TransactionType::ContractDeploy {
                template_id,
                init_args,
            } => {
                let tx_hash = tx.hash();
                if let Err(e) =
                    Self::deploy(state, tx, &tx_hash, *template_id, init_args, current_height)
                {
                    tracing::warn!("Contract deploy failed: {}", e);
                }
            }
            crate::core::transaction::TransactionType::ContractCall {
                contract_address,
                method,
                call_args,
            } => {
                if let Err(e) = Self::call(
                    state,
                    tx,
                    contract_address,
                    method,
                    call_args,
                    current_height,
                ) {
                    tracing::warn!("Contract call failed: {}", e);
                }
            }
            _ => {}
        }
    }

    pub fn generate_address(tx_hash: &str) -> String {
        format!("0xc_{}", &tx_hash[0..36])
    }

    pub fn deploy(
        state: &mut AccountState,
        tx: &Transaction,
        tx_hash: &str,
        template_id: u8,
        init_args: &[u8],
        current_height: u64,
    ) -> Result<String, String> {
        let addr = Self::generate_address(tx_hash);
        let mut s: HashMap<String, String> = HashMap::new();
        match template_id {
            TEMPLATE_ESCROW => {
                let a: EscrowInitArgs =
                    serde_json::from_slice(init_args).map_err(|e| e.to_string())?;
                s.insert("beneficiary".into(), a.beneficiary);
                s.insert("secret_hash".into(), a.secret_hash);
                s.insert("refund_height".into(), a.refund_height.to_string());
                s.insert("status".into(), "locked".into());
            }
            TEMPLATE_AGENT_JOB => {
                let a: AgentJobInitArgs =
                    serde_json::from_slice(init_args).map_err(|e| e.to_string())?;
                if a.deadline_height <= current_height {
                    return Err("deadline must be future".into());
                }
                s.insert("worker".into(), a.worker);
                s.insert("task_hash".into(), a.task_hash);
                s.insert("deadline_height".into(), a.deadline_height.to_string());
                s.insert("status".into(), "open".into());
            }
            TEMPLATE_AGENT_BID => {
                let a: AgentBidInitArgs =
                    serde_json::from_slice(init_args).map_err(|e| e.to_string())?;
                if a.bid_close_height <= current_height {
                    return Err("bid_close_height must be future".into());
                }
                s.insert("task_hash".into(), a.task_hash);
                s.insert("bid_close_height".into(), a.bid_close_height.to_string());
                s.insert("refund_height".into(), a.refund_height.to_string());
                s.insert("status".into(), "open".into());
                s.insert("bid_count".into(), "0".into());
            }
            TEMPLATE_STREAM => {
                let a: StreamInitArgs =
                    serde_json::from_slice(init_args).map_err(|e| e.to_string())?;
                if a.rate_per_block == 0 {
                    return Err("rate_per_block must be > 0".into());
                }
                s.insert("recipient".into(), a.recipient);
                s.insert("rate_per_block".into(), a.rate_per_block.to_string());
                s.insert("last_withdraw_height".into(), current_height.to_string());
                s.insert("status".into(), "active".into());
            }
            TEMPLATE_AGENT_REGISTRY => {
                let a: AgentRegisterArgs =
                    serde_json::from_slice(init_args).map_err(|e| e.to_string())?;
                s.insert("agent_address".into(), a.agent_address);
                s.insert("name".into(), a.name);
                s.insert("endpoint_hash".into(), a.endpoint_hash);
                s.insert("service_type".into(), a.service_type);
                s.insert("price_per_call".into(), a.price_per_call.to_string());
                s.insert("active".into(), "true".into());
                s.insert("registered_at".into(), current_height.to_string());
            }
            _ => return Err(format!("Unknown template_id: {}", template_id)),
        }
        let mut cs = ContractState {
            owner: tx.sender.clone(),
            template_id,
            deployed_at: current_height,
            storage: s,
            events: Vec::new(),
        };
        emit(
            &mut cs,
            current_height,
            "Deployed",
            vec![
                ("deployer", tx.sender.clone()),
                ("template_id", template_id.to_string()),
            ],
        );
        state.contracts.insert(addr.clone(), cs);
        Ok(addr)
    }

    pub fn call(
        state: &mut AccountState,
        tx: &Transaction,
        contract_address: &str,
        method: &str,
        call_args: &[u8],
        current_height: u64,
    ) -> Result<(), String> {
        let mut c = state
            .contracts
            .get(contract_address)
            .ok_or("Contract not found")?
            .clone();
        match c.template_id {
            TEMPLATE_ESCROW => Self::escrow(
                state,
                tx,
                &mut c,
                method,
                call_args,
                current_height,
                contract_address,
            )?,
            TEMPLATE_AGENT_JOB => Self::agent_job(
                state,
                tx,
                &mut c,
                method,
                call_args,
                current_height,
                contract_address,
            )?,
            TEMPLATE_AGENT_BID => Self::agent_bid(
                state,
                tx,
                &mut c,
                method,
                call_args,
                current_height,
                contract_address,
            )?,
            TEMPLATE_STREAM => {
                Self::stream(state, tx, &mut c, method, current_height, contract_address)?
            }
            TEMPLATE_AGENT_REGISTRY => {
                Self::agent_registry(state, tx, &mut c, method, call_args, current_height)?
            }
            _ => return Err("Unknown template_id in call".into()),
        }
        state.contracts.insert(contract_address.to_string(), c);
        Ok(())
    }

    // -- Escrow -----------------------------------------------------------------
    fn escrow(
        state: &mut AccountState,
        tx: &Transaction,
        c: &mut ContractState,
        method: &str,
        call_args: &[u8],
        h: u64,
        addr: &str,
    ) -> Result<(), String> {
        match method {
            "claim" => {
                if c.storage.get("status").map(|s| s.as_str()) != Some("locked") {
                    return Err("Not locked".into());
                }
                let a: EscrowClaimArgs =
                    serde_json::from_slice(call_args).map_err(|e| e.to_string())?;
                let pb = hex::decode(&a.preimage).map_err(|_| "Bad hex")?;
                let mut hasher = Sha3_256::new();
                hasher.update(&pb);
                let ch = hex::encode(hasher.finalize());
                if &ch != c.storage.get("secret_hash").ok_or("No hash")? {
                    return Err("Bad preimage".into());
                }
                let ben = c
                    .storage
                    .get("beneficiary")
                    .ok_or("No beneficiary")?
                    .clone();
                let amt = state.get_balance(addr);
                if amt > 0 {
                    state.debit_account(addr, amt);
                    state.credit_account_direct(&ben, amt);
                }
                c.storage.insert("status".into(), "claimed".into());
                emit(
                    c,
                    h,
                    "EscrowClaimed",
                    vec![("beneficiary", ben), ("amount", amt.to_string())],
                );
            }
            "refund" => {
                if c.storage.get("status").map(|s| s.as_str()) != Some("locked") {
                    return Err("Not locked".into());
                }
                if tx.sender != c.owner {
                    return Err("Not owner".into());
                }
                let rh: u64 = c
                    .storage
                    .get("refund_height")
                    .and_then(|h| h.parse().ok())
                    .unwrap_or(0);
                if h < rh {
                    return Err(format!("Refund after block {}", rh));
                }
                let amt = state.get_balance(addr);
                if amt > 0 {
                    state.debit_account(addr, amt);
                    state.credit_account_direct(&c.owner.clone(), amt);
                }
                c.storage.insert("status".into(), "refunded".into());
                emit(
                    c,
                    h,
                    "EscrowRefunded",
                    vec![("owner", c.owner.clone()), ("amount", amt.to_string())],
                );
            }
            _ => return Err(format!("Unknown Escrow method: {}", method)),
        }
        Ok(())
    }

    // -- AgentJob ---------------------------------------------------------------
    fn agent_job(
        state: &mut AccountState,
        tx: &Transaction,
        c: &mut ContractState,
        method: &str,
        call_args: &[u8],
        h: u64,
        addr: &str,
    ) -> Result<(), String> {
        match method {
            "claim" => {
                if c.storage.get("status").map(|s| s.as_str()) != Some("open") {
                    return Err("Not open".into());
                }
                let dl: u64 = c
                    .storage
                    .get("deadline_height")
                    .and_then(|d| d.parse().ok())
                    .unwrap_or(0);
                if h > dl {
                    return Err(format!("Deadline passed at block {}", dl));
                }
                let worker = c.storage.get("worker").ok_or("No worker")?.clone();
                if tx.sender != worker {
                    return Err("Not the worker".into());
                }
                let a: AgentJobClaimArgs =
                    serde_json::from_slice(call_args).map_err(|e| e.to_string())?;
                let amt = state.get_balance(addr);
                if amt > 0 {
                    state.debit_account(addr, amt);
                    state.credit_account_direct(&worker, amt);
                }
                c.storage
                    .insert("result_hash".into(), a.result_hash.clone());
                c.storage.insert("status".into(), "claimed".into());
                emit(
                    c,
                    h,
                    "AgentJobClaimed",
                    vec![
                        ("worker", worker),
                        ("result_hash", a.result_hash),
                        ("amount", amt.to_string()),
                    ],
                );
            }
            "refund" => {
                if c.storage.get("status").map(|s| s.as_str()) != Some("open") {
                    return Err("Not open".into());
                }
                if tx.sender != c.owner {
                    return Err("Not owner".into());
                }
                let dl: u64 = c
                    .storage
                    .get("deadline_height")
                    .and_then(|d| d.parse().ok())
                    .unwrap_or(u64::MAX);
                if h <= dl {
                    return Err(format!("Deadline not yet passed (block {})", dl));
                }
                let amt = state.get_balance(addr);
                if amt > 0 {
                    state.debit_account(addr, amt);
                    state.credit_account_direct(&c.owner.clone(), amt);
                }
                c.storage.insert("status".into(), "refunded".into());
                emit(
                    c,
                    h,
                    "AgentJobRefunded",
                    vec![("owner", c.owner.clone()), ("amount", amt.to_string())],
                );
            }
            _ => return Err(format!("Unknown AgentJob method: {}", method)),
        }
        Ok(())
    }

    // -- AgentBid ---------------------------------------------------------------
    fn agent_bid(
        state: &mut AccountState,
        tx: &Transaction,
        c: &mut ContractState,
        method: &str,
        call_args: &[u8],
        h: u64,
        addr: &str,
    ) -> Result<(), String> {
        match method {
            "submit_bid" => {
                if c.storage.get("status").map(|s| s.as_str()) != Some("open") {
                    return Err("Not open".into());
                }
                let close: u64 = c
                    .storage
                    .get("bid_close_height")
                    .and_then(|v| v.parse().ok())
                    .unwrap_or(0);
                if h > close {
                    return Err("Bidding closed".into());
                }
                let a: AgentBidSubmitArgs =
                    serde_json::from_slice(call_args).map_err(|e| e.to_string())?;
                let bal = state.get_balance(addr);
                if a.price > bal {
                    return Err(format!("Price {} > balance {}", a.price, bal));
                }
                let n: u64 = c
                    .storage
                    .get("bid_count")
                    .and_then(|v| v.parse().ok())
                    .unwrap_or(0);
                c.storage
                    .insert(format!("bid_{}_addr", n), tx.sender.clone());
                c.storage
                    .insert(format!("bid_{}_result", n), a.result_hash.clone());
                c.storage
                    .insert(format!("bid_{}_price", n), a.price.to_string());
                c.storage.insert("bid_count".into(), (n + 1).to_string());
                emit(
                    c,
                    h,
                    "BidSubmitted",
                    vec![
                        ("bidder", tx.sender.clone()),
                        ("result_hash", a.result_hash),
                        ("price", a.price.to_string()),
                    ],
                );
            }
            "select_winner" => {
                if c.storage.get("status").map(|s| s.as_str()) != Some("open") {
                    return Err("Not open".into());
                }
                if tx.sender != c.owner {
                    return Err("Not employer".into());
                }
                let a: AgentBidSelectArgs =
                    serde_json::from_slice(call_args).map_err(|e| e.to_string())?;
                let n: u64 = c
                    .storage
                    .get("bid_count")
                    .and_then(|v| v.parse().ok())
                    .unwrap_or(0);
                let mut price = None;
                let mut result = None;
                for i in 0..n {
                    if c.storage
                        .get(&format!("bid_{}_addr", i))
                        .map(|s| s.as_str())
                        == Some(&a.winner)
                    {
                        price = c
                            .storage
                            .get(&format!("bid_{}_price", i))
                            .and_then(|v| v.parse().ok());
                        result = c.storage.get(&format!("bid_{}_result", i)).cloned();
                        break;
                    }
                }
                let p = price.ok_or("Winner not in bids")?;
                let r = result.unwrap_or_default();
                let bal = state.get_balance(addr);
                if p > 0 {
                    state.debit_account(addr, p);
                    state.credit_account_direct(&a.winner, p);
                }
                let rem = bal.saturating_sub(p);
                if rem > 0 {
                    state.debit_account(addr, rem);
                    state.credit_account_direct(&c.owner.clone(), rem);
                }
                c.storage.insert("winner".into(), a.winner.clone());
                c.storage.insert("result_hash".into(), r.clone());
                c.storage.insert("status".into(), "settled".into());
                emit(
                    c,
                    h,
                    "AuctionSettled",
                    vec![
                        ("winner", a.winner),
                        ("price", p.to_string()),
                        ("result_hash", r),
                    ],
                );
            }
            "refund" => {
                if c.storage.get("status").map(|s| s.as_str()) != Some("open") {
                    return Err("Not open".into());
                }
                if tx.sender != c.owner {
                    return Err("Not employer".into());
                }
                let rh: u64 = c
                    .storage
                    .get("refund_height")
                    .and_then(|v| v.parse().ok())
                    .unwrap_or(u64::MAX);
                if h < rh {
                    return Err(format!("Refund after block {}", rh));
                }
                let amt = state.get_balance(addr);
                if amt > 0 {
                    state.debit_account(addr, amt);
                    state.credit_account_direct(&c.owner.clone(), amt);
                }
                c.storage.insert("status".into(), "refunded".into());
                emit(
                    c,
                    h,
                    "AuctionRefunded",
                    vec![("owner", c.owner.clone()), ("amount", amt.to_string())],
                );
            }
            _ => return Err(format!("Unknown AgentBid method: {}", method)),
        }
        Ok(())
    }

    // -- Stream -----------------------------------------------------------------
    fn stream(
        state: &mut AccountState,
        tx: &Transaction,
        c: &mut ContractState,
        method: &str,
        h: u64,
        addr: &str,
    ) -> Result<(), String> {
        match method {
            "withdraw" => {
                let recip = c.storage.get("recipient").ok_or("No recipient")?.clone();
                if tx.sender != recip {
                    return Err("Not recipient".into());
                }
                if c.storage.get("status").map(|s| s.as_str()) != Some("active") {
                    return Err("Not active".into());
                }
                let rate: u64 = c
                    .storage
                    .get("rate_per_block")
                    .and_then(|v| v.parse().ok())
                    .unwrap_or(0);
                let last: u64 = c
                    .storage
                    .get("last_withdraw_height")
                    .and_then(|v| v.parse().ok())
                    .unwrap_or(h);
                let owed = rate.saturating_mul(h.saturating_sub(last));
                let avail = state.get_balance(addr).min(owed);
                if avail > 0 {
                    state.debit_account(addr, avail);
                    state.credit_account_direct(&recip, avail);
                }
                c.storage
                    .insert("last_withdraw_height".into(), h.to_string());
                emit(
                    c,
                    h,
                    "StreamWithdrawn",
                    vec![
                        ("recipient", recip),
                        ("amount", avail.to_string()),
                        ("blocks", h.saturating_sub(last).to_string()),
                    ],
                );
            }
            "cancel" => {
                if tx.sender != c.owner {
                    return Err("Not owner".into());
                }
                if c.storage.get("status").map(|s| s.as_str()) != Some("active") {
                    return Err("Not active".into());
                }
                let recip = c.storage.get("recipient").ok_or("No recipient")?.clone();
                let rate: u64 = c
                    .storage
                    .get("rate_per_block")
                    .and_then(|v| v.parse().ok())
                    .unwrap_or(0);
                let last: u64 = c
                    .storage
                    .get("last_withdraw_height")
                    .and_then(|v| v.parse().ok())
                    .unwrap_or(h);
                let owed = rate.saturating_mul(h.saturating_sub(last));
                let bal = state.get_balance(addr);
                let payout = owed.min(bal);
                if payout > 0 {
                    state.debit_account(addr, payout);
                    state.credit_account_direct(&recip, payout);
                }
                let rem = state.get_balance(addr);
                if rem > 0 {
                    state.debit_account(addr, rem);
                    state.credit_account_direct(&c.owner.clone(), rem);
                }
                c.storage.insert("status".into(), "cancelled".into());
                emit(
                    c,
                    h,
                    "StreamCancelled",
                    vec![
                        ("owner", c.owner.clone()),
                        ("recipient_payout", payout.to_string()),
                        ("owner_refund", rem.to_string()),
                    ],
                );
            }
            "topup" => {
                let bal = state.get_balance(addr);
                let rate: u64 = c
                    .storage
                    .get("rate_per_block")
                    .and_then(|v| v.parse().ok())
                    .unwrap_or(0);
                let blocks_rem = if rate > 0 { bal / rate } else { 0 };
                emit(
                    c,
                    h,
                    "StreamToppedUp",
                    vec![
                        ("sender", tx.sender.clone()),
                        ("new_balance", bal.to_string()),
                        ("blocks_remaining", blocks_rem.to_string()),
                    ],
                );
            }
            _ => return Err(format!("Unknown Stream method: {}", method)),
        }
        Ok(())
    }

    // -- AgentRegistry ----------------------------------------------------------
    fn agent_registry(
        state: &mut AccountState,
        tx: &Transaction,
        c: &mut ContractState,
        method: &str,
        call_args: &[u8],
        h: u64,
    ) -> Result<(), String> {
        let _ = state;
        match method {
            "update" => {
                let agent = c.storage.get("agent_address").cloned().unwrap_or_default();
                if tx.sender != agent && tx.sender != c.owner {
                    return Err("Not authorized".into());
                }
                let a: AgentUpdateArgs =
                    serde_json::from_slice(call_args).map_err(|e| e.to_string())?;
                if let Some(ep) = a.endpoint_hash {
                    c.storage.insert("endpoint_hash".into(), ep);
                }
                if let Some(price) = a.price_per_call {
                    c.storage.insert("price_per_call".into(), price.to_string());
                }
                if let Some(act) = a.active {
                    c.storage.insert("active".into(), act.to_string());
                }
                emit(
                    c,
                    h,
                    "AgentRegistryUpdated",
                    vec![("agent", agent), ("updated_by", tx.sender.clone())],
                );
            }
            _ => return Err(format!("Unknown AgentRegistry method: {}", method)),
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::transaction::{SignatureScheme, TransactionType};

    fn tx(sender: &str, recipient: &str, amount: u64, tx_type: TransactionType) -> Transaction {
        Transaction {
            sender: sender.into(),
            recipient: recipient.into(),
            amount,
            timestamp: 0,
            signature: vec![],
            public_key: vec![],
            fee: 100,
            nonce: 1,
            lock_time: 0,
            tx_type,
            sig_scheme: SignatureScheme::Falcon512,
            network_id: 0,
            payload: vec![],
        }
    }

    #[test]
    fn test_escrow_claim() {
        let mut state = AccountState::new();
        let pre = b"secret";
        let mut h = Sha3_256::new();
        h.update(pre);
        let hash = hex::encode(h.finalize());
        let init = serde_json::to_vec(&EscrowInitArgs {
            beneficiary: "0xben".into(),
            secret_hash: hash,
            refund_height: 9999,
        })
        .unwrap();
        let t = tx(
            "0xseller",
            "",
            50_000,
            TransactionType::ContractDeploy {
                template_id: TEMPLATE_ESCROW,
                init_args: init,
            },
        );
        let addr = NativeContracts::generate_address(&t.hash());
        let mut t2 = t.clone();
        t2.recipient = addr.clone();
        state.credit_account(&t2, 100, 0);
        let claim_args = serde_json::to_vec(&EscrowClaimArgs {
            preimage: hex::encode(pre),
        })
        .unwrap();
        let ct = tx(
            "0xagent",
            &addr,
            0,
            TransactionType::ContractCall {
                contract_address: addr.clone(),
                method: "claim".into(),
                call_args: claim_args,
            },
        );
        state.credit_account(&ct, 101, 0);
        assert_eq!(state.get_balance("0xben"), 50_000);
        assert_eq!(
            state
                .contracts
                .get(&addr)
                .unwrap()
                .storage
                .get("status")
                .unwrap(),
            "claimed"
        );
    }

    #[test]
    fn test_agent_job_claim() {
        let mut state = AccountState::new();
        let init = serde_json::to_vec(&AgentJobInitArgs {
            worker: "0xworker".into(),
            task_hash: "cid1".into(),
            deadline_height: 999,
        })
        .unwrap();
        let t = tx(
            "0xemployer",
            "",
            10_000,
            TransactionType::ContractDeploy {
                template_id: TEMPLATE_AGENT_JOB,
                init_args: init,
            },
        );
        let addr = NativeContracts::generate_address(&t.hash());
        let mut t2 = t.clone();
        t2.recipient = addr.clone();
        state.credit_account(&t2, 100, 0);
        let ca = serde_json::to_vec(&AgentJobClaimArgs {
            result_hash: "result_cid".into(),
        })
        .unwrap();
        let ct = tx(
            "0xworker",
            &addr,
            0,
            TransactionType::ContractCall {
                contract_address: addr.clone(),
                method: "claim".into(),
                call_args: ca,
            },
        );
        state.credit_account(&ct, 200, 0);
        assert_eq!(state.get_balance("0xworker"), 10_000);
        assert!(state
            .contracts
            .get(&addr)
            .unwrap()
            .events
            .iter()
            .any(|e| e.name == "AgentJobClaimed"));
    }

    #[test]
    fn test_stream_withdraw() {
        let mut state = AccountState::new();
        let init = serde_json::to_vec(&StreamInitArgs {
            recipient: "0xrecip".into(),
            rate_per_block: 100,
        })
        .unwrap();
        let t = tx(
            "0xowner",
            "",
            10_000,
            TransactionType::ContractDeploy {
                template_id: TEMPLATE_STREAM,
                init_args: init,
            },
        );
        let addr = NativeContracts::generate_address(&t.hash());
        let mut t2 = t.clone();
        t2.recipient = addr.clone();
        state.credit_account(&t2, 0, 0);
        let wt = tx(
            "0xrecip",
            &addr,
            0,
            TransactionType::ContractCall {
                contract_address: addr.clone(),
                method: "withdraw".into(),
                call_args: vec![],
            },
        );
        state.credit_account(&wt, 50, 0);
        assert_eq!(state.get_balance("0xrecip"), 5_000); // 50 blocks * 100 = 5000
    }

    #[test]
    fn test_agent_bid_auction() {
        let mut state = AccountState::new();
        let init = serde_json::to_vec(&AgentBidInitArgs {
            task_hash: "task".into(),
            bid_close_height: 500,
            refund_height: 1000,
        })
        .unwrap();
        let t = tx(
            "0xemployer",
            "",
            100_000,
            TransactionType::ContractDeploy {
                template_id: TEMPLATE_AGENT_BID,
                init_args: init,
            },
        );
        let addr = NativeContracts::generate_address(&t.hash());
        let mut t2 = t.clone();
        t2.recipient = addr.clone();
        state.credit_account(&t2, 100, 0);
        let ba = serde_json::to_vec(&AgentBidSubmitArgs {
            result_hash: "r1".into(),
            price: 60_000,
        })
        .unwrap();
        let bt = tx(
            "0xagent1",
            &addr,
            0,
            TransactionType::ContractCall {
                contract_address: addr.clone(),
                method: "submit_bid".into(),
                call_args: ba,
            },
        );
        state.credit_account(&bt, 200, 0);
        let sa = serde_json::to_vec(&AgentBidSelectArgs {
            winner: "0xagent1".into(),
        })
        .unwrap();
        let st = tx(
            "0xemployer",
            &addr,
            0,
            TransactionType::ContractCall {
                contract_address: addr.clone(),
                method: "select_winner".into(),
                call_args: sa,
            },
        );
        state.credit_account(&st, 600, 0);
        assert_eq!(state.get_balance("0xagent1"), 60_000);
        assert_eq!(state.get_balance("0xemployer"), 40_000);
    }
}
