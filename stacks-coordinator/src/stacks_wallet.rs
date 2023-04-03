use crate::{
    make_contract_call::{
        Error as ContractError, MakeContractCall, SignedContractCallOptions, ANY,
    },
    peg_wallet::{Error as PegWalletError, PegWalletAddress, StacksWallet as StacksWalletTrait},
    stacks_node::{PegInOp, PegOutRequestOp},
    stacks_transaction::Error as StacksTransactionError,
};
use blockstack_lib::{
    address::AddressHashMode,
    chainstate::stacks::{address::PoxAddress, StacksTransaction},
    types::chainstate::{StacksAddress, StacksPublicKey},
    vm::{
        types::{ASCIIData, StacksAddressExtensions},
        Value,
    },
};

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
    ///An invalid peg out
    #[error("Invalid peg wallet address: {0}")]
    InvalidAddress(PoxAddress),
    ///An invalid transaction
    #[error("Invalid stacks transaction: {0}")]
    InvalidTransaction(#[from] StacksTransactionError),
    ///An invalid transaction
    #[error("Invalid Transaction: {0}")]
    InvalidPubkey(String),
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

        // Build the function arguments
        let amount = Value::UInt(op.amount.into());
        let principal = Value::from(op.recipient.clone());
        //Note that this tx_id is only used to print info in the contract call.
        let tx_id = Value::from(ASCIIData {
            data: op.txid.as_bytes().to_vec(),
        });
        let function_args: Vec<Value> = vec![amount, principal, tx_id];

        // Build the signed options to pass to the stacks.js call "makeContractCall"
        let input = SignedContractCallOptions::new(
            self.contract_address.clone(),
            self.contract_name.clone(),
            function_name,
            &function_args,
            ANY,
            self.sender_key.clone(),
        )
        .with_fee(0);

        let tx = self.make_contract_call.call(&input).map_err(Error::from)?;
        Ok(tx.to_blockstack_transaction().map_err(Error::from)?)
    }
    fn build_burn_transaction(
        &mut self,
        op: &PegOutRequestOp,
    ) -> Result<StacksTransaction, PegWalletError> {
        let function_name = "burn!";

        // Build the function arguments
        let amount = Value::UInt(op.amount.into());
        // Retrieve the address from the Message Signature
        let pub_key = StacksPublicKey::recover_to_pubkey(op.txid.as_bytes(), &op.signature)
            .map_err(|e| Error::InvalidPubkey(e.to_string()))?;
        let address = StacksAddress::from_public_keys(
            0, //Defaulting to mainnet
            &AddressHashMode::SerializeP2PKH,
            1,
            &vec![pub_key],
        )
        .ok_or(Error::InvalidPubkey(
            "Failed to generate stacks address from public key".to_string(),
        ))?;
        let principal_data = address.to_account_principal();
        let principal = Value::Principal(principal_data);
        //Note that this tx_id is only used to print info inside the contract call.
        let tx_id = Value::from(ASCIIData {
            data: op.txid.to_bytes().to_vec(),
        });
        let function_args: Vec<Value> = vec![amount, principal, tx_id];

        // Build the signed options to pass to the stacks.js call "makeContractCall"
        let input = SignedContractCallOptions::new(
            self.contract_address.clone(),
            self.contract_name.clone(),
            function_name,
            &function_args,
            ANY,
            self.sender_key.clone(),
        )
        .with_fee(0);
        let tx = self.make_contract_call.call(&input).map_err(Error::from)?;
        Ok(tx.to_blockstack_transaction().map_err(Error::from)?)
    }
    fn build_set_address_transaction(
        &mut self,
        address: PegWalletAddress,
    ) -> Result<StacksTransaction, PegWalletError> {
        let function_name = "set-bitcoin-wallet-address";

        // Build the function arguments
        let address = Value::from(ASCIIData {
            data: address.0.to_vec(),
        });
        let function_args = vec![address];

        // Build the signed options to pass to the stacks.js call "makeContractCall"
        let input = SignedContractCallOptions::new(
            self.contract_address.clone(),
            self.contract_name.clone(),
            function_name,
            &function_args,
            ANY,
            self.sender_key.clone(),
        )
        .with_fee(0);

        let tx = self.make_contract_call.call(&input).map_err(Error::from)?;
        Ok(tx.to_blockstack_transaction().map_err(Error::from)?)
    }
}
