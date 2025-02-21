use crate::config::Config;
use crate::core::meta_protocols::brc20::cache::{brc20_new_cache, Brc20MemoryCache};
use crate::core::pipeline::bitcoind_download_blocks;
use crate::core::pipeline::processors::block_archiving::start_block_archiving_processor;
use crate::core::pipeline::processors::inscription_indexing::{
    index_block, rollback_block, start_inscription_indexing_processor,
};
use crate::core::protocol::sequence_cursor::SequenceCursor;
use crate::core::{
    first_inscription_height, new_traversals_lazy_cache, should_sync_ordinals_db,
    should_sync_rocks_db,
};
use crate::db::blocks::{
    self, find_missing_blocks, open_blocks_db_with_retry, run_compaction,
};
use crate::db::cursor::{BlockBytesCursor, TransactionBytesCursor};
use crate::db::ordinals_pg;
use crate::utils::monitoring::{start_serving_prometheus_metrics, PrometheusMonitoring};
use crate::{try_crit, try_error, try_info};
use chainhook_postgres::{pg_begin, pg_pool, pg_pool_client};
use chainhook_sdk::observer::{
    start_event_observer, BitcoinBlockDataCached, ObserverEvent, ObserverSidecar,
};
use chainhook_sdk::utils::bitcoind::bitcoind_wait_for_chain_tip;
use chainhook_sdk::utils::{BlockHeights, Context};
use chainhook_types::BlockIdentifier;
use crossbeam_channel::select;
use dashmap::DashMap;
use deadpool_postgres::Pool;
use fxhash::FxHasher;

use std::collections::BTreeMap;
use std::hash::BuildHasherDefault;
use std::sync::mpsc::channel;
use std::sync::Arc;

#[derive(Debug, Clone)]
pub struct PgConnectionPools {
    pub ordinals: Pool,
    pub brc20: Option<Pool>,
}

pub struct Service {
    pub prometheus: PrometheusMonitoring,
    pub config: Config,
    pub ctx: Context,
    pub pg_pools: PgConnectionPools,
}

impl Service {
    pub fn new(config: &Config, ctx: &Context) -> Self {
        Self {
            prometheus: PrometheusMonitoring::new(),
            config: config.clone(),
            ctx: ctx.clone(),
            pg_pools: PgConnectionPools {
                ordinals: pg_pool(&config.ordinals_db.clone().unwrap()).unwrap(),
                brc20: match (config.meta_protocols.brc20, &config.brc20_db) {
                    (true, Some(brc20_db)) => Some(pg_pool(&brc20_db).unwrap()),
                    _ => None,
                },
            },
        }
    }

    /// Returns the last block height we have indexed. This only looks at the max index chain tip, not at the blocks DB chain tip.
    /// Adjusts for starting index height depending on Bitcoin network.
    pub async fn get_index_chain_tip(&self) -> Result<u64, String> {
        let mut ord_client = pg_pool_client(&self.pg_pools.ordinals).await?;
        let ord_tx = pg_begin(&mut ord_client).await?;

        // Update chain tip to match first inscription height at least.
        let db_height = ordinals_pg::get_chain_tip_block_height(&ord_tx)
            .await?
            .unwrap_or(0)
            .max(first_inscription_height(&self.config) - 1);
        ordinals_pg::update_chain_tip(db_height, &ord_tx).await?;

        ord_tx
            .commit()
            .await
            .map_err(|e| format!("unable to commit get_index_chain_tip transaction: {e}"))?;
        Ok(db_height)
    }

    pub async fn run(&mut self, check_blocks_integrity: bool) -> Result<(), String> {
        // 1: Initialize Prometheus monitoring server.
        if let Some(port) = self.config.network.prometheus_monitoring_port {
            let registry_moved = self.prometheus.registry.clone();
            let ctx_cloned = self.ctx.clone();
            let _ = std::thread::spawn(move || {
                let _ = hiro_system_kit::nestable_block_on(start_serving_prometheus_metrics(
                    port,
                    registry_moved,
                    ctx_cloned,
                ));
            });
        }
        let (max_inscription_number, chain_tip) = {
            let ord_client = pg_pool_client(&self.pg_pools.ordinals).await?;

            let inscription_number = ordinals_pg::get_highest_inscription_number(&ord_client)
                .await?
                .unwrap_or(0);
            let chain_tip = ordinals_pg::get_chain_tip_block_height(&ord_client)
                .await?
                .unwrap_or(0);

            (inscription_number, chain_tip)
        };
        self.prometheus
            .initialize(0, max_inscription_number as u64, chain_tip);

        // 2: Catch-up the ordinals index to Bitcoin chain tip.
        if check_blocks_integrity {
            self.check_blocks_db_integrity().await?;
        }
        self.catch_up_to_bitcoin_chain_tip().await?;
        try_info!(self.ctx, "Service: Streaming blocks start");

        // 3: Set up the real-time ZMQ Bitcoin block streaming channels and start listening.
        let zmq_observer_sidecar = self.set_up_bitcoin_zmq_observer_sidecar()?;
        let (observer_command_tx, observer_command_rx) = channel();
        let (observer_event_tx, observer_event_rx) = crossbeam_channel::unbounded();
        let inner_ctx = if self.config.logs.chainhook_internals {
            self.ctx.clone()
        } else {
            Context::empty()
        };

        let event_observer_config = self.config.get_event_observer_config();
        let _ = start_event_observer(
            event_observer_config,
            observer_command_tx.clone(),
            observer_command_rx,
            Some(observer_event_tx),
            Some(zmq_observer_sidecar),
            inner_ctx,
        );

        // 4: Block the main thread.
        loop {
            let event = match observer_event_rx.recv() {
                Ok(cmd) => cmd,
                Err(e) => {
                    try_error!(self.ctx, "Error: broken channel {}", e.to_string());
                    break;
                }
            };
            match event {
                ObserverEvent::Terminate => {
                    try_info!(&self.ctx, "Terminating runloop");
                    break;
                }
                _ => {}
            }
        }
        Ok(())
    }

