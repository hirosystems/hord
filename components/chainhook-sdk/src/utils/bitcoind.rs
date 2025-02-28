use std::{thread::sleep, time::Duration};

use crate::utils::Context;
use bitcoincore_rpc::{Auth, Client, RpcApi};
use config::BitcoindConfig;
use hiro_system_kit::slog;

use crate::{try_error, try_info};

fn bitcoind_get_client(config: &BitcoindConfig, ctx: &Context) -> Client {
    loop {
        let auth = Auth::UserPass(config.rpc_username.clone(), config.rpc_password.clone());
        match Client::new(&config.rpc_url, auth) {
            Ok(con) => {
                return con;
            }
            Err(e) => {
                try_error!(ctx, "bitcoind: Unable to get client: {}", e.to_string());
                sleep(Duration::from_secs(1));
            }
        }
    }
}

/// Retrieves the block height from bitcoind.
pub fn bitcoind_get_block_height(config: &BitcoindConfig, ctx: &Context) -> u64 {
    let bitcoin_rpc = bitcoind_get_client(config, ctx);
    loop {
        match bitcoin_rpc.get_blockchain_info() {
            Ok(result) => {
                return result.blocks;
            }
            Err(e) => {
                try_error!(
                    ctx,
                    "bitcoind: Unable to get block height: {}",
                    e.to_string()
                );
                sleep(Duration::from_secs(1));
            }
        };
    }
}

/// Checks if bitcoind is still synchronizing blocks and waits until it's finished if that is the case.
pub fn bitcoind_wait_for_chain_tip(config: &BitcoindConfig, ctx: &Context) {
    let bitcoin_rpc = bitcoind_get_client(config, ctx);
    let mut confirmations = 0;
    loop {
        match bitcoin_rpc.get_blockchain_info() {
            Ok(result) => {
                if result.initial_block_download == false && result.blocks == result.headers {
                    confirmations += 1;
                    // Wait for 10 confirmations before declaring node is at chain tip, just in case it's still connecting to
                    // peers.
                    if confirmations == 10 {
                        try_info!(ctx, "bitcoind: Chain tip reached");
                        return;
                    }
                    try_info!(ctx, "bitcoind: Verifying chain tip");
                } else {
                    confirmations = 0;
                    try_info!(
                        ctx,
                        "bitcoind: Node has not reached chain tip, trying again"
                    );
                }
            }
            Err(e) => {
                try_error!(
                    ctx,
                    "bitcoind: Unable to check for chain tip: {}",
                    e.to_string()
                );
            }
        };
        sleep(Duration::from_secs(1));
    }
}
