/// WASM Contract Executor with Gas Metering
/// Executes smart contracts in a sandboxed WASM environment

use crate::contract::{Account, ContractInstruction};
use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};
use wasmer::{
    imports, Function, FunctionEnv, FunctionEnvMut, Instance, Memory, Module, Store, Value,
};
use wasmer_compiler_singlepass::Singlepass;

/// Gas costs for operations (in gas units)
pub mod gas_costs {
    pub const BASE_INSTRUCTION: u64 = 1;
    pub const MEMORY_READ: u64 = 2;
    pub const MEMORY_WRITE: u64 = 3;
    pub const STORAGE_READ: u64 = 100;
    pub const STORAGE_WRITE: u64 = 200;
    pub const CRYPTO_HASH: u64 = 50;
    pub const CRYPTO_VERIFY: u64 = 1000;
    pub const CRYPTO_ENCRYPT: u64 = 500;
}

/// Execution limits
pub const MAX_GAS_PER_TX: u64 = 10_000_000;
pub const MAX_MEMORY_PAGES: u32 = 256; // 16MB max

/// Gas meter for tracking execution costs
#[derive(Clone, Debug)]
pub struct GasMeter {
    gas_limit: u64,
    gas_used: u64,
}

impl GasMeter {
    pub fn new(gas_limit: u64) -> Self {
        Self {
            gas_limit,
            gas_used: 0,
        }
    }

    pub fn consume(&mut self, amount: u64) -> Result<(), ExecutionError> {
        self.gas_used += amount;
        if self.gas_used > self.gas_limit {
            Err(ExecutionError::OutOfGas {
                limit: self.gas_limit,
                used: self.gas_used,
            })
        } else {
            Ok(())
        }
    }

    pub fn remaining(&self) -> u64 {
        self.gas_limit.saturating_sub(self.gas_used)
    }

    pub fn used(&self) -> u64 {
        self.gas_used
    }
}

/// Environment accessible to WASM contracts
#[derive(Clone)]
pub struct ContractEnv {
    pub gas_meter: Arc<Mutex<GasMeter>>,
    pub accounts: Arc<Mutex<Vec<Account>>>,
    pub block_height: u64,
    pub quantum_entropy: [u8; 32],
    pub logs: Arc<Mutex<Vec<String>>>,
    pub memory: Option<Memory>,
}

impl ContractEnv {
    pub fn new(
        gas_limit: u64,
        accounts: Vec<Account>,
        block_height: u64,
        quantum_entropy: [u8; 32],
    ) -> Self {
        Self {
            gas_meter: Arc::new(Mutex::new(GasMeter::new(gas_limit))),
            accounts: Arc::new(Mutex::new(accounts)),
            block_height,
            quantum_entropy,
            logs: Arc::new(Mutex::new(Vec::new())),
            memory: None,
        }
    }
    
    pub fn set_memory(&mut self, memory: Memory) {
        self.memory = Some(memory);
    }
}

/// Execution result
#[derive(Debug, Serialize, Deserialize)]
pub struct ExecutionResult {
    pub success: bool,
    pub gas_used: u64,
    pub return_data: Vec<u8>,
    pub logs: Vec<String>,
    pub error: Option<String>,
}

/// Execution errors
#[derive(Debug, thiserror::Error)]
pub enum ExecutionError {
    #[error("Out of gas: used {used}, limit {limit}")]
    OutOfGas { limit: u64, used: u64 },

    #[error("WASM compilation failed: {0}")]
    CompilationError(String),

    #[error("WASM instantiation failed: {0}")]
    InstantiationError(String),

    #[error("Function not found: {0}")]
    FunctionNotFound(String),

    #[error("Execution failed: {0}")]
    ExecutionFailed(String),

    #[error("Invalid account index: {0}")]
    InvalidAccountIndex(usize),

    #[error("Memory access error: {0}")]
    MemoryError(String),

    #[error("Account not writable")]
    AccountNotWritable,
}

/// Contract executor
pub struct ContractExecutor {
    store: Store,
}

impl ContractExecutor {
    pub fn new() -> Self {
        let compiler = Singlepass::default();
        let store = Store::new(compiler);
        Self { store }
    }

