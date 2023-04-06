use crate::{
    peg_wallet::{Error as PegWalletError, StacksWallet as StacksWalletTrait},
    stacks_node::{PegInOp, PegOutRequestOp},
};
use blockstack_lib::{
    address::{
        AddressHashMode, C32_ADDRESS_VERSION_MAINNET_SINGLESIG,
        C32_ADDRESS_VERSION_TESTNET_SINGLESIG,
    },
    chainstate::stacks::{
        address::PoxAddress, StacksTransaction, StacksTransactionSigner, TransactionAnchorMode,
        TransactionAuth, TransactionContractCall, TransactionPayload, TransactionSpendingCondition,
        TransactionVersion,
    },
    codec::Error as CodecError,
    core::{CHAIN_ID_MAINNET, CHAIN_ID_TESTNET},
    net::Error as NetError,
    types::{
        chainstate::{StacksAddress, StacksPrivateKey, StacksPublicKey},
        Address,
    },
    util::HexError,
    vm::{
        errors::{Error as ClarityError, RuntimeErrorType},
        types::{ASCIIData, StacksAddressExtensions},
        ClarityName, ContractName, Value,
    },
};

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("type conversion error from blockstack::bitcoin to bitcoin:: {0}")]
    ConversionError(#[from] bitcoin::hashes::Error),
    ///An invalid contract was specified in the config file
    #[error("Invalid contract name and address: {0}")]
    InvalidContract(String),
    ///An invalid peg out
    #[error("Invalid peg wallet address: {0}")]
    InvalidAddress(PoxAddress),
    ///An invalid public key
    #[error("Invalid public key: {0}")]
    InvalidPublicKey(String),
    #[error("Invalid private key: {0}")]
    InvalidPrivateKey(String),
    #[error("Failed to sign transaction.")]
    SigningError,
    #[error("Hex error: {0}")]
    ConversionErrorHex(#[from] HexError),
    #[error("Stacks network error: {0}")]
    NetworkError(#[from] NetError),
    #[error("Clarity runtime error: {0}")]
    ClarityRuntimeError(#[from] RuntimeErrorType),
    #[error("Clarity error: {0}")]
    ClarityGeneralError(#[from] ClarityError),
    #[error("Stacks Code error: {0}")]
    StacksCodeError(#[from] CodecError),
    #[error("Invalid peg-out request op: {0}")]
    InvalidPegOutRequestOp(String),
}

pub struct StacksWallet {
    contract_address: StacksAddress,
    contract_name: String,
    sender_key: StacksPrivateKey,
    version: TransactionVersion,
    address: StacksAddress,
}

impl StacksWallet {
    fn build_transaction_payload(
        &self,
        function_name: impl Into<String>,
        function_args: Vec<Value>,
    ) -> Result<TransactionPayload, Error> {
        let contract_name = ContractName::try_from(self.contract_name.clone())?;
        let function_name = ClarityName::try_from(function_name.into())?;
        let payload = TransactionContractCall {
            address: self.contract_address,
            contract_name,
            function_name,
            function_args,
        };
        Ok(payload.into())
    }

    fn build_transaction_unsigned(
        &self,
        function_name: impl Into<String>,
        function_args: Vec<Value>,
        nonce: u64,
    ) -> Result<StacksTransaction, Error> {
        // First build the payload from the provided function and its arguments
        let payload = self.build_transaction_payload(function_name, function_args)?;

        // Next build the authorization from the provided sender key
        let public_key = StacksPublicKey::from_private(&self.sender_key);
        let mut spending_condition = TransactionSpendingCondition::new_singlesig_p2pkh(public_key)
            .ok_or_else(|| {
                Error::InvalidPublicKey(
                    "Failed to create single sig transaction spending condition.".to_string(),
                )
            })?;
        spending_condition.set_nonce(nonce);
        spending_condition.set_tx_fee(0);
        let auth = TransactionAuth::Standard(spending_condition);

        // Viola! We have an unsigned transaction
        let mut tx = StacksTransaction::new(self.version, auth, payload);
        let chain_id = if self.version == TransactionVersion::Testnet {
            CHAIN_ID_TESTNET
        } else {
            CHAIN_ID_MAINNET
        };
        tx.chain_id = chain_id;
        tx.anchor_mode = TransactionAnchorMode::Any;

        Ok(tx)
    }

    fn build_transaction_signed(
        &self,
        function_name: impl Into<String>,
        function_args: Vec<Value>,
        nonce: u64,
    ) -> Result<StacksTransaction, Error> {
        // First build an unsigned transaction
        let unsigned_tx = self.build_transaction_unsigned(function_name, function_args, nonce)?;

        // Do the signing
        let mut tx_signer = StacksTransactionSigner::new(&unsigned_tx);
        tx_signer.sign_origin(&self.sender_key)?;

        // Retrieve the signed transaction from the signer
        let signed_tx = tx_signer.get_tx().ok_or(Error::SigningError)?;
        Ok(signed_tx)
    }

    pub fn new(
        contract: String,
        sender_key: &str,
        version: TransactionVersion,
    ) -> Result<Self, Error> {
        let sender_key = StacksPrivateKey::from_hex(sender_key)
            .map_err(|e| Error::InvalidPrivateKey(e.to_string()))?;

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
        .ok_or(Error::InvalidPrivateKey(
            "Failed to generate address from public key".to_string(),
        ))
        .map_err(Error::from)?;

        let contract_info: Vec<&str> = contract.split('.').collect();
        if contract_info.len() != 2 {
            return Err(Error::InvalidContract(contract));
        }

        let contract_address = StacksAddress::from_string(contract_info[0]).ok_or(
            Error::InvalidContract("Failed to parse contract address".to_string()),
        )?;
        Ok(Self {
            contract_address,
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
        let tx = self.build_transaction_signed(function_name, function_args, nonce)?;
        Ok(tx)
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
            .map_err(|e| {
                Error::InvalidPegOutRequestOp(format!(
                    "Failed to recover public key from txid and signature. {e}"
                ))
            })?;
        let address = StacksAddress::from_public_keys(
            self.version as u8,
            &AddressHashMode::SerializeP2PKH,
            1,
            &vec![pub_key],
        )
        .ok_or_else(|| Error::InvalidPublicKey("Failed to generate stacks address".to_string()))?;
        let principal_data = address.to_account_principal();
        let principal = Value::Principal(principal_data);
        //Note that this tx_id is only used to print info inside the contract call.
        let tx_id = Value::from(ASCIIData {
            data: op.txid.to_bytes().to_vec(),
        });
        let function_args: Vec<Value> = vec![amount, principal, tx_id];

        let tx = self.build_transaction_signed(function_name, function_args, nonce)?;
        Ok(tx)
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
        let tx = self.build_transaction_signed(function_name, function_args, nonce)?;
        Ok(tx)
    }

    fn get_address(&self) -> &StacksAddress {
        &self.address
    }
}

#[cfg(test)]
mod tests {

    use crate::{peg_wallet::StacksWallet as StacksWalletTrait, stacks_wallet::StacksWallet};
    use blockstack_lib::{
        burnchains::Txid,
        chainstate::{
            burn::operations::{PegInOp, PegOutRequestOp},
            stacks::{address::PoxAddress, TransactionVersion},
        },
        types::chainstate::{BurnchainHeaderHash, StacksAddress},
        util::hash::hex_bytes,
        util::{hash::Hash160, secp256k1::MessageSignature},
        vm::types::{PrincipalData, StandardPrincipalData},
    };

    fn pox_address() -> PoxAddress {
        PoxAddress::Standard(StacksAddress::new(0, Hash160::from_data(&[0; 20])), None)
    }

    fn stacks_wallet() -> StacksWallet {
        StacksWallet::new(
            "SP3FBR2AGK5H9QBDH3EEN6DF8EK8JY7RX8QJ5SVTE.sbtc-alpha".to_string(),
            &"b244296d5907de9864c0b0d51f98a13c52890be0404e83f273144cd5b9960eed01".to_string(),
            TransactionVersion::Mainnet,
        )
        .unwrap()
    }

    #[test]
    fn stacks_mint_test() {
        let p = PegInOp {
            recipient: PrincipalData::Standard(StandardPrincipalData(0, [0u8; 20])),
            peg_wallet_address: pox_address(),
            amount: 55155,
            memo: Vec::default(),
            txid: Txid([0u8; 32]),
            vtxindex: 0,
            block_height: 0,
            burn_header_hash: BurnchainHeaderHash([0; 32]),
        };
        let mut wallet = stacks_wallet();
        let tx = wallet.build_mint_transaction(&p, 0).unwrap();
        tx.verify().unwrap();
    }

    #[test]
    fn stacks_burn_test() {
        // Pulled from testnet (Need valid tx id and signature to build function args)
        // {
        //     "peg_out_request": [
        //       {
        //         "amount": 2918928493838336000,
        //         "recipient": "tb1pmmkznvm0pq5unp6geuwryu2f0m8xr6d229yzg2erx78nnk0ms48sk9s6q7",
        //         "signature": "003c293a89d9ebde9d32d20704a6e18ee38b3fa22444fd44bfcf27259bf0669dc40e4d55e148dc17b8fe3a21333b9593fafc68946c5a255da75d720743e0756a14",
        //         "peg_wallet_address": "tb1qp8r7ln235zx6nd8rsdzkgkrxc238p6eecys2m9",
        //         "fulfillment_fee": 1998900,
        //         "memo": "00",
        //         "txid": "947385b78087c66c4b93cfe2c0939678d36df954cc72354805f9f8d5da04cfc1",
        //         "vtxindex": 15,
        //         "block_height": 2425663,
        //         "burn_header_hash": "00000000000018eb8c1c7d4137b3de6a8544c7fa96f3cd037a58c4c5544e0544"
        //       }
        //     ]
        //   }
        let tx_id =
            hex_bytes("947385b78087c66c4b93cfe2c0939678d36df954cc72354805f9f8d5da04cfc1").unwrap();
        let mut txid_bytes = [0u8; 32];
        txid_bytes.copy_from_slice(tx_id.as_slice());
        let txid = Txid(txid_bytes);

        let signature = MessageSignature::from_hex("003c293a89d9ebde9d32d20704a6e18ee38b3fa22444fd44bfcf27259bf0669dc40e4d55e148dc17b8fe3a21333b9593fafc68946c5a255da75d720743e0756a14").unwrap();

        let p = PegOutRequestOp {
            amount: 0,
            recipient: pox_address(),
            signature,
            peg_wallet_address: pox_address(),
            fulfillment_fee: 1998900,
            memo: vec![0, 0],
            txid,
            vtxindex: 15,
            block_height: 2425663,
            burn_header_hash: BurnchainHeaderHash([0; 32]),
        };
        let mut wallet = stacks_wallet();
        let tx = wallet.build_burn_transaction(&p, 0).unwrap();
        tx.verify().unwrap();
    }

    #[test]
    fn stacks_build_set_address_transaction() {
        let mut wallet = stacks_wallet();
        let tx = wallet
            .build_set_address_transaction(wallet.get_address().clone(), 0)
            .unwrap();
        tx.verify().unwrap();
    }
}
