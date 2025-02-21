pub use chainhook_postgres::PgConnectionConfig;
use chainhook_sdk::{indexer::IndexerConfig, observer::EventObserverConfig};
use chainhook_types::{BitcoinBlockSignaling, BitcoinNetwork};
use std::path::PathBuf;

const DEFAULT_MAINNET_ORDINALS_SQLITE_ARCHIVE: &str =
    "https://archive.hiro.so/mainnet/ordhook/mainnet-ordhook-sqlite-latest";
const DEFAULT_MAINNET_BRC20_SQLITE_ARCHIVE: &str =
    "https://archive.hiro.so/mainnet/ordhook/mainnet-ordhook-brc20-latest";

pub const DEFAULT_INGESTION_PORT: u16 = 20455;
pub const DEFAULT_ULIMIT: usize = 2048;
pub const DEFAULT_MEMORY_AVAILABLE: usize = 8;
pub const DEFAULT_BITCOIND_RPC_THREADS: usize = 4;
pub const DEFAULT_BITCOIND_RPC_TIMEOUT: u32 = 15;
pub const DEFAULT_BRC20_LRU_CACHE_SIZE: usize = 50_000;

// TODO: move this one layer above runes and ordinals 
// as it includes the configs for both 
#[derive(Clone, Debug)]
pub struct Config {
    pub storage: StorageConfig,
    pub ordinals_db: Option<PgConnectionConfig>,
    pub brc20_db: Option<PgConnectionConfig>,
    pub runes_db: Option<PgConnectionConfig>,
    pub resources: ResourcesConfig,
    pub network: IndexerConfig,
    pub snapshot: SnapshotConfig,
    pub meta_protocols: MetaProtocolsConfig,
    pub logs: LogConfig,
}

#[derive(Clone, Debug)]
pub struct MetaProtocolsConfig {
    pub brc20: bool,
}

#[derive(Clone, Debug)]
pub struct LogConfig {
    pub ordinals_internals: bool,
    pub chainhook_internals: bool,
}

#[derive(Clone, Debug)]
pub struct StorageConfig {
    pub working_dir: String,
    pub observers_working_dir: String,
}

#[derive(Clone, Debug)]
pub struct SnapshotConfigDownloadUrls {
    pub ordinals: String,
    pub brc20: Option<String>,
}

#[derive(Clone, Debug)]
pub enum SnapshotConfig {
    Build,
    Download(SnapshotConfigDownloadUrls),
}

#[derive(Clone, Debug)]
pub struct PathConfig {
    pub file_path: PathBuf,
}

#[derive(Clone, Debug)]
pub struct UrlConfig {
    pub file_url: String,
}

#[derive(Deserialize, Debug, Clone)]
pub struct ResourcesConfig {
    pub ulimit: usize,
    pub cpu_core_available: usize,
    pub memory_available: usize,
    pub bitcoind_rpc_threads: usize,
    pub bitcoind_rpc_timeout: u32,
    pub expected_observers_count: usize,
    pub brc20_lru_cache_size: usize,
}

impl ResourcesConfig {
    pub fn get_optimal_thread_pool_capacity(&self) -> usize {
        // Generally speaking when dealing a pool, we need one thread for
        // feeding the thread pool and eventually another thread for
        // handling the "reduce" step.
        self.cpu_core_available.saturating_sub(2).max(1)
    }
}

impl Config {
    pub fn get_event_observer_config(&self) -> EventObserverConfig {
        EventObserverConfig {
            bitcoind_rpc_username: self.network.bitcoind_rpc_username.clone(),
            bitcoind_rpc_password: self.network.bitcoind_rpc_password.clone(),
            bitcoind_rpc_url: self.network.bitcoind_rpc_url.clone(),
            bitcoin_block_signaling: self.network.bitcoin_block_signaling.clone(),
            bitcoin_network: self.network.bitcoin_network.clone(),
        }
    }

    pub fn should_bootstrap_through_download(&self) -> bool {
        match &self.snapshot {
            SnapshotConfig::Build => false,
            SnapshotConfig::Download(_) => true,
        }
    }

    pub fn expected_cache_path(&self) -> PathBuf {
        let mut destination_path = PathBuf::new();
        destination_path.push(&self.storage.working_dir);
        destination_path
    }

