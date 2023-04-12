use crate::bitcoin_node::BitcoinTransaction;
use crate::peg_wallet::{BitcoinWallet as BitcoinWalletTrait, Error as PegWalletError};
use crate::stacks_node::PegOutRequestOp;
use bitcoin::hashes::hex::{FromHex, ToHex};
use bitcoin::hashes::Hash;
use bitcoin::Script;

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("type conversion error from blockstack::bitcoin to bitcoin:: {0}")]
    ConversionError(#[from] bitcoin::hashes::Error),
    #[error("type conversion error blockstack::bitcoin::hashes:hex {0}")]
    ConversionErrorHex(#[from] bitcoin::hashes::hex::Error),
}

pub struct BitcoinWallet {}

fn build_transaction(op: &PegOutRequestOp) -> Result<BitcoinTransaction, Error> {
    let bitcoin_txid = bitcoin::Txid::from_slice(op.txid.as_bytes())?;
    let utxo = bitcoin::OutPoint {
        txid: bitcoin_txid,
        vout: op.vtxindex,
    };
    let peg_out_input = bitcoin::TxIn {
        previous_output: utxo,
        script_sig: Default::default(),
        sequence: Default::default(),
        witness: Default::default(),
    };
    //let p2wpk = bitcoin::Script::new_v0_p2wpkh(&user_address.wpubkey_hash().unwrap());
    let peg_out_output_stx = op.recipient.to_bitcoin_tx_out(op.amount);
    let peg_out_script = Script::from_hex(&peg_out_output_stx.script_pubkey.to_hex())?;
    let peg_out_output = bitcoin::TxOut {
        value: peg_out_output_stx.value,
        script_pubkey: peg_out_script,
    };
    Ok(bitcoin::blockdata::transaction::Transaction {
        version: 0,
        lock_time: bitcoin::PackedLockTime(0),
        input: vec![peg_out_input],
        output: vec![peg_out_output],
    })
}

impl BitcoinWalletTrait for BitcoinWallet {
    type Error = Error;
    fn fulfill_peg_out(&self, op: &PegOutRequestOp) -> Result<BitcoinTransaction, PegWalletError> {
        let tx = build_transaction(op)?;
        Ok(tx)
    }
}

#[cfg(test)]
mod tests {
    use super::BitcoinWallet;
    use crate::peg_wallet::BitcoinWallet as BitcoinWalletTrait;
    use blockstack_lib::burnchains::Txid;
    use blockstack_lib::chainstate::stacks::address::{PoxAddress, PoxAddressType20};
    use blockstack_lib::types::chainstate::BurnchainHeaderHash;
    use blockstack_lib::util::secp256k1::MessageSignature;

    use crate::stacks_node::PegOutRequestOp;

    #[test]
    fn fulfill_peg_out() {
        let wallet = BitcoinWallet {};
        let recipient = PoxAddress::Addr20(true, PoxAddressType20::P2WPKH, [0x01; 20]);
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
        let btc_tx = wallet.fulfill_peg_out(&req_op).unwrap();
        assert_eq!(btc_tx.output[0].value, 1000)
    }
}