    /// Execute a contract instruction
    pub fn execute(
        &mut self,
        code: &[u8],
        _instruction: &ContractInstruction,
        accounts: Vec<Account>,
        block_height: u64,
        quantum_entropy: [u8; 32],
        gas_limit: u64,
    ) -> Result<ExecutionResult, ExecutionError> {
        // Create execution environment
        let mut env = ContractEnv::new(gas_limit, accounts, block_height, quantum_entropy);
        let func_env = FunctionEnv::new(&mut self.store, env.clone());

        // Compile WASM module
        let module = Module::new(&self.store, code)
            .map_err(|e| ExecutionError::CompilationError(e.to_string()))?;

        // Create imports with host functions
        let imports = imports! {
            "env" => {
                "consume_gas" => Function::new_typed_with_env(&mut self.store, &func_env, consume_gas),
                "log" => Function::new_typed_with_env(&mut self.store, &func_env, log_message),
                "get_account_balance" => Function::new_typed_with_env(&mut self.store, &func_env, get_account_balance),
                "set_account_data" => Function::new_typed_with_env(&mut self.store, &func_env, set_account_data),
                "get_account_data" => Function::new_typed_with_env(&mut self.store, &func_env, get_account_data),
                "quantum_random" => Function::new_typed_with_env(&mut self.store, &func_env, quantum_random),
                "sha3_hash" => Function::new_typed_with_env(&mut self.store, &func_env, sha3_hash),
                "falcon_verify" => Function::new_typed_with_env(&mut self.store, &func_env, falcon_verify),
                "get_block_height" => Function::new_typed_with_env(&mut self.store, &func_env, get_block_height),
            }
        };

        // Instantiate module
        let instance = Instance::new(&mut self.store, &module, &imports)
            .map_err(|e| ExecutionError::InstantiationError(e.to_string()))?;

        // Get memory and store it in environment
        if let Ok(memory) = instance.exports.get_memory("memory") {
            env.set_memory(memory.clone());
            func_env.as_mut(&mut self.store).set_memory(memory.clone());
        }

        // Get the entrypoint function
        let entrypoint = instance
            .exports
            .get_function("process_instruction")
            .map_err(|_| ExecutionError::FunctionNotFound("process_instruction".to_string()))?;

        // Execute the contract
        let result = entrypoint
            .call(&mut self.store, &[])
            .map_err(|e| ExecutionError::ExecutionFailed(e.to_string()))?;

        // Extract return value (0 = success, non-zero = error)
        let success = match result.first() {
            Some(Value::I32(0)) => true,
            _ => false,
        };

        // Get final state
        let gas_used = env.gas_meter.lock().unwrap().used();
        let logs = env.logs.lock().unwrap().clone();

        Ok(ExecutionResult {
            success,
            gas_used,
            return_data: vec![],
            logs,
            error: if success {
                None
            } else {
                Some("Contract execution returned error".to_string())
            },
        })
    }
}

// Host functions exposed to WASM contracts

/// Consume gas
fn consume_gas(env: FunctionEnvMut<ContractEnv>, amount: u64) -> i32 {
    let contract_env = env.data();
    match contract_env.gas_meter.lock().unwrap().consume(amount) {
        Ok(_) => 0,
        Err(_) => 1, // Out of gas
    }
}

/// Log a message from contract
fn log_message(mut env: FunctionEnvMut<ContractEnv>, ptr: u32, len: u32) -> i32 {
    let (data, store) = env.data_and_store_mut();
    
    // Get memory
    let memory = match &data.memory {
        Some(mem) => mem.clone(),
        None => {
            data.logs.lock().unwrap().push("Memory not available".to_string());
            return 1;
        }
    };
    
    // Read string from WASM memory
    let view = memory.view(&store);
    let mut bytes = vec![0u8; len as usize];
    
    if let Err(_) = view.read(ptr as u64, &mut bytes) {
        data.logs.lock().unwrap().push(format!("Failed to read memory at ptr={}", ptr));
        return 1;
    }

    if let Ok(message) = String::from_utf8(bytes) {
        data.logs.lock().unwrap().push(message);
        0
    } else {
        data.logs.lock().unwrap().push("Invalid UTF-8 in log message".to_string());
        1
    }
}

/// Get account balance
fn get_account_balance(env: FunctionEnvMut<ContractEnv>, index: u32) -> u64 {
    let contract_env = env.data();
    let accounts = contract_env.accounts.lock().unwrap();
    accounts
        .get(index as usize)
        .map(|acc| acc.balance)
        .unwrap_or(0)
}

/// Set account data
fn set_account_data(
    mut env: FunctionEnvMut<ContractEnv>,
    index: u32,
    ptr: u32,
    len: u32,
) -> i32 {
    let (data, store) = env.data_and_store_mut();

    // Consume gas for storage write
    if data
        .gas_meter
        .lock()
        .unwrap()
        .consume(gas_costs::STORAGE_WRITE)
        .is_err()
    {
        return 1;
    }

    // Get memory
    let memory = match &data.memory {
        Some(mem) => mem.clone(),
        None => return 1,
    };
    
    // Read data from WASM memory
    let view = memory.view(&store);
    let mut bytes = vec![0u8; len as usize];
    
    if let Err(_) = view.read(ptr as u64, &mut bytes) {
        return 1;
    }

    // Update account data
    let mut accounts = data.accounts.lock().unwrap();
    if let Some(account) = accounts.get_mut(index as usize) {
        account.data = bytes;
        0
    } else {
        1
    }
}