    pub fn expected_observers_cache_path(&self) -> PathBuf {
        let mut destination_path = PathBuf::new();
        destination_path.push(&self.storage.observers_working_dir);
        destination_path
    }

    pub fn devnet_ordinals_default() -> Config {
        Config {
            storage: StorageConfig {
                working_dir: default_cache_path(),
                observers_working_dir: default_observers_cache_path(),
            },
            ordinals_db: Some(PgConnectionConfig {
                dbname: "ordinals".to_string(),
                host: "localhost".to_string(),
                port: 5432,
                user: "postgres".to_string(),
                password: Some("postgres".to_string()),
                search_path: None,
                pool_max_size: None,
            }),
            brc20_db: None,
            runes_db: None,
            snapshot: SnapshotConfig::Build,
            resources: ResourcesConfig {
                cpu_core_available: num_cpus::get(),
                memory_available: DEFAULT_MEMORY_AVAILABLE,
                ulimit: DEFAULT_ULIMIT,
                bitcoind_rpc_threads: DEFAULT_BITCOIND_RPC_THREADS,
                bitcoind_rpc_timeout: DEFAULT_BITCOIND_RPC_TIMEOUT,
                expected_observers_count: 1,
                brc20_lru_cache_size: DEFAULT_BRC20_LRU_CACHE_SIZE,
            },
            network: IndexerConfig {
                bitcoind_rpc_url: "http://0.0.0.0:18443".into(),
                bitcoind_rpc_username: "devnet".into(),
                bitcoind_rpc_password: "devnet".into(),
                bitcoin_block_signaling: BitcoinBlockSignaling::ZeroMQ(
                    "http://0.0.0.0:18543".into(),
                ),
                bitcoin_network: BitcoinNetwork::Regtest,
                prometheus_monitoring_port: None,
            },
            logs: LogConfig {
                ordinals_internals: true,
                chainhook_internals: false,
            },
            meta_protocols: MetaProtocolsConfig { brc20: false },
        }
    }

    // TODO: update after integrating runes 
    pub fn devnet_runes_default() -> Config {
        Config {
            storage: StorageConfig {
                working_dir: default_cache_path(),
                observers_working_dir: default_observers_cache_path(),
            },
            ordinals_db: None,
            brc20_db: None,
            runes_db: None, 
            snapshot: SnapshotConfig::Build,
            resources: ResourcesConfig {
                cpu_core_available: num_cpus::get(),
                memory_available: DEFAULT_MEMORY_AVAILABLE,
                ulimit: DEFAULT_ULIMIT,
                bitcoind_rpc_threads: DEFAULT_BITCOIND_RPC_THREADS,
                bitcoind_rpc_timeout: DEFAULT_BITCOIND_RPC_TIMEOUT,
                expected_observers_count: 1,
                brc20_lru_cache_size: DEFAULT_BRC20_LRU_CACHE_SIZE,
            },
            network: IndexerConfig {
                bitcoind_rpc_url: "http://0.0.0.0:18443".into(),
                bitcoind_rpc_username: "devnet".into(),
                bitcoind_rpc_password: "devnet".into(),
                bitcoin_block_signaling: BitcoinBlockSignaling::ZeroMQ(
                    "http://0.0.0.0:18543".into(),
                ),
                bitcoin_network: BitcoinNetwork::Regtest,
                prometheus_monitoring_port: None,
            },
            logs: LogConfig {
                ordinals_internals: true,
                chainhook_internals: false,
            },
            meta_protocols: MetaProtocolsConfig { brc20: false },
        }
    }

