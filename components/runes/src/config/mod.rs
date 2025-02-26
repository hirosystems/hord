pub mod file;
pub mod generator;

use bitcoin::Network;
use chainhook_sdk::observer::EventObserverConfig;

use chainhook_sdk::types::BitcoinNetwork;
use file::ConfigFile;
use std::fs::File;
use std::io::{BufReader, Read};

#[derive(Clone, Debug)]
pub struct PostgresConfig {
    pub database: String,
    pub host: String,
    pub port: u16,
    pub username: String,
    pub password: Option<String>,
}

#[derive(Clone, Debug)]
pub struct ResourcesConfig {
    pub lru_cache_size: usize,
}

#[derive(Clone, Debug)]
pub struct Config {
    pub event_observer: EventObserverConfig,
    pub postgres: PostgresConfig,
    pub resources: ResourcesConfig,
}

impl Config {
    pub fn from_file_path(file_path: &str) -> Result<Config, String> {
        let file = File::open(file_path)
            .map_err(|e| format!("unable to read file {}\n{:?}", file_path, e))?;
        let mut file_reader = BufReader::new(file);
        let mut file_buffer = vec![];
        file_reader
            .read_to_end(&mut file_buffer)
            .map_err(|e| format!("unable to read file {}\n{:?}", file_path, e))?;

        let config_file: ConfigFile = match toml::from_slice(&file_buffer) {
            Ok(s) => s,
            Err(e) => {
                return Err(format!("Config file malformatted {}", e.to_string()));
            }
        };
        Config::from_config_file(config_file)
    }

    pub fn from_config_file(config_file: ConfigFile) -> Result<Config, String> {
        let event_observer =
            EventObserverConfig::new_using_overrides(config_file.network.as_ref())?;

        let config = Config {
            event_observer,
            postgres: PostgresConfig {
                database: config_file
                    .postgres
                    .database
                    .unwrap_or("postgres".to_string()),
                host: config_file.postgres.host.unwrap_or("localhost".to_string()),
                port: config_file.postgres.port.unwrap_or(5432),
                username: config_file
                    .postgres
                    .username
                    .unwrap_or("postgres".to_string()),
                password: config_file.postgres.password,
            },
            resources: ResourcesConfig {
                lru_cache_size: config_file.resources.lru_cache_size.unwrap_or(10_000),
            },
        };
        Ok(config)
    }

    pub fn get_bitcoin_network(&self) -> Network {
        match self.event_observer.bitcoin_network {
            BitcoinNetwork::Mainnet => Network::Bitcoin,
            BitcoinNetwork::Regtest => Network::Regtest,
            BitcoinNetwork::Testnet => Network::Testnet,
            BitcoinNetwork::Signet => Network::Signet,
        }
    }
}
