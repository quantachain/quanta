use serde::{Deserialize, Serialize};
use std::path::Path;
use config::{Config, ConfigError, File};
use crate::core::ChainNetwork;

/// Node storage/sync mode
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum NodeMode {
    /// Keep all blocks forever — needed for block explorers and history queries
    Archive,
    /// Keep only recent blocks (configurable window) — good for validators
    Pruned,
    /// Keep only block headers — minimal footprint, cannot serve full blocks
    Light,
}

/// Consensus engine selection
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ConsensusEngine {
    /// Proof-of-Work (SHA3-256) — V1 consensus (deprecated)
    ProofOfWork,
    /// Asynchronous Byzantine Fault Tolerance — V2 consensus
    Bft,
    /// Proof-of-Stake — Alias for Bft
    ProofOfStake,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuantaConfig {
    pub version: u32,
    pub network_type: ChainNetwork,
    pub node: NodeConfig,
    pub network: NetworkConfig,
    pub security: SecurityConfig,
    pub metrics: MetricsConfig,
    /// Consensus engine: proof_of_work | proof_of_stake (planned)
    #[serde(default = "QuantaConfig::default_engine")]
    pub consensus_engine: ConsensusEngine,
}

impl QuantaConfig {
    fn default_engine() -> ConsensusEngine { ConsensusEngine::Bft }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeConfig {
    pub api_port: u16,
    pub network_port: u16,
    pub rpc_port: u16,
    pub db_path: String,
    pub no_network: bool,
    /// Node storage/sync mode: archive | pruned | light
    #[serde(default = "NodeConfig::default_mode")]
    pub mode: NodeMode,
    /// Prune window in days (only used when mode = pruned, default: 30)
    #[serde(default = "NodeConfig::default_prune_days")]
    pub prune_days: u64,
}

impl NodeConfig {
    fn default_mode() -> NodeMode { NodeMode::Archive }
    fn default_prune_days() -> u64 { 30 }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkConfig {
    pub max_peers: usize,
    pub bootstrap_nodes: Vec<String>,
    pub dns_seeds: Vec<String>,
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
                mode: NodeMode::Archive,
                prune_days: 30,
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

            security: SecurityConfig {
                max_mempool_size: 5000,
                transaction_expiry_seconds: 86400,
                enable_rate_limiting: true,  // PRODUCTION: Always enable
                rate_limit_per_minute: 60,   // 60 requests/min per IP
                enable_peer_banning: true,   // Auto-ban malicious peers
                require_tls: false,          // Set true for public nodes
            },

            metrics: MetricsConfig {
                enabled: true,
                port: 9090,
            },
            consensus_engine: ConsensusEngine::Bft,
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
    #[allow(dead_code)]
    pub fn save<P: AsRef<Path>>(&self, path: P) -> Result<(), std::io::Error> {
        let toml_string = toml::to_string_pretty(self)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
        std::fs::write(path, toml_string)
    }
    
    /// Validate configuration for sanity and safety
    #[allow(dead_code)]
    pub fn validate(&self) -> Result<(), String> {
        // Port conflicts
        if self.node.api_port == self.node.network_port {
            return Err("API port and network port must differ".into());
        }
        if self.node.api_port == self.metrics.port {
            return Err("API port and metrics port must differ".into());
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
    #[allow(dead_code)]
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

        tracing::info!("Security:");
        tracing::info!("  Max Mempool: {} txs", self.security.max_mempool_size);
        tracing::info!("Metrics:");
        tracing::info!("  Enabled: {}", self.metrics.enabled);
        tracing::info!("  Port: {}", self.metrics.port);
        tracing::info!("========================================");
    }
}