    // TODO: update after integrating runes 
    pub fn devnet_ordinals_and_runes_default() -> Config {
        Config {
            storage: StorageConfig {
                working_dir: default_cache_path(),
                observers_working_dir: default_observers_cache_path(),
            },
            ordinals_db: Some(PgConnectionConfig {
                dbname: "ordinals".to_string(),
                host: "localhost".to_string(),
                port: 5432,
                user: "postgres".to_string(),
                password: Some("postgres".to_string()),
                search_path: None,
                pool_max_size: None,
            }),
            brc20_db: None,
            runes_db: None,
            snapshot: SnapshotConfig::Build,
            resources: ResourcesConfig {
                cpu_core_available: num_cpus::get(),
                memory_available: DEFAULT_MEMORY_AVAILABLE,
                ulimit: DEFAULT_ULIMIT,
                bitcoind_rpc_threads: DEFAULT_BITCOIND_RPC_THREADS,
                bitcoind_rpc_timeout: DEFAULT_BITCOIND_RPC_TIMEOUT,
                expected_observers_count: 1,
                brc20_lru_cache_size: DEFAULT_BRC20_LRU_CACHE_SIZE,
            },
            network: IndexerConfig {
                bitcoind_rpc_url: "http://0.0.0.0:18443".into(),
                bitcoind_rpc_username: "devnet".into(),
                bitcoind_rpc_password: "devnet".into(),
                bitcoin_block_signaling: BitcoinBlockSignaling::ZeroMQ(
                    "http://0.0.0.0:18543".into(),
                ),
                bitcoin_network: BitcoinNetwork::Regtest,
                prometheus_monitoring_port: None,
            },
            logs: LogConfig {
                ordinals_internals: true,
                chainhook_internals: false,
            },
            meta_protocols: MetaProtocolsConfig { brc20: false },
        }
    }

    pub fn testnet_ordinals_default() -> Config {
        Config {
            storage: StorageConfig {
                working_dir: default_cache_path(),
                observers_working_dir: default_observers_cache_path(),
            },
            ordinals_db: Some(PgConnectionConfig {
                dbname: "ordinals".to_string(),
                host: "localhost".to_string(),
                port: 5432,
                user: "postgres".to_string(),
                password: Some("postgres".to_string()),
                search_path: None,
                pool_max_size: None,
            }),
            brc20_db: None,
            runes_db: None,
            snapshot: SnapshotConfig::Build,
            resources: ResourcesConfig {
                cpu_core_available: num_cpus::get(),
                memory_available: DEFAULT_MEMORY_AVAILABLE,
                ulimit: DEFAULT_ULIMIT,
                bitcoind_rpc_threads: DEFAULT_BITCOIND_RPC_THREADS,
                bitcoind_rpc_timeout: DEFAULT_BITCOIND_RPC_TIMEOUT,
                expected_observers_count: 1,
                brc20_lru_cache_size: DEFAULT_BRC20_LRU_CACHE_SIZE,
            },
            network: IndexerConfig {
                bitcoind_rpc_url: "http://0.0.0.0:18332".into(),
                bitcoind_rpc_username: "devnet".into(),
                bitcoind_rpc_password: "devnet".into(),
                bitcoin_block_signaling: BitcoinBlockSignaling::ZeroMQ(
                    "http://0.0.0.0:18543".into(),
                ),
                bitcoin_network: BitcoinNetwork::Testnet,
                prometheus_monitoring_port: Some(9153),
            },
            logs: LogConfig {
                ordinals_internals: true,
                chainhook_internals: false,
            },
            meta_protocols: MetaProtocolsConfig { brc20: false },
        }
    }

    // TODO: update after integrating runes 
    pub fn testnet_runes_default() -> Config {
        Config {
            storage: StorageConfig {
                working_dir: default_cache_path(),
                observers_working_dir: default_observers_cache_path(),
            },
            ordinals_db: None,
            brc20_db: None,
            runes_db: None,
            snapshot: SnapshotConfig::Build,
            resources: ResourcesConfig {
                cpu_core_available: num_cpus::get(),
                memory_available: DEFAULT_MEMORY_AVAILABLE,
                ulimit: DEFAULT_ULIMIT,
                bitcoind_rpc_threads: DEFAULT_BITCOIND_RPC_THREADS,
                bitcoind_rpc_timeout: DEFAULT_BITCOIND_RPC_TIMEOUT,
                expected_observers_count: 1,
                brc20_lru_cache_size: DEFAULT_BRC20_LRU_CACHE_SIZE,
            },
            network: IndexerConfig {
                bitcoind_rpc_url: "http://0.0.0.0:18332".into(),
                bitcoind_rpc_username: "devnet".into(),
                bitcoind_rpc_password: "devnet".into(),
                bitcoin_block_signaling: BitcoinBlockSignaling::ZeroMQ(
                    "http://0.0.0.0:18543".into(),
                ),
                bitcoin_network: BitcoinNetwork::Testnet,
                prometheus_monitoring_port: Some(9153),
            },
            logs: LogConfig {
                ordinals_internals: true,
                chainhook_internals: false,
            },
            meta_protocols: MetaProtocolsConfig { brc20: false },
        }
    }

