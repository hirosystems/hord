mod http_api;
mod runloops;

use crate::cli::fetch_and_standardize_block;
use crate::config::{Config, PredicatesApi, PredicatesApiConfig};
use crate::core::pipeline::processors::inscription_indexing::process_blocks;
use crate::core::pipeline::processors::start_inscription_indexing_processor;
use crate::core::pipeline::{download_and_pipeline_blocks, PostProcessorCommand};
use crate::core::protocol::sequencing::update_storage_and_augment_bitcoin_block_with_inscription_transfer_data_tx;
use crate::core::{
    new_traversals_lazy_cache, parse_inscriptions_in_standardized_block,
    revert_hord_db_with_augmented_bitcoin_block, should_sync_hord_db,
};
use crate::db::{
    find_all_inscriptions_in_block, format_satpoint_to_watch, insert_entry_in_blocks,
    insert_entry_in_locations, open_readwrite_hord_db_conn, open_readwrite_hord_dbs,
    parse_satpoint_to_watch, remove_entries_from_locations_at_block_height, InscriptionHeigthHint,
    LazyBlock,
};
use crate::scan::bitcoin::process_block_with_predicates;
use crate::service::http_api::{load_predicates_from_redis, start_predicate_api_server};
use crate::service::runloops::start_bitcoin_scan_runloop;

use chainhook_sdk::bitcoincore_rpc_json::bitcoin::hashes::hex::FromHex;
use chainhook_sdk::bitcoincore_rpc_json::bitcoin::{Address, Network, Script};
use chainhook_sdk::chainhooks::types::{
    BitcoinChainhookSpecification, ChainhookConfig, ChainhookFullSpecification,
    ChainhookSpecification,
};

use chainhook_sdk::indexer::bitcoin::build_http_client;
use chainhook_sdk::observer::{
    start_event_observer, BitcoinConfig, EventObserverConfig, HandleBlock, ObserverEvent,
};
use chainhook_sdk::types::{
    BitcoinBlockData, BitcoinNetwork, OrdinalInscriptionTransferData, OrdinalOperation,
};
use chainhook_sdk::utils::Context;
use redis::{Commands, Connection};

use std::collections::BTreeMap;
use std::sync::mpsc::{channel, Sender};
use std::sync::Arc;

pub struct Service {
    config: Config,
    ctx: Context,
}

impl Service {
    pub fn new(config: Config, ctx: Context) -> Self {
        Self { config, ctx }
    }

