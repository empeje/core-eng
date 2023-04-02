use clap::Parser;
use serde::Deserialize;
use std::fs;
use toml;

#[derive(Clone, Deserialize, Default, Debug)]
pub struct Config {
    pub http_relay_url: String,
    pub total_signers: usize,
    pub total_keys: usize,
    pub keys_threshold: usize,
    pub frost_state_file: String,
    pub network_private_key: String,
    pub signer_public_keys: Vec<String>,
    pub key_public_keys: Vec<String>,
    pub coordinator_public_key: String,
}

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
pub struct Cli {
    /// Turn debugging information on
    #[arg(short, long, action = clap::ArgAction::Count)]
    debug: u8,

    /// Config file path
    #[arg(short, long)]
    pub config: String,

    /// Start a signing round
    #[arg(short, long)]
    pub start: bool,

    /// ID associated with signer
    #[arg(short, long)]
    pub id: u32,
}

impl Config {
    pub fn from_path(path: impl AsRef<std::path::Path>) -> Result<Config, Error> {
        let content = fs::read_to_string(path)?;
        Ok(toml::from_str(&content)?)
    }
}

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("IO Error: {0}")]
    IO(#[from] std::io::Error),
    #[error("Toml Deserializer Error: {0}")]
    Toml(#[from] toml::de::Error),
}
