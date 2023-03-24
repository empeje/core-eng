use bitcoin::hashes::Hash;
use bitcoin::{Script, WPubkeyHash};
use serde::Serialize;

use crate::bitcoin_node;
use crate::bitcoin_node::BitcoinTransaction;
use crate::stacks_node;
use crate::stacks_node::{PegInOp, PegOutRequestOp};
use crate::stacks_transaction::StacksTransaction;
use crate::stacks_wallet_js::Error as StacksWalletJsError;

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("Stacks Wallet JS Error: {0}")]
    StacksWalletJs(#[from] StacksWalletJsError),
    #[error("type conversion error from blockstack::bitcoin to bitcoin:: {0}")]
    ConversionError(#[from] bitcoin::hashes::Error),
    #[error("type conversion error blockstack::bitcoin::hashes:hex {0}")]
    ConversionErrorHex(#[from] bitcoin::hashes::hex::Error),
    #[error("type conversion error bitcoin::util::key::Error {0}")]
    ConversionErrorKey(#[from] bitcoin::util::key::Error),
}

pub trait StacksWallet {
    fn mint(&mut self, op: &stacks_node::PegInOp) -> Result<StacksTransaction, Error>;
    fn burn(&mut self, op: &stacks_node::PegOutRequestOp) -> Result<StacksTransaction, Error>;
    fn set_wallet_address(&mut self, address: PegWalletAddress)
        -> Result<StacksTransaction, Error>;
}

pub trait BitcoinWallet {
    fn fulfill_peg_out(
        &self,
        op: &stacks_node::PegOutRequestOp,
    ) -> Result<bitcoin_node::BitcoinTransaction, Error>;
    fn validate_peg_out_request(&self, txid: bitcoin::Txid) -> Result<(), Error>;
    fn build_peg_out_btc_tx(&self, op: &PegOutRequestOp) -> Result<BitcoinTransaction, Error>;
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
    pub(crate) bitcoin_wallet: FileBitcoinWallet,
}

impl PegWallet for WrapPegWallet {
    type StacksWallet = FileStacksWallet;
    type BitcoinWallet = FileBitcoinWallet;

    fn stacks_mut(&mut self) -> &mut Self::StacksWallet {
        todo!()
    }

    fn bitcoin_mut(&mut self) -> &mut Self::BitcoinWallet {
        &mut self.bitcoin_wallet
    }
}

pub struct FileStacksWallet {}

impl StacksWallet for FileStacksWallet {
    fn mint(&mut self, _op: &PegInOp) -> Result<StacksTransaction, Error> {
        todo!()
    }

    fn burn(&mut self, _op: &PegOutRequestOp) -> Result<StacksTransaction, Error> {
        todo!()
    }

    fn set_wallet_address(
        &mut self,
        _address: PegWalletAddress,
    ) -> Result<StacksTransaction, Error> {
        todo!()
    }
}

pub struct FileBitcoinWallet {}

impl BitcoinWallet for FileBitcoinWallet {
    fn fulfill_peg_out(&self, op: &PegOutRequestOp) -> Result<BitcoinTransaction, Error> {
        let bitcoin_peg_out_request_txid = bitcoin::Txid::from_slice(op.txid.as_bytes())?;
        self.validate_peg_out_request(bitcoin_peg_out_request_txid)?;
        self.build_peg_out_btc_tx(op)
    }

    fn validate_peg_out_request(&self, _txid: bitcoin::Txid) -> Result<(), Error> {
        Ok(()) // todo
    }

    fn build_peg_out_btc_tx(&self, op: &PegOutRequestOp) -> Result<BitcoinTransaction, Error> {
        let utxo = bitcoin::OutPoint {
            txid: bitcoin::Txid::from_slice(&[0; 32]).unwrap(),
            vout: op.vtxindex,
        };
        let peg_out_input = bitcoin::TxIn {
            previous_output: utxo,
            script_sig: Default::default(),
            sequence: Default::default(),
            witness: Default::default(),
        };
        let user_address_hash =
            bitcoin::hashes::hash160::Hash::from_slice(&op.recipient.bytes()).unwrap();
        let recipient_p2wpk = Script::new_v0_p2wpkh(&WPubkeyHash::from_hash(user_address_hash));
        let peg_out_output_recipient = bitcoin::TxOut {
            value: op.amount,
            script_pubkey: recipient_p2wpk,
        };
        let change_address_p2tr = Script::default(); // todo: Script::new_v1_p2tr();
        let peg_out_output_change = bitcoin::TxOut {
            value: op.amount,
            script_pubkey: change_address_p2tr,
        };
        Ok(bitcoin::blockdata::transaction::Transaction {
            version: 2,
            lock_time: bitcoin::PackedLockTime(0),
            input: vec![peg_out_input],
            output: vec![peg_out_output_recipient, peg_out_output_change],
        })
    }
}

#[cfg(test)]
mod tests {
    use blockstack_lib::burnchains::Txid;
    use blockstack_lib::chainstate::stacks::address::{PoxAddress, PoxAddressType20};
    use blockstack_lib::types::chainstate::BurnchainHeaderHash;
    use blockstack_lib::util::secp256k1::MessageSignature;

    use crate::peg_wallet::{BitcoinWallet, FileBitcoinWallet};
    use crate::stacks_node::PegOutRequestOp;

    #[test]
    fn bulid_peg_out_btc_op() {
        let wallet = FileBitcoinWallet {};
        let bitcoin_address =
            bitcoin::hashes::hex::FromHex::from_hex("dbc67065ff340e44956471a4b85a6b636c223a06")
                .unwrap();
        let recipient = PoxAddress::Addr20(true, PoxAddressType20::P2WPKH, bitcoin_address);
        let peg_wallet_address = PoxAddress::Addr20(true, PoxAddressType20::P2WPKH, [0x01; 20]);
        let req_op = PegOutRequestOp {
            amount: 1000,
            recipient: recipient,
            signature: MessageSignature([0x00; 65]),
            peg_wallet_address: peg_wallet_address,
            fulfillment_fee: 0,
            memo: vec![],
            txid: Txid([0x04; 32]),
            vtxindex: 0,
            block_height: 0,
            burn_header_hash: BurnchainHeaderHash([0x00; 32]),
        };
        let btc_tx = wallet.build_peg_out_btc_tx(&req_op).unwrap();
        assert_eq!(btc_tx.output[0].value, 1000)
    }
}
