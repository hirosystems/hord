use std::{thread::sleep, time::Duration};

use clap::{Parser, Subcommand};

use chainhook_sdk::utils::{BlockHeights, Context};

use crate::{
    config::{generator::generate_config, Config},
    db::{cache::index_cache::IndexCache, pg_connect},
    scan::bitcoin::{drop_blocks, scan_blocks},
    service::start_service,
    try_info,
};

#[derive(Parser, Debug)]
#[clap(author, version, about, long_about = None)]
struct Opts {
    #[clap(subcommand)]
    command: Command,
}

#[derive(Subcommand, PartialEq, Clone, Debug)]
enum Command {
    /// Generate configuration file
    #[clap(subcommand)]
    Config(ConfigCommand),
    /// Streaming blocks and indexing runes
    #[clap(subcommand)]
    Service(ServiceCommand),
    /// Scanning blocks and indexing runes
    #[clap(subcommand)]
    Scan(ScanCommand),
    /// Perform maintenance operations on local databases
    #[clap(subcommand)]
    Db(DbCommand),
}

#[derive(Subcommand, PartialEq, Clone, Debug)]
#[clap(bin_name = "config")]
enum ConfigCommand {
    /// Generate new config
    #[clap(name = "new", bin_name = "new", aliases = &["generate"])]
    New(NewConfig),
}

#[derive(Parser, PartialEq, Clone, Debug)]
struct NewConfig {
    /// Target Devnet network
    #[clap(
        long = "simnet",
        conflicts_with = "testnet",
        conflicts_with = "mainnet"
    )]
    pub devnet: bool,
    /// Target Testnet network
    #[clap(
        long = "testnet",
        conflicts_with = "simnet",
        conflicts_with = "mainnet"
    )]
    pub testnet: bool,
    /// Target Mainnet network
    #[clap(
        long = "mainnet",
        conflicts_with = "testnet",
        conflicts_with = "simnet"
    )]
    pub mainnet: bool,
}

#[derive(Subcommand, PartialEq, Clone, Debug)]
#[clap(bin_name = "stream")]
enum ServiceCommand {
    /// Run a service
    #[clap(name = "start", bin_name = "start")]
    Start(StartStreamCommand),
}

#[derive(Subcommand, PartialEq, Clone, Debug)]
#[clap(bin_name = "scan")]
enum ScanCommand {
    /// Run a scan
    #[clap(name = "start", bin_name = "start")]
    Start(StartScanCommand),
}

#[derive(Parser, PartialEq, Clone, Debug)]
struct StartStreamCommand {
    /// Load config file path
    #[clap(long = "config-path")]
    pub config_path: String,
}

#[derive(Parser, PartialEq, Clone, Debug)]
struct StartScanCommand {
    /// Load config file path
    #[clap(long = "config-path")]
    pub config_path: String,
    /// Interval of blocks (--interval 767430:800000)
    #[clap(long = "interval", conflicts_with = "blocks")]
    pub blocks_interval: Option<String>,
    /// List of blocks (--blocks 767430,767431,767433,800000)
    #[clap(long = "blocks", conflicts_with = "interval")]
    pub blocks: Option<String>,
}

#[derive(Parser, PartialEq, Clone, Debug)]
struct PingCommand {
    /// Load config file path
    #[clap(long = "config-path")]
    pub config_path: String,
}

#[derive(Subcommand, PartialEq, Clone, Debug)]
enum DbCommand {
    /// Rebuild inscriptions entries for a given block
    #[clap(name = "drop", bin_name = "drop")]
    Drop(DropDbCommand),
}

#[derive(Parser, PartialEq, Clone, Debug)]
struct DropDbCommand {
    /// Starting block
    pub start_block: u64,
    /// Ending block
    pub end_block: u64,
    /// Load config file path
    #[clap(long = "config-path")]
    pub config_path: String,
}

