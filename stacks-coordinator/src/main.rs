use clap::Parser;
use frost_signer::logging;
use stacks_coordinator::cli::{Cli, Command};
use stacks_coordinator::config::Config;
use stacks_coordinator::coordinator::{Coordinator, StacksCoordinator};
use tracing::{info, warn};

fn main() {
    let cli = Cli::parse();

    // Initialize logging
    logging::initiate_tracing_subscriber(if cli.debug {
        tracing::Level::DEBUG
    } else {
        tracing::Level::INFO
    })
    .unwrap();

    //TODO: get configs from sBTC contract
    match Config::from_path(&cli.config) {
        Ok(mut config) => {
            config.signer_config_path = cli.signer_config;
            if cli.start_block_height.is_some() {
                config.start_block_height = cli.start_block_height;
            }
            match StacksCoordinator::try_from(config) {
                Ok(mut coordinator) => {
                    // Determine what action the caller wishes to perform
                    match cli.command {
                        Command::Run => {
                            info!("Running coordinator");
                            //TODO: set up coordination with the stacks node
                            if let Err(e) = coordinator.run() {
                                warn!("An error occurred running the coordinator: {}", e);
                            }
                        }
                        Command::Dkg => {
                            info!("Running DKG Round");
                            if let Err(e) = coordinator.run_dkg_round() {
                                warn!("An error occurred during DKG round: {}", e);
                            }
                        }
                        Command::DkgSign => {
                            info!("Running DKG Round");
                            if let Err(e) = coordinator.run_dkg_round() {
                                warn!("An error occurred during DKG round: {}", e);
                            };
                            info!("Running Signing Round");
                            let (signature, schnorr_proof) =
                                match coordinator.sign_message("Hello, world!") {
                                    Ok((sig, proof)) => (sig, proof),
                                    Err(e) => {
                                        panic!("signing message failed: {e}");
                                    }
                                };
                            info!(
                                "Got good signature ({},{}) and schnorr proof ({},{})",
                                &signature.R, &signature.z, &schnorr_proof.r, &schnorr_proof.s
                            );
                        }
                    };
                }
                Err(e) => {
                    warn!("An error occurred creating coordinator: {}", e);
                }
            }
        }
        Err(e) => {
            warn!("An error occrred reading config file {}: {}", cli.config, e);
        }
    }
}
