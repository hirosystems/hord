use std::sync::mpsc::Sender;

use chainhook_sdk::{
    chainhooks::types::{BitcoinChainhookSpecification, ChainhookSpecification},
    observer::ObserverCommand,
    utils::Context,
};
use threadpool::ThreadPool;

use crate::{
    config::Config, scan::bitcoin::scan_bitcoin_chainstate_via_rpc_using_predicate, try_error,
    try_info,
};

use super::PgConnectionPools;

pub fn start_bitcoin_scan_runloop(
    config: &Config,
    bitcoin_scan_op_rx: crossbeam_channel::Receiver<BitcoinChainhookSpecification>,
    observer_command_tx: Sender<ObserverCommand>,
    pg_pools: &PgConnectionPools,
    ctx: &Context,
) {
    try_info!(ctx, "Starting bitcoin scan runloop");
    let bitcoin_scan_pool = ThreadPool::new(config.resources.expected_observers_count);
    while let Ok(predicate_spec) = bitcoin_scan_op_rx.recv() {
        let moved_ctx = ctx.clone();
        let moved_config = config.clone();
        let moved_pg_pools = pg_pools.clone();
        let observer_command_tx = observer_command_tx.clone();
        bitcoin_scan_pool.execute(move || {
            let op = scan_bitcoin_chainstate_via_rpc_using_predicate(
                &predicate_spec,
                &moved_config,
                None,
                &moved_pg_pools,
                &moved_ctx,
            );

            match hiro_system_kit::nestable_block_on(op) {
                Ok(_) => {}
                Err(e) => {
                    try_error!(
                        moved_ctx,
                        "Unable to evaluate predicate on Bitcoin chainstate: {e}",
                    );
                    return;
                }
            };
            let _ = observer_command_tx.send(ObserverCommand::EnablePredicate(
                ChainhookSpecification::Bitcoin(predicate_spec),
            ));
        });
    }
    let _ = bitcoin_scan_pool.join();
}