/// Get account data
fn get_account_data(
    mut env: FunctionEnvMut<ContractEnv>,
    index: u32,
    ptr: u32,
    max_len: u32,
) -> i32 {
    let (data, store) = env.data_and_store_mut();

    // Consume gas for storage read
    if data
        .gas_meter
        .lock()
        .unwrap()
        .consume(gas_costs::STORAGE_READ)
        .is_err()
    {
        return -1;
    }

    // Get account data
    let accounts = data.accounts.lock().unwrap();
    let account_data = match accounts.get(index as usize) {
        Some(acc) => &acc.data,
        None => return -1,
    };

    // Get memory
    let memory = match &data.memory {
        Some(mem) => mem.clone(),
        None => return -1,
    };
    
    // Write data to WASM memory
    let view = memory.view(&store);
    let write_len = account_data.len().min(max_len as usize);
    
    if let Err(_) = view.write(ptr as u64, &account_data[..write_len]) {
        return -1;
    }

    write_len as i32
}

/// Get quantum random number
fn quantum_random(env: FunctionEnvMut<ContractEnv>, max: u32) -> u32 {
    let contract_env = env.data();

    // Consume gas
    if contract_env
        .gas_meter
        .lock()
        .unwrap()
        .consume(gas_costs::CRYPTO_HASH)
        .is_err()
    {
        return 0;
    }

    // Use quantum entropy
    let result = crate::contract::quantum_primitives::quantum_random(
        contract_env.quantum_entropy,
        max as usize,
    );

    match result {
        Ok(val) => val as u32,
        Err(_) => 0,
    }
}

/// SHA3 hash
fn sha3_hash(mut env: FunctionEnvMut<ContractEnv>, ptr: u32, len: u32, out_ptr: u32) -> i32 {
    use sha3::{Digest, Sha3_256};
    
    let (data, store) = env.data_and_store_mut();

    // Consume gas
    if data
        .gas_meter
        .lock()
        .unwrap()
        .consume(gas_costs::CRYPTO_HASH)
        .is_err()
    {
        return 1;
    }

    // Get memory
    let memory = match &data.memory {
        Some(mem) => mem.clone(),
        None => return 1,
    };
    
    // Read input data
    let view = memory.view(&store);
    let mut input = vec![0u8; len as usize];
    
    if let Err(_) = view.read(ptr as u64, &mut input) {
        return 1;
    }

    // Compute hash
    let hash = Sha3_256::digest(&input);
    
    // Write hash to output
    if let Err(_) = view.write(out_ptr as u64, hash.as_slice()) {
        return 1;
    }

    0
}

/// Verify Falcon signature
fn falcon_verify(
    mut env: FunctionEnvMut<ContractEnv>,
    msg_ptr: u32,
    msg_len: u32,
    sig_ptr: u32,
    sig_len: u32,
    pk_ptr: u32,
    pk_len: u32,
) -> i32 {
    let (data, store) = env.data_and_store_mut();

    // Consume gas for signature verification
    if data
        .gas_meter
        .lock()
        .unwrap()
        .consume(gas_costs::CRYPTO_VERIFY)
        .is_err()
    {
        return 1;
    }

    // Get memory
    let memory = match &data.memory {
        Some(mem) => mem.clone(),
        None => return 1,
    };
    
    let view = memory.view(&store);
    
    // Read message
    let mut message = vec![0u8; msg_len as usize];
    if let Err(_) = view.read(msg_ptr as u64, &mut message) {
        return 1;
    }
    
    // Read signature
    let mut signature = vec![0u8; sig_len as usize];
    if let Err(_) = view.read(sig_ptr as u64, &mut signature) {
        return 1;
    }
    
    // Read public key
    let mut public_key = vec![0u8; pk_len as usize];
    if let Err(_) = view.read(pk_ptr as u64, &mut public_key) {
        return 1;
    }

    // Verify signature
    match crate::contract::quantum_primitives::verify_falcon_signature(&message, &signature, &public_key) {
        Ok(true) => 0,
        _ => 1,
    }
}

/// Get current block height
fn get_block_height(env: FunctionEnvMut<ContractEnv>) -> u64 {
    env.data().block_height
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_gas_meter() {
        let mut meter = GasMeter::new(1000);
        assert_eq!(meter.remaining(), 1000);

        meter.consume(300).unwrap();
        assert_eq!(meter.used(), 300);
        assert_eq!(meter.remaining(), 700);

        meter.consume(700).unwrap();
        assert_eq!(meter.used(), 1000);

        // Should fail
        assert!(meter.consume(1).is_err());
    }

    #[test]
    fn test_contract_env() {
        let accounts = vec![Account::new_user(
            "test".to_string(),
            vec![],
            1000,
        )];

        let env = ContractEnv::new(10000, accounts, 100, [0u8; 32]);
        assert_eq!(env.block_height, 100);
        assert_eq!(env.accounts.lock().unwrap().len(), 1);
    }
}