    // TODO: update after integrating runes 
    pub fn testnet_ordinals_and_runes_default() -> Config {
        Config {
            storage: StorageConfig {
                working_dir: default_cache_path(),
                observers_working_dir: default_observers_cache_path(),
            },
            ordinals_db: Some(PgConnectionConfig {
                dbname: "ordinals".to_string(),
                host: "localhost".to_string(),
                port: 5432,
                user: "postgres".to_string(),
                password: Some("postgres".to_string()),
                search_path: None,
                pool_max_size: None,
            }),
            brc20_db: None,
            runes_db: None,
            snapshot: SnapshotConfig::Build,
            resources: ResourcesConfig {
                cpu_core_available: num_cpus::get(),
                memory_available: DEFAULT_MEMORY_AVAILABLE,
                ulimit: DEFAULT_ULIMIT,
                bitcoind_rpc_threads: DEFAULT_BITCOIND_RPC_THREADS,
                bitcoind_rpc_timeout: DEFAULT_BITCOIND_RPC_TIMEOUT,
                expected_observers_count: 1,
                brc20_lru_cache_size: DEFAULT_BRC20_LRU_CACHE_SIZE,
            },
            network: IndexerConfig {
                bitcoind_rpc_url: "http://0.0.0.0:18332".into(),
                bitcoind_rpc_username: "devnet".into(),
                bitcoind_rpc_password: "devnet".into(),
                bitcoin_block_signaling: BitcoinBlockSignaling::ZeroMQ(
                    "http://0.0.0.0:18543".into(),
                ),
                bitcoin_network: BitcoinNetwork::Testnet,
                prometheus_monitoring_port: Some(9153),
            },
            logs: LogConfig {
                ordinals_internals: true,
                chainhook_internals: false,
            },
            meta_protocols: MetaProtocolsConfig { brc20: false },
        }
    }

    pub fn mainnet_ordinals_default() -> Config {
        Config {
            storage: StorageConfig {
                working_dir: default_cache_path(),
                observers_working_dir: default_observers_cache_path(),
            },
            ordinals_db: Some(PgConnectionConfig {
                dbname: "ordinals".to_string(),
                host: "localhost".to_string(),
                port: 5432,
                user: "postgres".to_string(),
                password: Some("postgres".to_string()),
                search_path: None,
                pool_max_size: None,
            }),
            brc20_db: None,
            runes_db: None,
            snapshot: SnapshotConfig::Download(SnapshotConfigDownloadUrls {
                ordinals: DEFAULT_MAINNET_ORDINALS_SQLITE_ARCHIVE.to_string(),
                brc20: Some(DEFAULT_MAINNET_BRC20_SQLITE_ARCHIVE.to_string()),
            }),
            resources: ResourcesConfig {
                cpu_core_available: num_cpus::get(),
                memory_available: DEFAULT_MEMORY_AVAILABLE,
                ulimit: DEFAULT_ULIMIT,
                bitcoind_rpc_threads: DEFAULT_BITCOIND_RPC_THREADS,
                bitcoind_rpc_timeout: DEFAULT_BITCOIND_RPC_TIMEOUT,
                expected_observers_count: 1,
                brc20_lru_cache_size: DEFAULT_BRC20_LRU_CACHE_SIZE,
            },
            network: IndexerConfig {
                bitcoind_rpc_url: "http://0.0.0.0:8332".into(),
                bitcoind_rpc_username: "devnet".into(),
                bitcoind_rpc_password: "devnet".into(),
                bitcoin_block_signaling: BitcoinBlockSignaling::ZeroMQ(
                    "http://0.0.0.0:18543".into(),
                ),
                bitcoin_network: BitcoinNetwork::Mainnet,
                prometheus_monitoring_port: Some(9153),
            },
            logs: LogConfig {
                ordinals_internals: true,
                chainhook_internals: false,
            },
            meta_protocols: MetaProtocolsConfig { brc20: false },
        }
    }


