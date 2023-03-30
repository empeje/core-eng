use crate::bitcoin_node;
use crate::bitcoin_wallet::{BitcoinWallet as BitcoinWalletStruct, Error as BitcoinWalletError};
use crate::stacks_node;
use crate::stacks_wallet::{Error as StacksWalletError, StacksWallet as StacksWalletStruct};
use blockstack_lib::chainstate::stacks::StacksTransaction;
use serde::Serialize;
use std::fmt::Debug;

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("Stacks Wallet Error: {0}")]
    StacksWalletError(#[from] StacksWalletError),
    #[error("Bitcoin Wallet Error: {0}")]
    BitcoinWalletError(#[from] BitcoinWalletError),
}

pub trait StacksWallet {
    fn build_mint_transaction(
        &mut self,
        op: &stacks_node::PegInOp,
    ) -> Result<StacksTransaction, Error>;
    fn build_burn_transaction(
        &mut self,
        op: &stacks_node::PegOutRequestOp,
    ) -> Result<StacksTransaction, Error>;
    fn build_set_address_transaction(
        &mut self,
        address: PegWalletAddress,
    ) -> Result<StacksTransaction, Error>;
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
    fn bitcoin_mut(&mut self) -> &mut Self::BitcoinWallet;
}

// TODO: Representation
// Should correspond to a [u8; 32] - perhaps reuse a FROST type?
#[derive(Serialize)]
pub struct PegWalletAddress(pub [u8; 32]);

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

    fn bitcoin_mut(&mut self) -> &mut Self::BitcoinWallet {
        &mut self.bitcoin_wallet
    }
}
