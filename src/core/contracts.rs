use serde::{Deserialize, Serialize};
use sha3::{Sha3_256, Digest};
use crate::core::transaction::{AccountState, ContractState, Transaction};

pub const TEMPLATE_ESCROW: u8 = 1;
pub const TEMPLATE_AGENT_JOB: u8 = 2;

#[derive(Serialize, Deserialize)]
pub struct EscrowInitArgs {
    pub beneficiary: String,
    pub secret_hash: String, // Hex string of the sha3-256 hash
}

#[derive(Serialize, Deserialize)]
pub struct EscrowClaimArgs {
    pub preimage: String, // Hex string of the pre-image
}

#[derive(Serialize, Deserialize)]
pub struct AgentJobInitArgs {
    pub worker: String, // The agent authorized to execute the job
    pub task_hash: String, // IPFS/SHA3 hash of the task prompt/requirements
}

#[derive(Serialize, Deserialize)]
pub struct AgentJobClaimArgs {
    pub result_hash: String, // IPFS/SHA3 hash of the inference result
}

/// Routes ContractDeploy and ContractCall to the appropriate Native Template.
pub struct NativeContracts;

impl NativeContracts {
    /// Executes a contract transaction safely. If the contract fails, the error is logged
    /// but the transaction itself remains valid (fee is paid, nonce increments).
    pub fn execute(state: &mut AccountState, tx: &Transaction, current_height: u64) {
        match &tx.tx_type {
            crate::core::transaction::TransactionType::ContractDeploy { template_id, init_args } => {
                let tx_hash = tx.hash();
                if let Err(e) = Self::deploy(state, tx, &tx_hash, *template_id, init_args, current_height) {
                    tracing::warn!("Contract deployment failed: {}", e);
                }
            }
            crate::core::transaction::TransactionType::ContractCall { contract_address, method, call_args } => {
                if let Err(e) = Self::call(state, tx, contract_address, method, call_args) {
                    tracing::warn!("Contract call failed: {}", e);
                }
            }
            _ => {}
        }
    }

    /// Generates a deterministic contract address from the deployment transaction hash.
    pub fn generate_address(tx_hash: &str) -> String {
        format!("0xc_{}", &tx_hash[0..36])
    }

    /// Handles a ContractDeploy transaction.
    pub fn deploy(
        state: &mut AccountState,
        tx: &Transaction,
        tx_hash: &str,
        template_id: u8,
        init_args: &[u8],
        current_height: u64,
    ) -> Result<String, String> {
        let contract_address = Self::generate_address(tx_hash);

        let mut storage = std::collections::HashMap::new();

        match template_id {
            TEMPLATE_ESCROW => {
                let args: EscrowInitArgs = serde_json::from_slice(init_args)
                    .map_err(|e| format!("Failed to parse EscrowInitArgs: {}", e))?;
                
                storage.insert("beneficiary".to_string(), args.beneficiary);
                storage.insert("secret_hash".to_string(), args.secret_hash);
                storage.insert("status".to_string(), "locked".to_string());
            }
            TEMPLATE_AGENT_JOB => {
                let args: AgentJobInitArgs = serde_json::from_slice(init_args)
                    .map_err(|e| format!("Failed to parse AgentJobInitArgs: {}", e))?;
                
                storage.insert("worker".to_string(), args.worker);
                storage.insert("task_hash".to_string(), args.task_hash);
                storage.insert("status".to_string(), "open".to_string());
            }
            _ => return Err(format!("Unknown template_id: {}", template_id)),
        }

        let contract_state = ContractState {
            owner: tx.sender.clone(),
            template_id,
            deployed_at: current_height,
            storage,
        };

        state.contracts.insert(contract_address.clone(), contract_state);
        
        // The transaction amount is credited to the contract address in `blockchain.rs`

        Ok(contract_address)
    }

