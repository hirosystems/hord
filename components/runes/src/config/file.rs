use chainhook_sdk::observer::EventObserverConfigOverrides;

#[derive(Deserialize, Debug, Clone)]
pub struct ConfigFile {
    pub network: Option<EventObserverConfigOverrides>,
    pub postgres: PostgresConfigFile,
    pub resources: ResourcesConfigFile,
}
#[derive(Deserialize, Debug, Clone)]
pub struct LogConfigFile {
    pub runes_internals: Option<bool>,
    pub chainhook_internals: Option<bool>,
}

#[derive(Deserialize, Debug, Clone)]
pub struct PostgresConfigFile {
    pub database: Option<String>,
    pub host: Option<String>,
    pub port: Option<u16>,
    pub username: Option<String>,
    pub password: Option<String>,
}

#[derive(Deserialize, Debug, Clone)]
pub struct PredicatesApiConfigFile {
    pub http_port: Option<u16>,
    pub database_uri: Option<String>,
    pub display_logs: Option<bool>,
    pub disabled: Option<bool>,
}

#[derive(Deserialize, Debug, Clone)]
pub struct ResourcesConfigFile {
    pub lru_cache_size: Option<usize>,
}