pub fn main() {
    let logger = hiro_system_kit::log::setup_logger();
    let _guard = hiro_system_kit::log::setup_global_logger(logger.clone());
    let ctx = Context {
        logger: Some(logger),
        tracer: false,
    };

    let opts: Opts = match Opts::try_parse() {
        Ok(opts) => opts,
        Err(e) => {
            println!("{}", e);
            std::process::exit(1);
        }
    };

    match hiro_system_kit::nestable_block_on(handle_command(opts, ctx)) {
        Err(e) => {
            println!("{e}");
            std::process::exit(1);
        }
        Ok(_) => {}
    }
}

async fn handle_command(opts: Opts, ctx: Context) -> Result<(), String> {
    match opts.command {
        Command::Config(ConfigCommand::New(_options)) => {
            use std::fs::File;
            use std::io::Write;
            use std::path::PathBuf;
            let config_content = generate_config();
            let mut file_path = PathBuf::new();
            file_path.push("Runehook.toml");
            let mut file = File::create(&file_path)
                .map_err(|e| format!("unable to open file {}\n{}", file_path.display(), e))?;
            file.write_all(config_content.as_bytes())
                .map_err(|e| format!("unable to write file {}\n{}", file_path.display(), e))?;
            println!("Created file Runehook.toml");
        }
        Command::Service(ServiceCommand::Start(cmd)) => {
            let config = Config::from_file_path(&cmd.config_path)?;
            let maintenance_enabled = std::env::var("MAINTENANCE_MODE").unwrap_or("0".into());
            if maintenance_enabled.eq("1") {
                try_info!(ctx, "Entering maintenance mode. Unset MAINTENANCE_MODE and reboot to resume operations.");
                sleep(Duration::from_secs(u64::MAX))
            }
            start_service(&config, &ctx).await?;
        }
        Command::Scan(ScanCommand::Start(cmd)) => {
            let config = Config::from_file_path(&cmd.config_path)?;
            let blocks = cmd.get_blocks();
            let mut pg_client = pg_connect(&config, true, &ctx).await;
            let mut index_cache = IndexCache::new(&config, &mut pg_client, &ctx).await;
            scan_blocks(blocks, &config, &mut pg_client, &mut index_cache, &ctx).await?;
        }
        Command::Db(DbCommand::Drop(cmd)) => {
            let config = Config::from_file_path(&cmd.config_path)?;
            println!(
                "{} blocks will be deleted. Confirm? [Y/n]",
                cmd.end_block - cmd.start_block + 1
            );
            let mut buffer = String::new();
            std::io::stdin().read_line(&mut buffer).unwrap();
            if buffer.starts_with('n') {
                return Err("Deletion aborted".to_string());
            }

            let mut pg_client = pg_connect(&config, false, &ctx).await;
            drop_blocks(cmd.start_block, cmd.end_block, &mut pg_client, &ctx).await;
        }
    }
    Ok(())
}

impl StartScanCommand {
    pub fn get_blocks(&self) -> Vec<u64> {
        let blocks = match (&self.blocks_interval, &self.blocks) {
            (Some(interval), None) => {
                let blocks = interval.split(':').collect::<Vec<_>>();
                let start_block: u64 = blocks
                    .first()
                    .expect("unable to get start_block")
                    .parse::<u64>()
                    .expect("unable to parse start_block");
                let end_block: u64 = blocks
                    .get(1)
                    .expect("unable to get end_block")
                    .parse::<u64>()
                    .expect("unable to parse end_block");
                BlockHeights::BlockRange(start_block, end_block).get_sorted_entries()
            }
            (None, Some(blocks)) => {
                let blocks = blocks
                    .split(',')
                    .map(|b| b.parse::<u64>().expect("unable to parse block"))
                    .collect::<Vec<_>>();
                BlockHeights::Blocks(blocks).get_sorted_entries()
            }
            _ => panic!("'--interval' or '--blocks' argument required. '--help' for more details."),
        };
        blocks.unwrap().into()
    }
}