    /// Handles a ContractCall transaction.
    pub fn call(
        state: &mut AccountState,
        tx: &Transaction,
        contract_address: &str,
        method: &str,
        call_args: &[u8],
    ) -> Result<(), String> {
        // We must clone the contract state temporarily to mutate it and the global state
        let mut contract = state.contracts.get(contract_address)
            .ok_or_else(|| "Contract not found".to_string())?
            .clone();

        match contract.template_id {
            TEMPLATE_ESCROW => {
                if method != "claim" {
                    return Err(format!("Unknown method for Escrow: {}", method));
                }
                
                if contract.storage.get("status").map(|s| s.as_str()) != Some("locked") {
                    return Err("Escrow is not locked".to_string());
                }

                let args: EscrowClaimArgs = serde_json::from_slice(call_args)
                    .map_err(|e| format!("Failed to parse EscrowClaimArgs: {}", e))?;
                
                // Verify the pre-image
                let preimage_bytes = hex::decode(&args.preimage)
                    .map_err(|_| "Invalid hex in preimage".to_string())?;
                
                let mut hasher = Sha3_256::new();
                hasher.update(&preimage_bytes);
                let computed_hash = hex::encode(hasher.finalize());

                let expected_hash = contract.storage.get("secret_hash")
                    .ok_or_else(|| "Missing secret_hash in storage".to_string())?;

                if &computed_hash != expected_hash {
                    return Err("Invalid preimage hash".to_string());
                }

                // Unlock funds
                let beneficiary = contract.storage.get("beneficiary")
                    .ok_or_else(|| "Missing beneficiary in storage".to_string())?
                    .clone();
                
                let amount = state.get_balance(contract_address);
                if amount > 0 {
                    if !state.debit_account(contract_address, amount) {
                        return Err("Failed to debit contract".to_string());
                    }
                    // We must manually credit the beneficiary here since the overarching
                    // transaction only credits the contract (or whatever the tx recipient is).
                    state.credit_account_direct(&beneficiary, amount);
                }

                contract.storage.insert("status".to_string(), "claimed".to_string());
            }
            TEMPLATE_AGENT_JOB => {
                if method != "claim" {
                    return Err(format!("Unknown method for AgentJob: {}", method));
                }
                
                if contract.storage.get("status").map(|s| s.as_str()) != Some("open") {
                    return Err("Agent job is not open".to_string());
                }

                let args: AgentJobClaimArgs = serde_json::from_slice(call_args)
                    .map_err(|e| format!("Failed to parse AgentJobClaimArgs: {}", e))?;
                
                // Only the designated worker can claim
                let expected_worker = contract.storage.get("worker")
                    .ok_or_else(|| "Missing worker in storage".to_string())?
                    .clone();
                
                if tx.sender != expected_worker {
                    return Err("Caller is not the designated worker".to_string());
                }

                // Record the result hash
                contract.storage.insert("result_hash".to_string(), args.result_hash);
                contract.storage.insert("status".to_string(), "claimed".to_string());

                // Pay out any locked QUA gas to the worker
                let amount = state.get_balance(contract_address);
                if amount > 0 {
                    if !state.debit_account(contract_address, amount) {
                        return Err("Failed to debit contract".to_string());
                    }
                    state.credit_account_direct(&expected_worker, amount);
                }
            }
            _ => return Err("Unknown template_id during call".to_string()),
        }

        // Save contract state back
        state.contracts.insert(contract_address.to_string(), contract);

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::transaction::{TransactionType, SignatureScheme};

    #[test]
    fn test_native_escrow_template() {
        let mut state = AccountState::new();
        
        let preimage = b"super_secret_data";
        let mut hasher = Sha3_256::new();
        hasher.update(preimage);
        let secret_hash = hex::encode(hasher.finalize());

        let init_args = serde_json::to_vec(&EscrowInitArgs {
            beneficiary: "0xbeneficiary".to_string(),
            secret_hash: secret_hash.clone(),
        }).unwrap();

        // 1. Deploy Contract
        let mut deploy_tx = Transaction {
            sender: "0xseller".to_string(),
            recipient: "".to_string(),
            amount: 50_000,
            timestamp: 0,
            signature: vec![],
            public_key: vec![],
            fee: 100,
            nonce: 1,
            lock_time: 0,
            tx_type: TransactionType::ContractDeploy {
                template_id: TEMPLATE_ESCROW,
                init_args,
            },
            sig_scheme: SignatureScheme::Falcon512,
            network_id: 0,
            payload: vec![],
        };

        // We simulate the transaction being added to a block by computing its hash
        let tx_hash = deploy_tx.hash();
        let contract_address = NativeContracts::generate_address(&tx_hash);
        deploy_tx.recipient = contract_address.clone();

        // Execute deployment via state credit
        state.credit_account(&deploy_tx, 100, 0);

        // Verify contract is deployed
        let contract = state.contracts.get(&contract_address).expect("Contract must exist");
        assert_eq!(contract.template_id, TEMPLATE_ESCROW);
        assert_eq!(contract.storage.get("status").unwrap(), "locked");
        assert_eq!(state.get_balance(&contract_address), 50_000);

        // 2. Claim Funds
        let claim_args = serde_json::to_vec(&EscrowClaimArgs {
            preimage: hex::encode(preimage),
        }).unwrap();

        let claim_tx = Transaction {
            sender: "0xagent".to_string(),
            recipient: contract_address.clone(),
            amount: 0,
            timestamp: 0,
            signature: vec![],
            public_key: vec![],
            fee: 100,
            nonce: 2,
            lock_time: 0,
            tx_type: TransactionType::ContractCall {
                contract_address: contract_address.clone(),
                method: "claim".to_string(),
                call_args: claim_args,
            },
            sig_scheme: SignatureScheme::Falcon512,
            network_id: 0,
            payload: vec![],
        };

        state.credit_account(&claim_tx, 101, 0);

        // Verify funds transferred
        assert_eq!(state.get_balance(&contract_address), 0, "Contract should be drained");
        assert_eq!(state.get_balance("0xbeneficiary"), 50_000, "Beneficiary should receive funds");
        
        let contract_after = state.contracts.get(&contract_address).unwrap();
        assert_eq!(contract_after.storage.get("status").unwrap(), "claimed");
    }

    #[test]
    fn test_native_agent_job_template() {
        let mut state = AccountState::new();
        
        let init_args = serde_json::to_vec(&AgentJobInitArgs {
            worker: "0xworker".to_string(),
            task_hash: "abcd1234taskhash".to_string(),
        }).unwrap();

        // 1. Deploy Contract
        let mut deploy_tx = Transaction {
            sender: "0xemployer".to_string(),
            recipient: "".to_string(),
            amount: 10_000,
            timestamp: 0,
            signature: vec![],
            public_key: vec![],
            fee: 100,
            nonce: 1,
            lock_time: 0,
            tx_type: TransactionType::ContractDeploy {
                template_id: TEMPLATE_AGENT_JOB,
                init_args,
            },
            sig_scheme: SignatureScheme::Falcon512,
            network_id: 0,
            payload: vec![],
        };

        let tx_hash = deploy_tx.hash();
        let contract_address = NativeContracts::generate_address(&tx_hash);
        deploy_tx.recipient = contract_address.clone();

        state.credit_account(&deploy_tx, 100, 0);

        let contract = state.contracts.get(&contract_address).expect("Contract must exist");
        assert_eq!(contract.template_id, TEMPLATE_AGENT_JOB);
        assert_eq!(contract.storage.get("status").unwrap(), "open");
        assert_eq!(state.get_balance(&contract_address), 10_000);

        // 2. Claim Funds
        let claim_args = serde_json::to_vec(&AgentJobClaimArgs {
            result_hash: "deadbeefresult".to_string(),
        }).unwrap();

        let claim_tx = Transaction {
            sender: "0xworker".to_string(), // MUST be the worker
            recipient: contract_address.clone(),
            amount: 0,
            timestamp: 0,
            signature: vec![],
            public_key: vec![],
            fee: 100,
            nonce: 2,
            lock_time: 0,
            tx_type: TransactionType::ContractCall {
                contract_address: contract_address.clone(),
                method: "claim".to_string(),
                call_args: claim_args,
            },
            sig_scheme: SignatureScheme::Falcon512,
            network_id: 0,
            payload: vec![],
        };

        state.credit_account(&claim_tx, 101, 0);

        // Verify funds transferred
        assert_eq!(state.get_balance(&contract_address), 0, "Contract should be drained");
        assert_eq!(state.get_balance("0xworker"), 10_000, "Worker should receive gas funds");
        
        let contract_after = state.contracts.get(&contract_address).unwrap();
        assert_eq!(contract_after.storage.get("status").unwrap(), "claimed");
        assert_eq!(contract_after.storage.get("result_hash").unwrap(), "deadbeefresult");
    }
}
