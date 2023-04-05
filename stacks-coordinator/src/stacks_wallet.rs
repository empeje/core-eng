use crate::{
    make_contract_call::{
        Error as ContractError, MakeContractCall, SignedContractCallOptions, ANY,
    },
    peg_wallet::{Error as PegWalletError, StacksWallet as StacksWalletTrait},
    stacks_node::{PegInOp, PegOutRequestOp},
    stacks_transaction::Error as StacksTransactionError,
};
use blockstack_lib::{
    address::{
        AddressHashMode, C32_ADDRESS_VERSION_MAINNET_SINGLESIG,
        C32_ADDRESS_VERSION_TESTNET_SINGLESIG,
    },
    chainstate::stacks::{address::PoxAddress, StacksTransaction, TransactionVersion},
    types::{
        chainstate::{StacksAddress, StacksPrivateKey, StacksPublicKey},
        Address,
    },
    vm::{
        types::{ASCIIData, StacksAddressExtensions},
        Value,
    },
};

pub const MAINNET: &str = "mainnet";
pub const TESTNET: &str = "testnet";

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
    #[error("Invalid Sender Key: {0}")]
    InvalidSenderKey(String),
}

pub struct StacksWallet {
    make_contract_call: MakeContractCall,
    contract_address: String,
    contract_name: String,
    sender_key: StacksPrivateKey,
    version: TransactionVersion,
    address: StacksAddress,
}

impl StacksWallet {
    fn version_string(&self) -> String {
        match self.version {
            TransactionVersion::Mainnet => MAINNET.to_string(),
            TransactionVersion::Testnet => TESTNET.to_string(),
        }
    }

    pub fn new(
        path: &str,
        contract: String,
        sender_key: &str,
        version: TransactionVersion,
    ) -> Result<Self, Error> {
        let sender_key = StacksPrivateKey::from_hex(sender_key)
            .map_err(|e| Error::InvalidSenderKey(e.to_string()))?;

        let pk = StacksPublicKey::from_private(&sender_key);
        let addr_version = match version {
            TransactionVersion::Mainnet => C32_ADDRESS_VERSION_MAINNET_SINGLESIG,
            TransactionVersion::Testnet => C32_ADDRESS_VERSION_TESTNET_SINGLESIG,
        };

        let address = StacksAddress::from_public_keys(
            addr_version,
            &AddressHashMode::SerializeP2PKH,
            1,
            &vec![pk],
        )
        .ok_or(Error::InvalidSenderKey(
            "Failed to generate address from public key".to_string(),
        ))
        .map_err(Error::from)?;

        let contract_info: Vec<&str> = contract.split('.').collect();
        if contract_info.len() != 2 {
            return Err(Error::InvalidContract(contract));
        }
        Ok(Self {
            make_contract_call: MakeContractCall::new(path)?,
            contract_address: contract_info[0].to_owned(),
            contract_name: contract_info[1].to_owned(),
            sender_key,
            version,
            address,
        })
    }
}

impl StacksWalletTrait for StacksWallet {
    fn build_mint_transaction(
        &mut self,
        op: &PegInOp,
        nonce: u64,
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
            self.sender_key.to_hex(),
        )
        .with_nonce(nonce)
        .with_network(self.version_string())
        .with_fee(0);

        let tx = self.make_contract_call.call(&input).map_err(Error::from)?;
        Ok(tx.to_blockstack_transaction().map_err(Error::from)?)
    }

    fn build_burn_transaction(
        &mut self,
        op: &PegOutRequestOp,
        nonce: u64,
    ) -> Result<StacksTransaction, PegWalletError> {
        let function_name = "burn!";

        // Build the function arguments
        let amount = Value::UInt(op.amount.into());
        // Retrieve the address from the Message Signature
        let pub_key = StacksPublicKey::recover_to_pubkey(op.txid.as_bytes(), &op.signature)
            .map_err(|e| Error::InvalidPubkey(e.to_string()))?;
        let address = StacksAddress::from_public_keys(
            self.version as u8,
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
            self.sender_key.to_hex(),
        )
        .with_nonce(nonce)
        .with_network(self.version_string())
        .with_fee(0);
        let tx = self.make_contract_call.call(&input).map_err(Error::from)?;
        Ok(tx.to_blockstack_transaction().map_err(Error::from)?)
    }

    fn build_set_address_transaction(
        &mut self,
        address: StacksAddress,
        nonce: u64,
    ) -> Result<StacksTransaction, PegWalletError> {
        let function_name = "set-bitcoin-wallet-address";

        // Build the function arguments
        let address = Value::from(ASCIIData {
            data: address.to_bytes(),
        });
        let function_args = vec![address];

        // Build the signed options to pass to the stacks.js call "makeContractCall"
        let input = SignedContractCallOptions::new(
            self.contract_address.clone(),
            self.contract_name.clone(),
            function_name,
            &function_args,
            ANY,
            self.sender_key.to_hex(),
        )
        .with_nonce(nonce)
        .with_network(self.version_string())
        .with_fee(0);

        let tx = self.make_contract_call.call(&input).map_err(Error::from)?;
        Ok(tx.to_blockstack_transaction().map_err(Error::from)?)
    }

    fn get_address(&self) -> &StacksAddress {
        &self.address
    }
}
