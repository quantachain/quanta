use serde::{Deserialize, Serialize};
use std::path::Path;
use config::{Config, ConfigError, File};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuantaConfig {
    pub node: NodeConfig,
    pub network: NetworkConfig,
    pub security: SecurityConfig,
    pub mining: MiningConfig,
    pub metrics: MetricsConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeConfig {
    pub api_port: u16,
    pub network_port: u16,
    pub db_path: String,
    pub no_network: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkConfig {
    pub max_peers: usize,
    pub bootstrap_nodes: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityConfig {
    pub max_mempool_size: usize,
    pub max_block_transactions: usize,
    pub max_block_size_bytes: usize,
    pub min_transaction_fee: f64,
    pub transaction_expiry_seconds: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MiningConfig {
    pub initial_reward: f64,
    pub halving_interval: u64,
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
            node: NodeConfig {
                api_port: 3000,
                network_port: 8333,
                db_path: "./quanta_data".to_string(),
                no_network: false,
            },
            network: NetworkConfig {
                max_peers: 125,
                bootstrap_nodes: Vec::new(),
            },
            security: SecurityConfig {
                max_mempool_size: 5000,
                max_block_transactions: 2000,
                max_block_size_bytes: 1_048_576,
                min_transaction_fee: 0.0001,
                transaction_expiry_seconds: 86400,
            },
            mining: MiningConfig {
                initial_reward: 50.0,
                halving_interval: 210,
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
    /// Load configuration from file
    pub fn from_file<P: AsRef<Path>>(path: P) -> Result<Self, ConfigError> {
        let config = Config::builder()
            .add_source(File::from(path.as_ref()))
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

        Ok(config)
    }

    /// Save configuration to file
    pub fn save<P: AsRef<Path>>(&self, path: P) -> Result<(), std::io::Error> {
        let toml_string = toml::to_string_pretty(self)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
        std::fs::write(path, toml_string)
    }
}
