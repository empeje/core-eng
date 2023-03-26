use bitcoin::{
    psbt::Prevouts, secp256k1::Error as Secp256k1Error, util::sighash::Error as SighashError,
    SchnorrSighashType, XOnlyPublicKey,
};

use frost_coordinator::{coordinator::Error as FrostCoordinatorError, create_coordinator};
use frost_signer::net::{Error as HttpNetError, HttpNetListen};
use std::sync::mpsc;
use std::sync::mpsc::Sender;
use std::{thread, time};
use tracing::info;
use wtfrost::{bip340::SchnorrProof, common::Signature};

use crate::config::{Config, Error as ConfigError};
use crate::peg_wallet::{BitcoinWallet, Error as PegWalletError, FileBitcoinWallet, PegWallet};
use crate::peg_wallet::{StacksWallet, WrapPegWallet};
use crate::stacks_node;
// Traits in scope
use crate::bitcoin_node::{BitcoinNode, BitcoinTransaction, LocalhostBitcoinNode};
use crate::peg_queue::{
    Error as PegQueueError, PegQueue, SbtcOp, SqlitePegQueue, SqlitePegQueueError,
};
use crate::stacks_node::client::NodeClient;
use crate::stacks_node::StacksNode;

type FrostCoordinator = frost_coordinator::coordinator::Coordinator<HttpNetListen>;

pub type PublicKey = XOnlyPublicKey;

/// Helper that uses this module's error type
pub type Result<T> = std::result::Result<T, Error>;

