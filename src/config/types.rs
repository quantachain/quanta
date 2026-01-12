use serde::{Deserialize, Serialize};
use std::path::Path;
use config::{Config, ConfigError, File};
use crate::core::ChainNetwork;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuantaConfig {
    pub version: u32,
    pub network_type: ChainNetwork,
    pub node: NodeConfig,
    pub network: NetworkConfig,
    pub consensus: ConsensusConfig,
    pub security: SecurityConfig,
    pub mining: MiningConfig,
    pub metrics: MetricsConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeConfig {
    pub api_port: u16,
    pub network_port: u16,
    pub rpc_port: u16,
    pub db_path: String,
    pub no_network: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkConfig {
    pub max_peers: usize,
    pub bootstrap_nodes: Vec<String>,
    pub dns_seeds: Vec<String>,
}

/// Consensus-critical configuration (MUST match across all nodes)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConsensusConfig {
    pub max_block_transactions: usize,
    pub max_block_size_bytes: usize,
    pub min_transaction_fee_microunits: u64,
    pub transaction_expiry_blocks: u64,
    pub coinbase_maturity: u64,
}

/// Node-local security preferences (can differ between nodes)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityConfig {
    pub max_mempool_size: usize,
    pub transaction_expiry_seconds: i64,
    /// Enable rate limiting on API endpoints (PRODUCTION: true)
    pub enable_rate_limiting: bool,
    /// Max requests per minute per IP (PRODUCTION: 60)
    pub rate_limit_per_minute: u32,
    /// Enable peer banning for malicious behavior
    pub enable_peer_banning: bool,
    /// Require TLS for API (PRODUCTION: true)
    pub require_tls: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MiningConfig {
    // Adaptive tokenomics with annual reduction
    pub year_1_reward_microunits: u64,
    pub annual_reduction_percent: u8,
    pub min_reward_microunits: u64,
    pub blocks_per_year: u64,
    
    // Early adopter incentives
    pub early_adopter_bonus_blocks: u64,
    pub early_adopter_multiplier: f64,
    pub bootstrap_phase_blocks: u64,
    
    // Mining reward vesting (anti-dump mechanism)
    pub mining_reward_lock_percent: u8,
    pub mining_reward_lock_blocks: u64,
    
    // Fee distribution
    pub fee_burn_percent: u8,
    pub fee_treasury_percent: u8,
    pub fee_validator_percent: u8,
    
    // Blockchain timing
    pub target_block_time: u64,
    pub difficulty_adjustment_interval: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricsConfig {
    pub enabled: bool,
    pub port: u16,
}

impl Default for QuantaConfig {
    fn default() -> Self {
        Self {
            version: 1,
            network_type: ChainNetwork::Mainnet,
            node: NodeConfig {
                api_port: 3000,
                network_port: 8333,
                rpc_port: 7782,
                db_path: "./quanta_data".to_string(),
                no_network: false,
            },
            network: NetworkConfig {
                max_peers: 125,
                bootstrap_nodes: Vec::new(),
                dns_seeds: vec![
                    // Add DNS seeds here for mainnet:
                    // "seed1.quanta.network".to_string(),
                    // "seed2.quanta.network".to_string(),
                    // "seed3.quanta.network".to_string(),
                ],
            },
            consensus: ConsensusConfig {
                max_block_transactions: 2000,
                max_block_size_bytes: 1_048_576,
                min_transaction_fee_microunits: 100, // 0.0001 QUA
                transaction_expiry_blocks: 8640, // ~24 hours at 10s blocks
                coinbase_maturity: 100,
            },
            security: SecurityConfig {
                max_mempool_size: 5000,
                transaction_expiry_seconds: 86400,
                enable_rate_limiting: true,  // PRODUCTION: Always enable
                rate_limit_per_minute: 60,   // 60 requests/min per IP
                enable_peer_banning: true,   // Auto-ban malicious peers
                require_tls: false,          // Set true for public nodes
            },
            mining: MiningConfig {
                year_1_reward_microunits: 100_000_000, // 100 QUA
                annual_reduction_percent: 15,
                min_reward_microunits: 5_000_000, // 5 QUA floor
                blocks_per_year: 3_153_600,
                early_adopter_bonus_blocks: 100_000,
                early_adopter_multiplier: 1.5,
                bootstrap_phase_blocks: 315_360,
                mining_reward_lock_percent: 50,
                mining_reward_lock_blocks: 157_680,
                fee_burn_percent: 70,
                fee_treasury_percent: 20,
                fee_validator_percent: 10,
                target_block_time: 10,
                difficulty_adjustment_interval: 10,
            },
            metrics: MetricsConfig {
                enabled: true,
                port: 9090,
            },
        }
    }
}

impl QuantaConfig {
    /// Load configuration from file (with optional environment variable overrides)
    pub fn from_file<P: AsRef<Path>>(path: P) -> Result<Self, ConfigError> {
        let config = Config::builder()
            .add_source(File::from(path.as_ref()))
            // Add environment variable overrides (e.g., QUANTA_NODE__API_PORT=4000)
            .add_source(
                config::Environment::with_prefix("QUANTA")
                    .separator("__")
                    .try_parsing(true)
            )
            .build()?;
        
        config.try_deserialize()
    }

    /// Load configuration with CLI overrides
    pub fn load_with_overrides(
        config_file: Option<String>,
        api_port: Option<u16>,
        network_port: Option<u16>,
        db_path: Option<String>,
        bootstrap: Option<String>,
        network_name: Option<String>,
        no_network: bool,
    ) -> Result<Self, ConfigError> {
        let mut config = if let Some(path) = config_file {
            Self::from_file(path)?
        } else if Path::new("quanta.toml").exists() {
            Self::from_file("quanta.toml")?
        } else {
            Self::default()
        };

        // CLI overrides
        if let Some(port) = api_port {
            config.node.api_port = port;
        }
        if let Some(port) = network_port {
            config.node.network_port = port;
        }
        if let Some(path) = db_path {
            config.node.db_path = path;
        }
        if let Some(bootstrap_str) = bootstrap {
            config.network.bootstrap_nodes = bootstrap_str
                .split(',')
                .map(|s| s.trim().to_string())
                .collect();
        }
        if no_network {
            config.node.no_network = true;
        }
        
        // Handle Network Type Override
        if let Some(net) = network_name {
            match net.as_str() {
                "testnet" => {
                    config.network_type = ChainNetwork::Testnet;
                    // Auto-configure testnet defaults if not explicitly set
                    if config.node.network_port == 8333 { config.node.network_port = 18333; }
                    if config.node.api_port == 3000 { config.node.api_port = 13000; }
                    if config.node.rpc_port == 7782 { config.node.rpc_port = 17782; }
                    if config.node.db_path == "./quanta_data" { config.node.db_path = "./quanta_data_testnet".to_string(); }
                    
                    // Add testnet seed
                    config.network.dns_seeds = vec!["seed.testnet.quantachain.org".to_string()];
                },
                "mainnet" => {
                    config.network_type = ChainNetwork::Mainnet;
                },
                _ => {} // Unknown network, keep default or config file value
            }
        }

        Ok(config)
    }

    /// Save configuration to file
    pub fn save<P: AsRef<Path>>(&self, path: P) -> Result<(), std::io::Error> {
        let toml_string = toml::to_string_pretty(self)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
        std::fs::write(path, toml_string)
    }
    
    /// Validate configuration for sanity and safety
    pub fn validate(&self) -> Result<(), String> {
        // Port conflicts
        if self.node.api_port == self.node.network_port {
            return Err("API port and network port must differ".into());
        }
        if self.node.api_port == self.metrics.port {
            return Err("API port and metrics port must differ".into());
        }
        
        // Consensus rules must be sane
        if self.consensus.max_block_size_bytes == 0 {
            return Err("Block size must be > 0".into());
        }
        if self.consensus.max_block_transactions == 0 {
            return Err("Max block transactions must be > 0".into());
        }
        if self.consensus.min_transaction_fee_microunits == 0 {
            return Err("Minimum transaction fee must be > 0 (prevents spam)".into());
        }
        if self.consensus.transaction_expiry_blocks == 0 {
            return Err("Transaction expiry blocks must be > 0".into());
        }
        if self.consensus.coinbase_maturity == 0 {
            return Err("Coinbase maturity must be > 0 (prevents mining attacks)".into());
        }
        
        // Mining config validation
        if self.mining.target_block_time == 0 {
            return Err("Target block time must be > 0".into());
        }
        if self.mining.difficulty_adjustment_interval == 0 {
            return Err("Difficulty adjustment interval must be > 0".into());
        }
        if self.mining.year_1_reward_microunits == 0 {
            return Err("Year 1 mining reward must be > 0".into());
        }
        if self.mining.annual_reduction_percent > 100 {
            return Err("Annual reduction percent must be <= 100".into());
        }
        if self.mining.fee_burn_percent + self.mining.fee_treasury_percent + self.mining.fee_validator_percent != 100 {
            return Err("Fee distribution percentages must sum to 100".into());
        }
        
        // Security limits
        if self.security.max_mempool_size == 0 {
            return Err("Max mempool size must be > 0".into());
        }
        
        // Network sanity
        if self.network.max_peers == 0 {
            return Err("Max peers must be > 0 (unless running solo)".into());
        }
        
        Ok(())
    }
    
    /// Print effective configuration on startup (debugging lifesaver)
    pub fn print_effective_config(&self) {
        tracing::info!("========================================");
        tracing::info!("Quanta Node Configuration (v{})", self.version);
        tracing::info!("========================================");
        tracing::info!("Node:");
        tracing::info!("  API Port: {}", self.node.api_port);
        tracing::info!("  Network Port: {}", self.node.network_port);
        tracing::info!("  DB Path: {}", self.node.db_path);
        tracing::info!("  Network Disabled: {}", self.node.no_network);
        tracing::info!("Network:");
        tracing::info!("  Max Peers: {}", self.network.max_peers);
        tracing::info!("  Bootstrap Nodes: {:?}", self.network.bootstrap_nodes);
        tracing::info!("Consensus (MUST match network):");
        tracing::info!("  Max Block Size: {} bytes", self.consensus.max_block_size_bytes);
        tracing::info!("  Max Block Txs: {}", self.consensus.max_block_transactions);
        tracing::info!("  Min Fee: {} microunits", self.consensus.min_transaction_fee_microunits);
        tracing::info!("  Tx Expiry: {} blocks", self.consensus.transaction_expiry_blocks);
        tracing::info!("  Coinbase Maturity: {} blocks", self.consensus.coinbase_maturity);
        tracing::info!("Mining:");
        tracing::info!("  Year 1 Reward: {} microunits", self.mining.year_1_reward_microunits);
        tracing::info!("  Annual Reduction: {}%", self.mining.annual_reduction_percent);
        tracing::info!("  Min Reward Floor: {} microunits", self.mining.min_reward_microunits);
        tracing::info!("  Early Adopter Bonus: {} blocks @ {}x", self.mining.early_adopter_bonus_blocks, self.mining.early_adopter_multiplier);
        tracing::info!("  Reward Vesting: {}% locked for {} blocks", self.mining.mining_reward_lock_percent, self.mining.mining_reward_lock_blocks);
        tracing::info!("  Fee Distribution: {}% burn, {}% treasury, {}% validator", 
            self.mining.fee_burn_percent, self.mining.fee_treasury_percent, self.mining.fee_validator_percent);
        tracing::info!("  Target Block Time: {}s", self.mining.target_block_time);
        tracing::info!("  Difficulty Adjustment: {} blocks", self.mining.difficulty_adjustment_interval);
        tracing::info!("Security:");
        tracing::info!("  Max Mempool: {} txs", self.security.max_mempool_size);
        tracing::info!("Metrics:");
        tracing::info!("  Enabled: {}", self.metrics.enabled);
        tracing::info!("  Port: {}", self.metrics.port);
        tracing::info!("========================================");
    }
}