    /// Rolls back index data for the specified block heights.
    pub async fn rollback(&self, block_heights: &Vec<u64>) -> Result<(), String> {
        for block_height in block_heights.iter() {
            rollback_block(*block_height, &self.config, &self.pg_pools, &self.ctx).await?;
        }
        Ok(())
    }

    fn set_up_bitcoin_zmq_observer_sidecar(&self) -> Result<ObserverSidecar, String> {
        let (block_mutator_in_tx, block_mutator_in_rx) = crossbeam_channel::unbounded();
        let (block_mutator_out_tx, block_mutator_out_rx) = crossbeam_channel::unbounded();
        let (chain_event_notifier_tx, chain_event_notifier_rx) = crossbeam_channel::unbounded();
        let observer_sidecar = ObserverSidecar {
            bitcoin_blocks_mutator: Some((block_mutator_in_tx, block_mutator_out_rx)),
            bitcoin_chain_event_notifier: Some(chain_event_notifier_tx),
        };
        // TODO(rafaelcr): Move these outside so they can be used across blocks.
        let cache_l2 = Arc::new(new_traversals_lazy_cache(100_000));
        let mut brc20_cache = brc20_new_cache(&self.config);
        let ctx = self.ctx.clone();
        let config = self.config.clone();
        let pg_pools = self.pg_pools.clone();
        let prometheus = self.prometheus.clone();

        hiro_system_kit::thread_named("Observer Sidecar Runloop")
            .spawn(move || {
                hiro_system_kit::nestable_block_on(async move {
                    loop {
                        select! {
                            // Mutate a newly-received Bitcoin block and add any Ordinals or BRC-20 activity to it. Write index
                            // data to DB.
                            recv(block_mutator_in_rx) -> msg => {
                                if let Ok((mut blocks_to_mutate, blocks_ids_to_rollback)) = msg {
                                    match chainhook_sidecar_mutate_blocks(
                                        &mut blocks_to_mutate,
                                        &blocks_ids_to_rollback,
                                        &cache_l2,
                                        &mut brc20_cache,
                                        &prometheus,
                                        &config,
                                        &pg_pools,
                                        &ctx,
                                    ).await {
                                        Ok(_) => {
                                            let _ = block_mutator_out_tx.send(blocks_to_mutate);
                                        },
                                        Err(e) => {
                                            try_crit!(ctx, "Error indexing streamed block: {e}");
                                            std::process::exit(1);
                                        },
                                    };
                                }
                            }
                            recv(chain_event_notifier_rx) -> _msg => {
                                // No action required.
                            }
                        }
                    }
                })
            })
            .expect("unable to spawn zmq thread");

        Ok(observer_sidecar)
    }

    pub async fn check_blocks_db_integrity(&mut self) -> Result<(), String> {
        bitcoind_wait_for_chain_tip(&self.config.network, &self.ctx);
        let (tip, missing_blocks) = {
            let blocks_db = open_blocks_db_with_retry(false, &self.config, &self.ctx);
            let ord_client = pg_pool_client(&self.pg_pools.ordinals).await?;

            let tip = ordinals_pg::get_chain_tip_block_height(&ord_client)
                .await?
                .unwrap_or(0);
            let missing_blocks = find_missing_blocks(&blocks_db, 0, tip as u32, &self.ctx);

            (tip, missing_blocks)
        };
        if !missing_blocks.is_empty() {
            info!(
                self.ctx.expect_logger(),
                "{} missing blocks detected, will attempt to repair data",
                missing_blocks.len()
            );
            let block_ingestion_processor =
                start_block_archiving_processor(&self.config, &self.ctx, false, None);
            bitcoind_download_blocks(
                &self.config,
                missing_blocks.into_iter().map(|x| x as u64).collect(),
                tip.into(),
                &block_ingestion_processor,
                10_000,
                &self.ctx,
            )
            .await?;
        }
        let blocks_db_rw = open_blocks_db_with_retry(false, &self.config, &self.ctx);
        info!(self.ctx.expect_logger(), "Running database compaction",);
        run_compaction(&blocks_db_rw, tip as u32);
        Ok(())
    }

