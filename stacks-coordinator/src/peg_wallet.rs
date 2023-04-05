use crate::bitcoin_node;
use crate::bitcoin_wallet::{BitcoinWallet as BitcoinWalletStruct, Error as BitcoinWalletError};
use crate::stacks_node;
use crate::stacks_wallet::{Error as StacksWalletError, StacksWallet as StacksWalletStruct};
use blockstack_lib::chainstate::stacks::StacksTransaction;
use blockstack_lib::types::chainstate::StacksAddress;
use std::fmt::Debug;

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("Stacks Wallet Error: {0}")]
    StacksWalletError(#[from] StacksWalletError),
    #[error("Bitcoin Wallet Error: {0}")]
    BitcoinWalletError(#[from] BitcoinWalletError),
}

pub trait StacksWallet {
    /// Builds a verified signed transaction for a given peg-in operation
    fn build_mint_transaction(
        &mut self,
        op: &stacks_node::PegInOp,
        nonce: u64,
    ) -> Result<StacksTransaction, Error>;
    /// Builds a verified signed transaction for a given peg-out request operation
    fn build_burn_transaction(
        &mut self,
        op: &stacks_node::PegOutRequestOp,
        nonce: u64,
    ) -> Result<StacksTransaction, Error>;
    /// Builds a verified signed transaction for setting the sBTC wallet address
    fn build_set_address_transaction(
        &mut self,
        address: StacksAddress,
        nonce: u64,
    ) -> Result<StacksTransaction, Error>;
    /// Gets the sBTC address for the wallet
    fn get_address(&self) -> &StacksAddress;
}

pub trait BitcoinWallet {
    type Error: Debug;
    fn fulfill_peg_out(
        &self,
        op: &stacks_node::PegOutRequestOp,
    ) -> Result<bitcoin_node::BitcoinTransaction, Error>;
}

pub trait PegWallet {
    type StacksWallet: StacksWallet;
    type BitcoinWallet: BitcoinWallet;
    fn stacks_mut(&mut self) -> &mut Self::StacksWallet;
    fn stacks(&self) -> &Self::StacksWallet;
    fn bitcoin_mut(&mut self) -> &mut Self::BitcoinWallet;
}

pub struct WrapPegWallet {
    pub(crate) bitcoin_wallet: BitcoinWalletStruct,
    pub(crate) stacks_wallet: StacksWalletStruct,
}

impl PegWallet for WrapPegWallet {
    type StacksWallet = StacksWalletStruct;
    type BitcoinWallet = BitcoinWalletStruct;
    fn stacks_mut(&mut self) -> &mut Self::StacksWallet {
        &mut self.stacks_wallet
    }

    fn stacks(&self) -> &Self::StacksWallet {
        &self.stacks_wallet
    }

    fn bitcoin_mut(&mut self) -> &mut Self::BitcoinWallet {
        &mut self.bitcoin_wallet
    }
}