    // TODO: update after integrating runes 
    pub fn mainnet_runes_default() -> Config {
        Config {
            storage: StorageConfig {
                working_dir: default_cache_path(),
                observers_working_dir: default_observers_cache_path(),
            },
            ordinals_db: None,
            brc20_db: None,
            runes_db: None,
            snapshot: SnapshotConfig::Download(SnapshotConfigDownloadUrls {
                ordinals: DEFAULT_MAINNET_ORDINALS_SQLITE_ARCHIVE.to_string(),
                brc20: Some(DEFAULT_MAINNET_BRC20_SQLITE_ARCHIVE.to_string()),
            }),
            resources: ResourcesConfig {
                cpu_core_available: num_cpus::get(),
                memory_available: DEFAULT_MEMORY_AVAILABLE,
                ulimit: DEFAULT_ULIMIT,
                bitcoind_rpc_threads: DEFAULT_BITCOIND_RPC_THREADS,
                bitcoind_rpc_timeout: DEFAULT_BITCOIND_RPC_TIMEOUT,
                expected_observers_count: 1,
                brc20_lru_cache_size: DEFAULT_BRC20_LRU_CACHE_SIZE,
            },
            network: IndexerConfig {
                bitcoind_rpc_url: "http://0.0.0.0:8332".into(),
                bitcoind_rpc_username: "devnet".into(),
                bitcoind_rpc_password: "devnet".into(),
                bitcoin_block_signaling: BitcoinBlockSignaling::ZeroMQ(
                    "http://0.0.0.0:18543".into(),
                ),
                bitcoin_network: BitcoinNetwork::Mainnet,
                prometheus_monitoring_port: Some(9153),
            },
            logs: LogConfig {
                ordinals_internals: true,
                chainhook_internals: false,
            },
            meta_protocols: MetaProtocolsConfig { brc20: false },
        }
    }

    // TODO: update after integrating runes 
    pub fn mainnet_ordinals_and_runes_default() -> Config {
        Config {
            storage: StorageConfig {
                working_dir: default_cache_path(),
                observers_working_dir: default_observers_cache_path(),
            },
            ordinals_db: Some(PgConnectionConfig {
                dbname: "ordinals".to_string(),
                host: "localhost".to_string(),
                port: 5432,
                user: "postgres".to_string(),
                password: Some("postgres".to_string()),
                search_path: None,
                pool_max_size: None,
            }),
            brc20_db: None,
            runes_db: None,
            snapshot: SnapshotConfig::Download(SnapshotConfigDownloadUrls {
                ordinals: DEFAULT_MAINNET_ORDINALS_SQLITE_ARCHIVE.to_string(),
                brc20: Some(DEFAULT_MAINNET_BRC20_SQLITE_ARCHIVE.to_string()),
            }),
            resources: ResourcesConfig {
                cpu_core_available: num_cpus::get(),
                memory_available: DEFAULT_MEMORY_AVAILABLE,
                ulimit: DEFAULT_ULIMIT,
                bitcoind_rpc_threads: DEFAULT_BITCOIND_RPC_THREADS,
                bitcoind_rpc_timeout: DEFAULT_BITCOIND_RPC_TIMEOUT,
                expected_observers_count: 1,
                brc20_lru_cache_size: DEFAULT_BRC20_LRU_CACHE_SIZE,
            },
            network: IndexerConfig {
                bitcoind_rpc_url: "http://0.0.0.0:8332".into(),
                bitcoind_rpc_username: "devnet".into(),
                bitcoind_rpc_password: "devnet".into(),
                bitcoin_block_signaling: BitcoinBlockSignaling::ZeroMQ(
                    "http://0.0.0.0:18543".into(),
                ),
                bitcoin_network: BitcoinNetwork::Mainnet,
                prometheus_monitoring_port: Some(9153),
            },
            logs: LogConfig {
                ordinals_internals: true,
                chainhook_internals: false,
            },
            meta_protocols: MetaProtocolsConfig { brc20: false },
        }
    }

    #[cfg(test)]
    pub fn test_default() -> Config {
        let mut config = Self::mainnet_ordinals_default();
        config.storage.working_dir = "tmp".to_string();
        config.resources.bitcoind_rpc_threads = 1;
        config.resources.cpu_core_available = 1;
        config
    }
}

pub fn default_cache_path() -> String {
    let mut cache_path = std::env::current_dir().expect("unable to get current dir");
    cache_path.push("ordhook");
    format!("{}", cache_path.display())
}

pub fn default_observers_cache_path() -> String {
    let mut cache_path = std::env::current_dir().expect("unable to get current dir");
    cache_path.push("observers");
    format!("{}", cache_path.display())
}