    pub async fn run(&mut self, predicates: Vec<ChainhookFullSpecification>) -> Result<(), String> {
        let mut event_observer_config = self.config.get_event_observer_config();
        let chainhook_config = create_and_consolidate_chainhook_config_with_predicates(
            predicates,
            &self.config,
            &self.ctx,
        );
        event_observer_config.chainhook_config = Some(chainhook_config);

        let hord_config = self.config.get_hord_config();

        // Sleep
        // std::thread::sleep(std::time::Duration::from_secs(180));

        // Force rebuild
        // {
        //     let blocks_db = open_readwrite_hord_db_conn_rocks_db(
        //         &self.config.expected_cache_path(),
        //         &self.ctx,
        //     )?;
        //     let inscriptions_db_conn_rw =
        //         open_readwrite_hord_db_conn(&self.config.expected_cache_path(), &self.ctx)?;

        //     delete_data_in_hord_db(
        //         767430,
        //         800000,
        //         &blocks_db,
        //         &inscriptions_db_conn_rw,
        //         &self.ctx,
        //     )?;
        // }

        // download_and_pipeline_blocks(&self.config, 767400, 767429, 767400, None, &self.ctx).await?;

        // Catch-up with chain tip
        {
            // Start predicate processor
            let (tx_replayer, rx_replayer) = channel();

            let blocks_post_processor =
                start_inscription_indexing_processor(&self.config, &self.ctx, Some(tx_replayer));

            let mut moved_event_observer_config = event_observer_config.clone();
            let moved_ctx = self.ctx.clone();

            let _ = hiro_system_kit::thread_named("Initial predicate processing")
                .spawn(move || {
                    if let Some(mut chainhook_config) =
                        moved_event_observer_config.chainhook_config.take()
                    {
                        let mut bitcoin_predicates_ref: Vec<&BitcoinChainhookSpecification> =
                            vec![];
                        for bitcoin_predicate in chainhook_config.bitcoin_chainhooks.iter_mut() {
                            bitcoin_predicate.enabled = false;
                            bitcoin_predicates_ref.push(bitcoin_predicate);
                        }
                        while let Ok(block) = rx_replayer.recv() {
                            let future = process_block_with_predicates(
                                block,
                                &bitcoin_predicates_ref,
                                &moved_event_observer_config,
                                &moved_ctx,
                            );
                            let res = hiro_system_kit::nestable_block_on(future);
                            if let Err(_) = res {
                                error!(moved_ctx.expect_logger(), "Initial ingestion failing");
                            }
                        }
                    }
                })
                .expect("unable to spawn thread");

            while let Some((start_block, end_block, speed)) =
                should_sync_hord_db(&self.config, &self.ctx)?
            {
                info!(
                    self.ctx.expect_logger(),
                    "Indexing inscriptions from block #{start_block} to block #{end_block}"
                );

                let hord_config = self.config.get_hord_config();

                download_and_pipeline_blocks(
                    &self.config,
                    start_block,
                    end_block,
                    hord_config.first_inscription_height,
                    Some(&blocks_post_processor),
                    Some(&blocks_post_processor),
                    speed,
                    &self.ctx,
                )
                .await?;
            }

            let _ = blocks_post_processor
                .commands_tx
                .send(PostProcessorCommand::Terminate);
        }

        // Bitcoin scan operation threadpool
        let (observer_command_tx, observer_command_rx) = channel();
        let (block_processor_in_tx, block_processor_in_rx) = channel();
        let (block_processor_out_tx, block_processor_out_rx) = channel();

        let (bitcoin_scan_op_tx, bitcoin_scan_op_rx) = crossbeam_channel::unbounded();
        let ctx = self.ctx.clone();
        let config = self.config.clone();
        let observer_command_tx_moved = observer_command_tx.clone();
        let _ = hiro_system_kit::thread_named("Bitcoin scan runloop")
            .spawn(move || {
                start_bitcoin_scan_runloop(
                    &config,
                    bitcoin_scan_op_rx,
                    observer_command_tx_moved,
                    &ctx,
                );
            })
            .expect("unable to spawn thread");

        // Enable HTTP Predicates API, if required
        if let PredicatesApi::On(ref api_config) = self.config.http_api {
            info!(
                self.ctx.expect_logger(),
                "Listening on port {} for chainhook predicate registrations", api_config.http_port
            );
            let ctx = self.ctx.clone();
            let api_config = api_config.clone();
            let moved_observer_command_tx = observer_command_tx.clone();
            // Test and initialize a database connection
            let _ = hiro_system_kit::thread_named("HTTP Predicate API").spawn(move || {
                let future = start_predicate_api_server(api_config, moved_observer_command_tx, ctx);
                let _ = hiro_system_kit::nestable_block_on(future);
            });
        }

        let (observer_event_tx, observer_event_rx) = crossbeam_channel::unbounded();
        let traversals_cache = Arc::new(new_traversals_lazy_cache(hord_config.cache_size));

        let inner_ctx = if hord_config.logs.chainhook_internals {
            self.ctx.clone()
        } else {
            Context::empty()
        };

        info!(
            self.ctx.expect_logger(),
            "Database up to date, service will start streaming blocks"
        );

        let _ = start_event_observer(
            event_observer_config.clone(),
            observer_command_tx,
            observer_command_rx,
            Some(observer_event_tx),
            Some((block_processor_in_tx, block_processor_out_rx)),
            inner_ctx,
        );

        let ctx = self.ctx.clone();
        let config = self.config.clone();
        let moved_traversals_cache = traversals_cache.clone();
        let _ = hiro_system_kit::thread_named("Block pre-processor").spawn(move || loop {
            let command = match block_processor_in_rx.recv() {
                Ok(cmd) => cmd,
                Err(e) => {
                    error!(
                        ctx.expect_logger(),
                        "Error: broken channel {}",
                        e.to_string()
                    );
                    break;
                }
            };

            let (blocks_db_rw, mut inscriptions_db_conn_rw) =
                match open_readwrite_hord_dbs(&config.expected_cache_path(), &ctx) {
                    Ok(dbs) => dbs,
                    Err(e) => {
                        ctx.try_log(|logger| {
                            error!(logger, "Unable to open readwtite connection: {e}",)
                        });
                        continue;
                    }
                };

            match command {
                HandleBlock::UndoBlocks(mut blocks) => {
                    for block in blocks.iter_mut() {
                        // Todo: first we need to "augment" the blocks with predicate data
                        info!(
                            ctx.expect_logger(),
                            "Re-org handling: reverting changes in block #{}",
                            block.block_identifier.index
                        );

                        if let Err(e) = revert_hord_db_with_augmented_bitcoin_block(
                            block,
                            &blocks_db_rw,
                            &inscriptions_db_conn_rw,
                            &ctx,
                        ) {
                            ctx.try_log(|logger| {
                                error!(
                                    logger,
                                    "Unable to rollback bitcoin block {}: {e}",
                                    block.block_identifier
                                )
                            });
                        }
                    }
                    let _ = block_processor_out_tx.send(blocks);
                }
                HandleBlock::ApplyBlocks(mut blocks) => {
                    for block in blocks.iter_mut() {
                        let compressed_block: LazyBlock =
                            match LazyBlock::from_standardized_block(&block) {
                                Ok(block) => block,
                                Err(e) => {
                                    error!(
                                        ctx.expect_logger(),
                                        "Unable to compress block #{}: #{}",
                                        block.block_identifier.index,
                                        e.to_string()
                                    );
                                    continue;
                                }
                            };
                        insert_entry_in_blocks(
                            block.block_identifier.index as u32,
                            &compressed_block,
                            &blocks_db_rw,
                            &ctx,
                        );
                        let _ = blocks_db_rw.flush();

                        parse_inscriptions_in_standardized_block(block, &ctx);
                    }

                    let mut hint = InscriptionHeigthHint::new();
                    process_blocks(
                        &mut blocks,
                        &moved_traversals_cache,
                        &mut hint,
                        &mut inscriptions_db_conn_rw,
                        &config.get_hord_config(),
                        &None,
                        &ctx,
                    );

                    let _ = block_processor_out_tx.send(blocks);
                }
            }
        });

        loop {
            let event = match observer_event_rx.recv() {
                Ok(cmd) => cmd,
                Err(e) => {
                    error!(
                        self.ctx.expect_logger(),
                        "Error: broken channel {}",
                        e.to_string()
                    );
                    break;
                }
            };
            match event {
                ObserverEvent::PredicateRegistered(spec) => {
                    // If start block specified, use it.
                    // If no start block specified, depending on the nature the hook, we'd like to retrieve:
                    // - contract-id
                    if let PredicatesApi::On(ref config) = self.config.http_api {
                        let mut predicates_db_conn = match open_readwrite_predicates_db_conn(config)
                        {
                            Ok(con) => con,
                            Err(e) => {
                                error!(
                                    self.ctx.expect_logger(),
                                    "unable to register predicate: {}",
                                    e.to_string()
                                );
                                continue;
                            }
                        };
                        update_predicate_spec(
                            &spec.key(),
                            &spec,
                            &mut predicates_db_conn,
                            &self.ctx,
                        );
                        update_predicate_status(
                            &spec.key(),
                            PredicateStatus::Disabled,
                            &mut predicates_db_conn,
                            &self.ctx,
                        );
                    }
                    match spec {
                        ChainhookSpecification::Stacks(_predicate_spec) => {}
                        ChainhookSpecification::Bitcoin(predicate_spec) => {
                            let _ = bitcoin_scan_op_tx.send(predicate_spec);
                        }
                    }
                }
                ObserverEvent::PredicateEnabled(spec) => {
                    if let PredicatesApi::On(ref config) = self.config.http_api {
                        let mut predicates_db_conn = match open_readwrite_predicates_db_conn(config)
                        {
                            Ok(con) => con,
                            Err(e) => {
                                error!(
                                    self.ctx.expect_logger(),
                                    "unable to enable predicate: {}",
                                    e.to_string()
                                );
                                continue;
                            }
                        };
                        update_predicate_spec(
                            &spec.key(),
                            &spec,
                            &mut predicates_db_conn,
                            &self.ctx,
                        );
                        update_predicate_status(
                            &spec.key(),
                            PredicateStatus::InitialScanCompleted,
                            &mut predicates_db_conn,
                            &self.ctx,
                        );
                    }
                }
                ObserverEvent::PredicateDeregistered(spec) => {
                    if let PredicatesApi::On(ref config) = self.config.http_api {
                        let mut predicates_db_conn = match open_readwrite_predicates_db_conn(config)
                        {
                            Ok(con) => con,
                            Err(e) => {
                                error!(
                                    self.ctx.expect_logger(),
                                    "unable to deregister predicate: {}",
                                    e.to_string()
                                );
                                continue;
                            }
                        };
                        let predicate_key = spec.key();
                        let res: Result<(), redis::RedisError> =
                            predicates_db_conn.del(predicate_key);
                        if let Err(e) = res {
                            error!(
                                self.ctx.expect_logger(),
                                "unable to delete predicate: {}",
                                e.to_string()
                            );
                        }
                    }
                }
                ObserverEvent::Terminate => {
                    info!(self.ctx.expect_logger(), "Terminating runloop");
                    break;
                }
                _ => {}
            }
        }
        Ok(())
    }

