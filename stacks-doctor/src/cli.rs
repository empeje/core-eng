use core::fmt::{self, Formatter};
use std::{
    fmt::{Debug, Display},
    path::PathBuf,
    str::FromStr,
};

use clap::{Parser, Subcommand};

#[derive(Clone, Debug)]
pub enum Network {
    Mainnet,
    Testnet,
}

impl Display for Network {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        write!(f, "{:?}", self)
    }
}

impl FromStr for Network {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_ascii_lowercase().as_str() {
            "mainnet" => Ok(Network::Mainnet),
            "testnet" => Ok(Network::Testnet),
            _ => Err(format!("Could not parse Network: {}", s)),
        }
    }
}

#[derive(Parser, Clone, Debug)]
pub struct BlocksArgs {
    // How many recent blocks to take into account
    #[arg(short, long, default_value_t = 1000)]
    pub blocks: u64,
}

#[derive(Subcommand, Clone, Debug)]
pub enum Commands {
    /// Analyze miner
    Analyze,
    /// Print burn fee information
    Burns(BlocksArgs),
    /// Print reorg information
    Reorgs(BlocksArgs),
    /// Print related environment variables that are set
    Env,
}

/// Tool for debugging running Stacks nodes
#[derive(Parser, Clone, Debug)]
#[command(author, version, about)]
pub struct Args {
    /// Which network to analyze
    #[arg(short, long, default_value_t = Network::Mainnet, env = "DOCTOR_NETWORK")]
    pub network: Network,

    /// URL to the node RPC API
    #[arg(short, long, env = "DOCTOR_RPC_URL")]
    pub rpc_url: Option<String>,

    /// Path to the node log file
    #[arg(short, long, env = "DOCTOR_LOG_FILE")]
    pub log_file: Option<PathBuf>,

    /// Path to the node directory with all the databases, usually contains a <mode>/ dir such as xenon/
    #[arg(short, long, env = "DOCTOR_DB_DIR")]
    pub db_dir: Option<PathBuf>,

    #[command(subcommand)]
    pub cmd: Commands,
}
