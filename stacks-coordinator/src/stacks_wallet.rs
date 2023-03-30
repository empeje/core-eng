use crate::{
    make_contract_call::{
        Error as ContractError, MakeContractCall, SignedContractCallOptions, ANY,
    },
    peg_wallet::{Error as PegWalletError, PegWalletAddress, StacksWallet as StacksWalletTrait},
    stacks_node::{PegInOp, PegOutRequestOp},
    stacks_transaction::StacksTransaction,
};

use blockstack_lib::vm::types::{ASCIIData, BuffData, CharType, SequenceData};
use blockstack_lib::vm::Value;

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("type conversion error from blockstack::bitcoin to bitcoin:: {0}")]
    ConversionError(#[from] bitcoin::hashes::Error),
    #[error("type conversion error blockstack::bitcoin::hashes:hex {0}")]
    ConversionErrorHex(#[from] bitcoin::hashes::hex::Error),
    ///Error occurred calling the sBTC contract
    #[error("Contract Error: {0}")]
    ContractError(#[from] ContractError),
    ///An invalid contract was specified in the config file
    #[error("Invalid contract name and address: {0}")]
    InvalidContract(String),
}

pub struct StacksWallet {
    make_contract_call: MakeContractCall,
    contract_address: String,
    contract_name: String,
    sender_key: String,
}

impl StacksWallet {
    pub fn new(path: &str, contract: String, sender_key: String) -> Result<Self, Error> {
        let contract_info: Vec<&str> = contract.split('.').collect();
        if contract_info.len() != 2 {
            return Err(Error::InvalidContract(contract));
        }
        Ok(Self {
            make_contract_call: MakeContractCall::new(path)?,
            contract_address: contract_info[0].to_owned(),
            contract_name: contract_info[1].to_owned(),
            sender_key,
        })
    }
}

impl StacksWalletTrait for StacksWallet {
    fn build_mint_transaction(
        &mut self,
        op: &PegInOp,
    ) -> Result<StacksTransaction, PegWalletError> {
        let function_name = "mint!";
        let amount = Value::UInt(op.amount.into());
        let principal = Value::Principal(op.recipient.clone());
        let tx_id = Value::Sequence(SequenceData::Buffer(BuffData {
            data: op.txid.to_bytes().to_vec(),
        }));
        let function_args: Vec<Value> = vec![amount, principal, tx_id];
        let input = SignedContractCallOptions::new(
            self.contract_address.clone(),
            self.contract_name.clone(),
            function_name,
            &function_args,
            ANY,
            self.sender_key.clone(),
        );
        Ok(self.make_contract_call.call(&input).map_err(Error::from)?)
    }
    fn build_burn_transaction(
        &mut self,
        _op: &PegOutRequestOp,
    ) -> Result<StacksTransaction, PegWalletError> {
        // let function_name = "burn!";
        // let amount = Value::UInt(op.amount.into());
        // let principal = todo!();
        // let tx_id = Value::Sequence(SequenceData::Buffer(BuffData {
        //     data: op.txid.to_bytes().to_vec(),
        // }));
        // let function_args: Vec<Value> = vec![amount, principal, tx_id];
        // let input = SignedContractCallOptions::new(
        //     self.contract_address.clone(),
        //     self.contract_name.clone(),
        //     function_name,
        //     &function_args,
        //     ANY,
        //     self.sender_key.clone(),
        // );
        // Ok(self.make_contract_call.call(&input).map_err(Error::from)?)
        todo!()
    }
    fn build_set_address_transaction(
        &mut self,
        address: PegWalletAddress,
    ) -> Result<StacksTransaction, PegWalletError> {
        let function_name = "set-bitcoin-wallet-address";
        let address = Value::Sequence(SequenceData::String(CharType::ASCII(ASCIIData {
            data: address.0.to_vec(),
        })));
        let function_args = vec![address];
        let input = SignedContractCallOptions::new(
            self.contract_address.clone(),
            self.contract_name.clone(),
            function_name,
            &function_args,
            ANY,
            self.sender_key.clone(),
        );
        Ok(self.make_contract_call.call(&input).map_err(Error::from)?)
    }
}