    pub fn replay_transfers(
        &self,
        start_block: u64,
        end_block: u64,
        block_post_processor: Option<crossbeam_channel::Sender<BitcoinBlockData>>,
    ) -> Result<(), String> {
        info!(self.ctx.expect_logger(), "Transfers only");

        let bitcoin_config = BitcoinConfig {
            username: self.config.network.bitcoind_rpc_username.clone(),
            password: self.config.network.bitcoind_rpc_password.clone(),
            rpc_url: self.config.network.bitcoind_rpc_url.clone(),
            network: self.config.network.bitcoin_network.clone(),
            bitcoin_block_signaling: self.config.network.bitcoin_block_signaling.clone(),
        };
        let (tx, rx) = crossbeam_channel::bounded(100);
        let moved_ctx = self.ctx.clone();
        hiro_system_kit::thread_named("Block fetch")
            .spawn(move || {
                let http_client = build_http_client();
                for cursor in start_block..=end_block {
                    info!(moved_ctx.expect_logger(), "Fetching block {}", cursor);
                    let future = fetch_and_standardize_block(
                        &http_client,
                        cursor,
                        &bitcoin_config,
                        &moved_ctx,
                    );

                    let block = hiro_system_kit::nestable_block_on(future).unwrap();

                    let _ = tx.send(block);
                }
            })
            .unwrap();

        let mut inscriptions_db_conn_rw =
            open_readwrite_hord_db_conn(&self.config.expected_cache_path(), &self.ctx)?;

        while let Ok(mut block) = rx.recv() {
            let network = match block.metadata.network {
                BitcoinNetwork::Mainnet => Network::Bitcoin,
                BitcoinNetwork::Regtest => Network::Regtest,
                BitcoinNetwork::Testnet => Network::Testnet,
            };

            info!(
                self.ctx.expect_logger(),
                "Cleaning transfers from block {}", block.block_identifier.index
            );
            let inscriptions = find_all_inscriptions_in_block(
                &block.block_identifier.index,
                &inscriptions_db_conn_rw,
                &self.ctx,
            );
            info!(
                self.ctx.expect_logger(),
                "{} inscriptions retrieved at block {}",
                inscriptions.len(),
                block.block_identifier.index
            );
            let mut operations = BTreeMap::new();

            let transaction = inscriptions_db_conn_rw.transaction().unwrap();

            remove_entries_from_locations_at_block_height(
                &block.block_identifier.index,
                &transaction,
                &self.ctx,
            );

            for (_, entry) in inscriptions.iter() {
                let inscription_id = entry.get_inscription_id();
                info!(
                    self.ctx.expect_logger(),
                    "Processing inscription {}", inscription_id
                );
                insert_entry_in_locations(
                    &inscription_id,
                    block.block_identifier.index,
                    &entry.transfer_data,
                    &transaction,
                    &self.ctx,
                );

                operations.insert(
                    entry.transaction_identifier_inscription.clone(),
                    OrdinalInscriptionTransferData {
                        inscription_id: entry.get_inscription_id(),
                        updated_address: None,
                        satpoint_pre_transfer: format_satpoint_to_watch(
                            &entry.transaction_identifier_inscription,
                            entry.inscription_input_index,
                            0,
                        ),
                        satpoint_post_transfer: format_satpoint_to_watch(
                            &entry.transfer_data.transaction_identifier_location,
                            entry.transfer_data.output_index,
                            entry.transfer_data.inscription_offset_intra_output,
                        ),
                        post_transfer_output_value: None,
                        tx_index: 0,
                    },
                );
            }

            info!(
                self.ctx.expect_logger(),
                "Rewriting transfers for block {}", block.block_identifier.index
            );

            for tx in block.transactions.iter_mut() {
                tx.metadata.ordinal_operations.clear();
                if let Some(mut entry) = operations.remove(&tx.transaction_identifier) {
                    let (_, output_index, _) =
                        parse_satpoint_to_watch(&entry.satpoint_post_transfer);

                    let script_pub_key_hex =
                        tx.metadata.outputs[output_index].get_script_pubkey_hex();
                    let updated_address = match Script::from_hex(&script_pub_key_hex) {
                        Ok(script) => match Address::from_script(&script, network.clone()) {
                            Ok(address) => Some(address.to_string()),
                            Err(_e) => None,
                        },
                        Err(_e) => None,
                    };

                    entry.updated_address = updated_address;
                    entry.post_transfer_output_value =
                        Some(tx.metadata.outputs[output_index].value);

                    tx.metadata
                        .ordinal_operations
                        .push(OrdinalOperation::InscriptionTransferred(entry));
                }
            }

            update_storage_and_augment_bitcoin_block_with_inscription_transfer_data_tx(
                &mut block,
                &transaction,
                &self.ctx,
            )
            .unwrap();

            info!(
                self.ctx.expect_logger(),
                "Saving supdates for block {}", block.block_identifier.index
            );
            transaction.commit().unwrap();

            info!(
                self.ctx.expect_logger(),
                "Transfers in block {} repaired", block.block_identifier.index
            );

            if let Some(ref tx) = block_post_processor {
                let _ = tx.send(block);
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PredicateStatus {
    Scanning(ScanningData),
    Streaming(StreamingData),
    InitialScanCompleted,
    Interrupted(String),
    Disabled,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScanningData {
    pub number_of_blocks_to_scan: u64,
    pub number_of_blocks_scanned: u64,
    pub number_of_blocks_sent: u64,
    pub current_block_height: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamingData {
    pub last_occurence: u64,
    pub last_evaluation: u64,
}

pub fn update_predicate_status(
    predicate_key: &str,
    status: PredicateStatus,
    predicates_db_conn: &mut Connection,
    ctx: &Context,
) {
    let serialized_status = json!(status).to_string();
    if let Err(e) =
        predicates_db_conn.hset::<_, _, _, ()>(&predicate_key, "status", &serialized_status)
    {
        error!(
            ctx.expect_logger(),
            "Error updating status: {}",
            e.to_string()
        );
    } else {
        info!(
            ctx.expect_logger(),
            "Updating predicate {predicate_key} status: {serialized_status}"
        );
    }
}

pub fn update_predicate_spec(
    predicate_key: &str,
    spec: &ChainhookSpecification,
    predicates_db_conn: &mut Connection,
    ctx: &Context,
) {
    let serialized_spec = json!(spec).to_string();
    if let Err(e) =
        predicates_db_conn.hset::<_, _, _, ()>(&predicate_key, "specification", &serialized_spec)
    {
        error!(
            ctx.expect_logger(),
            "Error updating status: {}",
            e.to_string()
        );
    } else {
        info!(
            ctx.expect_logger(),
            "Updating predicate {predicate_key} with spec: {serialized_spec}"
        );
    }
}

pub fn retrieve_predicate_status(
    predicate_key: &str,
    predicates_db_conn: &mut Connection,
) -> Option<PredicateStatus> {
    match predicates_db_conn.hget::<_, _, String>(predicate_key.to_string(), "status") {
        Ok(ref payload) => match serde_json::from_str(payload) {
            Ok(data) => Some(data),
            Err(_) => None,
        },
        Err(_) => None,
    }
}

pub fn open_readwrite_predicates_db_conn(
    config: &PredicatesApiConfig,
) -> Result<Connection, String> {
    let redis_uri = &config.database_uri;
    let client = redis::Client::open(redis_uri.clone()).unwrap();
    client
        .get_connection()
        .map_err(|e| format!("unable to connect to db: {}", e.to_string()))
}

pub fn open_readwrite_predicates_db_conn_or_panic(
    config: &PredicatesApiConfig,
    ctx: &Context,
) -> Connection {
    let redis_con = match open_readwrite_predicates_db_conn(config) {
        Ok(con) => con,
        Err(message) => {
            error!(ctx.expect_logger(), "Redis: {}", message.to_string());
            panic!();
        }
    };
    redis_con
}

// Cases to cover:
// - Empty state
// - State present, but not up to date
//      - Blocks presents, no inscriptions
//      - Blocks presents, inscription presents
// - State up to date

pub fn start_predicate_processor(
    event_observer_config: &EventObserverConfig,
    ctx: &Context,
) -> Sender<BitcoinBlockData> {
    let (tx, rx) = channel();

    let mut moved_event_observer_config = event_observer_config.clone();
    let moved_ctx = ctx.clone();

    let _ = hiro_system_kit::thread_named("Initial predicate processing")
        .spawn(move || {
            if let Some(mut chainhook_config) = moved_event_observer_config.chainhook_config.take()
            {
                let mut bitcoin_predicates_ref: Vec<&BitcoinChainhookSpecification> = vec![];
                for bitcoin_predicate in chainhook_config.bitcoin_chainhooks.iter_mut() {
                    bitcoin_predicate.enabled = false;
                    bitcoin_predicates_ref.push(bitcoin_predicate);
                }
                while let Ok(block) = rx.recv() {
                    let future = process_block_with_predicates(
                        block,
                        &bitcoin_predicates_ref,
                        &moved_event_observer_config,
                        &moved_ctx,
                    );
                    let res = hiro_system_kit::nestable_block_on(future);
                    if let Err(_) = res {
                        error!(moved_ctx.expect_logger(), "Initial ingestion failing");
                    }
                }
            }
        })
        .expect("unable to spawn thread");
    tx
}

pub fn create_and_consolidate_chainhook_config_with_predicates(
    predicates: Vec<ChainhookFullSpecification>,
    config: &Config,
    ctx: &Context,
) -> ChainhookConfig {
    let mut chainhook_config: ChainhookConfig = ChainhookConfig::new();

    // If no predicates passed at launch, retrieve predicates from Redis
    if predicates.is_empty() && config.is_http_api_enabled() {
        let registered_predicates = match load_predicates_from_redis(&config, &ctx) {
            Ok(predicates) => predicates,
            Err(e) => {
                error!(
                    ctx.expect_logger(),
                    "Failed loading predicate from storage: {}",
                    e.to_string()
                );
                vec![]
            }
        };
        for (predicate, _status) in registered_predicates.into_iter() {
            let predicate_uuid = predicate.uuid().to_string();
            match chainhook_config.register_specification(predicate) {
                Ok(_) => {
                    info!(
                        ctx.expect_logger(),
                        "Predicate {} retrieved from storage and loaded", predicate_uuid,
                    );
                }
                Err(e) => {
                    error!(
                        ctx.expect_logger(),
                        "Failed loading predicate from storage: {}",
                        e.to_string()
                    );
                }
            }
        }
    }

    // For each predicate found, register in memory.
    for predicate in predicates.into_iter() {
        match chainhook_config.register_full_specification(
            (
                &config.network.bitcoin_network,
                &config.network.stacks_network,
            ),
            predicate,
        ) {
            Ok(spec) => {
                info!(
                    ctx.expect_logger(),
                    "Predicate {} retrieved from config and loaded",
                    spec.uuid(),
                );
            }
            Err(e) => {
                error!(
                    ctx.expect_logger(),
                    "Failed loading predicate from config: {}",
                    e.to_string()
                );
            }
        }
    }

    chainhook_config
}