    /// Synchronizes and indexes all databases until their block height matches bitcoind's block height.
    pub async fn catch_up_to_bitcoin_chain_tip(&self) -> Result<(), String> {
        // 0: Make sure bitcoind is synchronized.
        bitcoind_wait_for_chain_tip(&self.config.network, &self.ctx);

        // 1: Catch up blocks DB so it is at least at the same height as the ordinals DB.
        if let Some((start_block, end_block)) =
            should_sync_rocks_db(&self.config, &self.pg_pools, &self.ctx).await?
        {
            try_info!(
                self.ctx,
                "Blocks DB is out of sync with ordinals DB, archiving blocks from #{start_block} to #{end_block}"
            );
            let blocks_post_processor =
                start_block_archiving_processor(&self.config, &self.ctx, true, None);
            let blocks = BlockHeights::BlockRange(start_block, end_block)
                .get_sorted_entries()
                .map_err(|_e| format!("Block start / end block spec invalid"))?;
            bitcoind_download_blocks(
                &self.config,
                blocks.into(),
                first_inscription_height(&self.config),
                &blocks_post_processor,
                10_000,
                &self.ctx,
            )
            .await?;
        }

        // 2: Catch up ordinals DB until it reaches bitcoind block height. This will also advance blocks DB and BRC-20 DB if
        // enabled.
        let mut last_block_processed = 0;
        while let Some((start_block, end_block, speed)) =
            should_sync_ordinals_db(&self.config, &self.pg_pools, &self.ctx).await?
        {
            if last_block_processed == end_block {
                break;
            }
            let blocks_post_processor = start_inscription_indexing_processor(
                &self.config,
                &self.pg_pools,
                &self.ctx,
                &self.prometheus,
            );
            try_info!(
                self.ctx,
                "Indexing inscriptions from #{start_block} to #{end_block}"
            );
            let blocks = BlockHeights::BlockRange(start_block, end_block)
                .get_sorted_entries()
                .map_err(|_e| format!("Block start / end block spec invalid"))?;
            bitcoind_download_blocks(
                &self.config,
                blocks.into(),
                first_inscription_height(&self.config),
                &blocks_post_processor,
                speed,
                &self.ctx,
            )
            .await?;
            last_block_processed = end_block;
        }

        try_info!(self.ctx, "Index has reached bitcoin chain tip");
        Ok(())
    }
}

pub async fn chainhook_sidecar_mutate_blocks(
    blocks_to_mutate: &mut Vec<BitcoinBlockDataCached>,
    block_ids_to_rollback: &Vec<BlockIdentifier>,
    cache_l2: &Arc<DashMap<(u32, [u8; 8]), TransactionBytesCursor, BuildHasherDefault<FxHasher>>>,
    brc20_cache: &mut Option<Brc20MemoryCache>,
    prometheus: &PrometheusMonitoring,
    config: &Config,
    pg_pools: &PgConnectionPools,
    ctx: &Context,
) -> Result<(), String> {
    if block_ids_to_rollback.len() > 0 {
        let blocks_db_rw = open_blocks_db_with_retry(true, &config, ctx);
        for block_id in block_ids_to_rollback.iter() {
            blocks::delete_blocks_in_block_range(
                block_id.index as u32,
                block_id.index as u32,
                &blocks_db_rw,
                &ctx,
            );
            rollback_block(block_id.index, config, pg_pools, ctx).await?;
        }
        blocks_db_rw
            .flush()
            .map_err(|e| format!("error dropping rollback blocks from rocksdb: {e}"))?;
    }

    for cached_block in blocks_to_mutate.iter_mut() {
        if cached_block.processed_by_sidecar {
            continue;
        }
        let block_bytes = match BlockBytesCursor::from_standardized_block(&cached_block.block) {
            Ok(block_bytes) => block_bytes,
            Err(e) => {
                return Err(format!(
                    "Unable to compress block #{}: #{e}",
                    cached_block.block.block_identifier.index
                ));
            }
        };
        {
            let blocks_db_rw = open_blocks_db_with_retry(true, &config, ctx);
            blocks::insert_entry_in_blocks(
                cached_block.block.block_identifier.index as u32,
                &block_bytes,
                true,
                &blocks_db_rw,
                &ctx,
            );
            blocks_db_rw
                .flush()
                .map_err(|e| format!("error inserting block to rocksdb: {e}"))?;
        }
        let mut cache_l1 = BTreeMap::new();
        let mut sequence_cursor = SequenceCursor::new();
        index_block(
            &mut cached_block.block,
            &vec![],
            &mut sequence_cursor,
            &mut cache_l1,
            &cache_l2,
            brc20_cache.as_mut(),
            prometheus,
            &config,
            pg_pools,
            &ctx,
        )
        .await?;
        cached_block.processed_by_sidecar = true;
    }
    Ok(())
}