/// Kinds of common errors used by stacks coordinator
#[derive(thiserror::Error, Debug)]
pub enum Error {
    /// Error occurred with the HTTP Relay
    #[error("Http Network Error: {0}")]
    HttpNetError(#[from] HttpNetError),
    /// Error occurred reading Config
    #[error("Config Error: {0}")]
    ConfigError(#[from] ConfigError),
    /// Error occurred in the Peg Queue
    #[error("Peg Queue Error: {0}")]
    PegQueueError(#[from] PegQueueError),
    /// Error occurred in the Peg Wallet
    #[error("Peg Wallet Error: {0}")]
    PegWalletError(#[from] PegWalletError),
    /// Error occurred in the Frost Coordinator
    #[error("Frost Coordinator Error: {0}")]
    FrostCoordinatorError(#[from] FrostCoordinatorError),
    /// Error occurred in the Sqlite Peg Queue
    #[error("Sqlite Peg Queue Error: {0}")]
    SqlitePegQueueError(#[from] SqlitePegQueueError),
    /// "Bitcoin Secp256k1 Error"
    #[error("Bitcoin Secp256k1 Error")]
    BitcoinSecp256k1(#[from] Secp256k1Error),
    /// "Bitcoin Sighash Error"
    #[error("Bitcoin Sighash Error")]
    BitcoinSighash(#[from] SighashError),
    #[error("Command sender disconnected unexpectedly: {0}")]
    UnexpectedSenderDisconnect(#[from] std::sync::mpsc::RecvError),
}

pub trait Coordinator: Sized {
    type PegQueue: PegQueue;
    type FeeWallet: PegWallet;
    type StacksNode: StacksNode;
    type BitcoinNode: BitcoinNode;

    // Required methods
    fn peg_queue(&self) -> &Self::PegQueue;
    fn fee_wallet(&mut self) -> &mut Self::FeeWallet;
    fn frost_coordinator(&self) -> &FrostCoordinator;
    fn frost_coordinator_mut(&mut self) -> &mut FrostCoordinator;
    fn stacks_node(&self) -> &Self::StacksNode;
    fn bitcoin_node(&self) -> &Self::BitcoinNode;

    // Provided methods
    fn run(mut self) -> Result<()> {
        let (sender, receiver) = mpsc::channel::<Command>();
        Self::poll_ping_thread(sender);

        loop {
            match receiver.recv()? {
                Command::Stop => break,
                Command::Timeout => {
                    self.peg_queue().poll(self.stacks_node())?;
                    self.process_queue()?;
                }
            }
        }
        Ok(())
    }

    fn poll_ping_thread(sender: Sender<Command>) {
        thread::spawn(move || loop {
            sender
                .send(Command::Timeout)
                .expect("thread send error {0}");
            thread::sleep(time::Duration::from_millis(500));
        });
    }

    fn process_queue(&mut self) -> Result<()> {
        match self.peg_queue().sbtc_op()? {
            Some(SbtcOp::PegIn(op)) => self.peg_in(op),
            Some(SbtcOp::PegOutRequest(op)) => self.peg_out(op),
            None => Ok(()),
        }
    }
}

// Private helper functions
trait CoordinatorHelpers: Coordinator {
    fn peg_in(&mut self, op: stacks_node::PegInOp) -> Result<()> {
        let _tx = self.fee_wallet().stacks_mut().mint(&op)?;
        //self.stacks_node().broadcast_transaction(&tx);
        Ok(())
    }

    fn peg_out(&mut self, op: stacks_node::PegOutRequestOp) -> Result<()> {
        let _stacks = self.fee_wallet().stacks_mut();
        let _burn_tx = self.fee_wallet().stacks_mut().burn(&op)?;
        //self.stacks_node().broadcast_transaction(&burn_tx);

        let fulfill_tx = self.btc_fulfill_peg_out(&op)?;
        self.bitcoin_node().broadcast_transaction(&fulfill_tx);
        Ok(())
    }

    fn btc_fulfill_peg_out(
        &mut self,
        op: &stacks_node::PegOutRequestOp,
    ) -> Result<BitcoinTransaction> {
        let mut fulfill_tx = self.fee_wallet().bitcoin_mut().fulfill_peg_out(op)?;
        let pubkey = self.frost_coordinator().get_aggregate_public_key()?;
        let _xonly_pubkey =
            PublicKey::from_slice(&pubkey.x().to_bytes()).map_err(Error::BitcoinSecp256k1)?;
        let mut comp = bitcoin::util::sighash::SighashCache::new(&fulfill_tx);
        let taproot_sighash = comp.taproot_signature_hash(
            1,
            &Prevouts::All(&[&fulfill_tx.output[0]]),
            None,
            None,
            SchnorrSighashType::All,
        )?;

        let (_frost_sig, schnorr_proof) = self
            .frost_coordinator_mut()
            .sign_message(&taproot_sighash)?;

        info!(
            "Fulfill Tx {:?} SchnorrProof ({},{})",
            &fulfill_tx, schnorr_proof.r, schnorr_proof.s
        );

        let finalized = [
            schnorr_proof.to_bytes().as_ref(),
            &[SchnorrSighashType::All as u8],
        ]
        .concat();
        let finalized_b58 = bitcoin::util::base58::encode_slice(&finalized);
        info!("CALC SIG ({}) {}", finalized.len(), finalized_b58);
        fulfill_tx.input[0].witness.push(finalized);
        Ok(fulfill_tx)
    }
}

impl<T: Coordinator> CoordinatorHelpers for T {}

pub enum Command {
    Stop,
    Timeout,
}

pub struct StacksCoordinator {
    frost_coordinator: FrostCoordinator,
    local_peg_queue: SqlitePegQueue,
    local_stacks_node: NodeClient,
    pub local_fee_wallet: WrapPegWallet,
}

impl StacksCoordinator {
    pub fn run_dkg_round(&mut self) -> Result<PublicKey> {
        let p = self.frost_coordinator.run_distributed_key_generation()?;
        PublicKey::from_slice(&p.x().to_bytes()).map_err(Error::BitcoinSecp256k1)
    }

    pub fn sign_message(&mut self, message: &str) -> Result<(Signature, SchnorrProof)> {
        Ok(self.frost_coordinator.sign_message(message.as_bytes())?)
    }
}

impl TryFrom<Config> for StacksCoordinator {
    type Error = Error;
    fn try_from(config: Config) -> Result<Self> {
        Ok(Self {
            local_peg_queue: SqlitePegQueue::try_from(&config)?,
            local_stacks_node: NodeClient::new(&config.stacks_node_rpc_url),
            frost_coordinator: create_coordinator(config.signer_config_path)?,
            local_fee_wallet: WrapPegWallet {
                bitcoin_wallet: FileBitcoinWallet {},
            },
        })
    }
}

impl Coordinator for StacksCoordinator {
    type PegQueue = SqlitePegQueue;
    type FeeWallet = WrapPegWallet;
    type StacksNode = NodeClient;
    type BitcoinNode = LocalhostBitcoinNode;

    fn peg_queue(&self) -> &Self::PegQueue {
        &self.local_peg_queue
    }

    fn fee_wallet(&mut self) -> &mut Self::FeeWallet {
        &mut self.local_fee_wallet
    }

    fn frost_coordinator(&self) -> &FrostCoordinator {
        &self.frost_coordinator
    }

    fn frost_coordinator_mut(&mut self) -> &mut FrostCoordinator {
        &mut self.frost_coordinator
    }

    fn stacks_node(&self) -> &Self::StacksNode {
        &self.local_stacks_node
    }

    fn bitcoin_node(&self) -> &Self::BitcoinNode {
        todo!()
    }
}

#[cfg(test)]
mod tests {
    use crate::config::Config;
    use crate::coordinator::{CoordinatorHelpers, StacksCoordinator};
    use crate::stacks_node::PegOutRequestOp;
    use bitcoin::consensus::Encodable;
    use blockstack_lib::burnchains::Txid;
    use blockstack_lib::chainstate::stacks::address::{PoxAddress, PoxAddressType20};
    use blockstack_lib::types::chainstate::BurnchainHeaderHash;

    #[ignore]
    #[test]
    fn btc_fulfill_peg_out() {
        let config = Config {
            sbtc_contract: "".to_string(),
            stacks_private_key: "".to_string(),
            bitcoin_private_key: "".to_string(),
            stacks_node_rpc_url: "".to_string(),
            bitcoin_node_rpc_url: "".to_string(),
            frost_dkg_round_id: 0,
            signer_config_path: "conf/signer.toml".to_string(),
            start_block_height: None,
            rusqlite_path: None,
        };
        // todo: make StacksCoordinator with mock FrostCoordinator to locally generate PublicKey and Signature for unit test
        let mut sc = StacksCoordinator::try_from(config).unwrap();
        let recipient = PoxAddress::Addr20(false, PoxAddressType20::P2WPKH, [0; 20]);
        let peg_wallet_address = PoxAddress::Addr20(false, PoxAddressType20::P2WPKH, [0; 20]);
        let op = PegOutRequestOp {
            amount: 0,
            recipient: recipient,
            signature: blockstack_lib::util::secp256k1::MessageSignature([0; 65]),
            peg_wallet_address: peg_wallet_address,
            fulfillment_fee: 0,
            memo: vec![],
            txid: Txid([0; 32]),
            vtxindex: 0,
            block_height: 0,
            burn_header_hash: BurnchainHeaderHash([0; 32]),
        };
        let btc_tx_result = sc.btc_fulfill_peg_out(&op);
        assert!(btc_tx_result.is_ok());
        let btc_tx = btc_tx_result.unwrap();
        let mut btc_tx_encoded: Vec<u8> = vec![];
        btc_tx.consensus_encode(&mut btc_tx_encoded).unwrap();
        let verify_result = bitcoin::bitcoinconsensus::verify(&[], 100, &btc_tx_encoded, 0);
        assert!(verify_result.is_ok())
    }
}
