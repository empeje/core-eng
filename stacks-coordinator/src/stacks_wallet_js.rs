use crate::{
    make_contract_call::{
        Error as ContractError, MakeContractCall, SignedContractCallOptions, ANY,
    },
    peg_wallet::{Error as PegWalletError, PegWalletAddress, StacksWallet},
    stacks_node::{PegInOp, PegOutRequestOp},
    stacks_transaction::StacksTransaction,
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
}

pub struct StacksWalletJs {
    make_contract_call: MakeContractCall,
    contract_address: String,
    sender_key: String,
}

impl StacksWalletJs {
    pub fn new(path: &str, contract_address: String, sender_key: String) -> Result<Self, Error> {
        Ok(Self {
            make_contract_call: MakeContractCall::new(path)?,
            contract_address,
            sender_key,
        })
    }
    fn call(&mut self, function_name: String) -> Result<StacksTransaction, Error> {
        let input = SignedContractCallOptions {
            contractAddress: self.contract_address.clone(),
            contractName: "".to_string(),
            functionName: function_name,
            functionArgs: Vec::default(),
            fee: Some(0.to_string()),
            feeEstimateApiUrl: None,
            nonce: None,
            network: None,
            anchorMode: ANY,
            postConditionMode: None,
            postConditions: None,
            validateWithAbi: None,
            sponsored: None,
            senderKey: self.sender_key.clone(),
        };
        Ok(self.make_contract_call.call(&input)?)
    }
}

impl StacksWallet for StacksWalletJs {
    fn mint(&mut self, _op: &PegInOp) -> Result<StacksTransaction, PegWalletError> {
        Ok(self.call("mint".to_string())?)
    }
    fn burn(&mut self, _op: &PegOutRequestOp) -> Result<StacksTransaction, PegWalletError> {
        Ok(self.call("burn".to_string())?)
    }
    fn set_wallet_address(
        &mut self,
        _address: PegWalletAddress,
    ) -> Result<StacksTransaction, PegWalletError> {
        Ok(self.call("set_wallet_address".to_string())?)
    }
}
