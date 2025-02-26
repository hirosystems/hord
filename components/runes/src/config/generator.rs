pub fn generate_config() -> String {
    let conf = format!(
        r#"
[postgres]
username = "postgres"
password = "postgres"
database = "postgres"
host = "localhost"
port = 5432

[network]
bitcoin_network = "mainnet"
bitcoind_rpc_url = "http://0.0.0.0:8332"
bitcoind_rpc_username = "user"
bitcoind_rpc_password = "pass"
bitcoind_zmq_url = "tcp://0.0.0.0:18543"

[resources]
lru_cache_size = 50000

[logs]
runes_internals = true
chainhook_internals = false
"#
    );
    return conf;
}
