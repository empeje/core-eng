use crate::make_contract_call::{
    AnchorMode, Authorization, ChainID, LengthPrefixedList, Payload, PostConditionMode,
    TransactionVersion, ANY, OFF_CHAIN_ONLY, ON_CHAIN_ONLY,
};

use blockstack_lib::{
    chainstate::stacks::{
        SinglesigHashMode, SinglesigSpendingCondition, StacksTransaction as BlockstackTransaction,
        TransactionAnchorMode, TransactionAuth, TransactionContractCall, TransactionPayload,
        TransactionPostConditionMode, TransactionPublicKeyEncoding, TransactionSpendingCondition,
        TransactionVersion as Version,
    },
    types::chainstate::StacksAddress,
    util::{hash::Hash160, secp256k1::MessageSignature},
    vm::{
        types::{ASCIIData, PrincipalData, StandardPrincipalData, Value},
        ClarityName, ContractName,
    },
};

use serde::{Deserialize, Serialize};

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("Invalid Anchor Mode: {0}")]
    InvalidAnchorMode(u8),
    #[error("Invalid Version: {0}")]
    InvalidVersion(u8),
    #[error("JSON serde failure: {0}")]
    InvalidJson(#[from] serde_json::Error),
    #[error("Invalid Auth Type: {0}")]
    InvalidAuthType(u8),
    #[error("Conversion error: {0}")]
    ConversionError(#[from] std::num::ParseIntError),
    #[error("Invalid Public Key Encoding: {0}")]
    InvalidKeyEncoding(u8),
    #[error("Invalid Hash Mode: {0}")]
    InvalidHashMode(u8),
    #[error("Invalid Stacks Address: {0}")]
    InvalidStacksAddress(String),
    #[error("Invalid Contract Name: {0}")]
    InvalidContractName(String),
    #[error("Invalid Function Name: {0}")]
    InvalidFunctionName(String),
    #[error("Invalid Function Argument Type: {0}")]
    InvalidFunctionArgType(u8),
    #[error("Invalid Function Argument")]
    InvalidFunctionArg,
    #[error("Invalid Post Condition Mode: {0}")]
    InvalidPostConditionMode(u8),
}

#[allow(non_snake_case)]
#[derive(Serialize, Deserialize, Debug)]
pub struct StacksTransaction {
    pub version: TransactionVersion,
    pub chainId: ChainID,
    pub auth: Authorization,
    pub anchorMode: AnchorMode,
    pub payload: Payload,
    pub postConditionMode: PostConditionMode,
    pub postConditions: LengthPrefixedList,
}

impl StacksTransaction {
    pub fn to_blockstack_transaction(self) -> Result<BlockstackTransaction, Error> {
        let version = match self.version {
            0x00 => Version::Mainnet,
            0x80 => Version::Testnet,
            _ => return Err(Error::InvalidVersion(self.version)),
        };
        let anchor_mode = match self.anchorMode {
            ANY => TransactionAnchorMode::Any,
            OFF_CHAIN_ONLY => TransactionAnchorMode::OffChainOnly,
            ON_CHAIN_ONLY => TransactionAnchorMode::OnChainOnly,
            _ => return Err(Error::InvalidAnchorMode(self.anchorMode)),
        };
        let auth = serde_json::from_value::<StacksAuth>(self.auth)?.to_blockstack()?;
        let payload = serde_json::from_value::<StacksPayload>(self.payload)?.to_blockstack()?;
        let post_condition_mode = match self.postConditionMode {
            0x01 => TransactionPostConditionMode::Allow,
            0x02 => TransactionPostConditionMode::Deny,
            _ => {
                return Err(Error::InvalidPostConditionMode(self.postConditionMode));
            }
        };
        Ok(BlockstackTransaction {
            version,
            chain_id: self.chainId,
            auth,
            anchor_mode,
            payload,
            post_condition_mode,
            post_conditions: vec![],
        })
    }
}

#[allow(non_snake_case)]
#[derive(Serialize, Deserialize, Debug)]
pub struct StacksAuth {
    authType: u8,
    spendingCondition: serde_json::Value,
}

impl StacksAuth {
    fn to_standard(&self) -> Result<TransactionAuth, Error> {
        //TODO: currently only support single sig. What about multisig?
        let sig: SingleSig = serde_json::from_value::<SingleSig>(self.spendingCondition.clone())?;
        let key_encoding = TransactionPublicKeyEncoding::from_u8(sig.keyEncoding)
            .ok_or_else(|| Error::InvalidKeyEncoding(sig.keyEncoding))?;
        let hash_mode = SinglesigHashMode::from_u8(sig.hashMode)
            .ok_or_else(|| Error::InvalidHashMode(sig.hashMode))?;
        let mut sig_buf = [0u8; 65];
        let sig_bytes = sig.signature.data.as_bytes();
        if sig_bytes.len() < 65 {
            sig_buf.copy_from_slice(&sig_bytes[..]);
        } else {
            sig_buf.copy_from_slice(&sig_bytes[..65]);
        }
        let sig = SinglesigSpendingCondition {
            hash_mode: hash_mode,
            signer: Hash160::from_data(sig.signer.as_bytes()),
            nonce: sig.nonce.parse()?,
            tx_fee: sig.fee.parse()?,
            key_encoding,
            signature: MessageSignature(sig_buf),
        };
        Ok(TransactionAuth::Standard(
            TransactionSpendingCondition::Singlesig(sig),
        ))
    }

    fn to_sponsored(&self) -> Result<TransactionAuth, Error> {
        // TODO: Is this something we should support?
        // Should I output a warning for now about not yet implemented instead?
        return Err(Error::InvalidAuthType(self.authType));
    }

    pub fn to_blockstack(&self) -> Result<TransactionAuth, Error> {
        let auth = match self.authType {
            0x04 => self.to_standard()?,
            0x05 => self.to_sponsored()?,
            _ => return Err(Error::InvalidAuthType(self.authType)),
        };
        Ok(auth)
    }
}

#[allow(non_snake_case)]
#[derive(Serialize, Deserialize, Debug)]
pub struct SingleSig {
    pub hashMode: u8,
    pub signer: String,
    pub nonce: String,
    pub fee: String,
    pub keyEncoding: u8,
    pub signature: Signature,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Signature {
    data: String,
    #[serde(alias = "type")]
    sig_type: u8,
}

#[allow(non_snake_case)]
#[derive(Serialize, Deserialize, Debug)]
pub struct LengthPrefixedString {
    #[serde(alias = "type")]
    data_type: u8, //Always a length prefix string type
    content: String,
    lengthPrefixBytes: u8, //Always 1
    maxLengthBytes: u8,    //Always 128
}

#[allow(non_snake_case)]
#[derive(Serialize, Deserialize, Debug)]
pub struct ContractAddress {
    #[serde(alias = "type")]
    contract_type: u8,
    version: u8,
    hash160: String,
}

#[allow(non_snake_case)]
#[derive(Serialize, Deserialize, Debug)]
pub struct FunctionArg {
    #[serde(alias = "type")]
    arg_type: u8,
    #[serde(alias = "address", alias = "data", alias = "value")]
    arg_value: serde_json::Value,
}

#[allow(non_snake_case)]
#[derive(Debug, Serialize, Deserialize)]
pub struct PrincipalArg {
    #[serde(alias = "type")]
    address_type: u8,
    version: u8,
    hash160: String,
}

pub const UINT_CV: u8 = 1;
pub const STANDARD_PRINCIPAL_CV: u8 = 5;
pub const STRING_ASCII_CV: u8 = 13;

#[allow(non_snake_case)]
#[derive(Serialize, Deserialize, Debug)]
pub struct StacksPayload {
    #[serde(alias = "type")]
    message_type: u8, //Always a payload type
    payloadType: u8, //Always a contract call
    contractAddress: ContractAddress,
    contractName: LengthPrefixedString,
    functionName: LengthPrefixedString,
    functionArgs: Vec<FunctionArg>,
}

impl StacksPayload {
    fn to_blockstack(&self) -> Result<TransactionPayload, Error> {
        let address = StacksAddress {
            version: self.contractAddress.version,
            bytes: Hash160::from_data(self.contractAddress.hash160.as_bytes()),
        };

        let mut function_args = Vec::with_capacity(self.functionArgs.len());
        for val in &self.functionArgs {
            let value = match &val.arg_type {
                &STANDARD_PRINCIPAL_CV => {
                    let principal = serde_json::from_value::<PrincipalArg>(val.arg_value.clone())?;
                    Value::from(PrincipalData::from(StandardPrincipalData(
                        principal.version,
                        Hash160::from_data(principal.hash160.as_bytes()).0,
                    )))
                }
                &UINT_CV => {
                    let amount = val
                        .arg_value
                        .as_str()
                        .ok_or_else(|| Error::InvalidFunctionArg)?;
                    Value::UInt(amount.parse()?)
                }
                &STRING_ASCII_CV => {
                    let data = val
                        .arg_value
                        .as_str()
                        .ok_or_else(|| Error::InvalidFunctionArg)?;
                    Value::from(ASCIIData {
                        data: data.as_bytes().to_vec(),
                    })
                }
                _ => {
                    return Err(Error::InvalidFunctionArgType(val.arg_type));
                }
            };
            function_args.push(value);
        }
        Ok(TransactionPayload::ContractCall(TransactionContractCall {
            address,
            contract_name: ContractName::try_from(self.contractName.content.clone())
                .map_err(|_| Error::InvalidContractName(self.contractName.content.clone()))?,
            function_name: ClarityName::try_from(self.functionName.content.clone())
                .map_err(|_| Error::InvalidFunctionName(self.functionName.content.clone()))?,
            function_args,
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[ignore]
    #[test]
    fn to_blockstack_transaction() {
        let str = "{\"version\":0,\"chainId\":1,\"auth\":{\"authType\":4,\"spendingCondition\":{\"fee\":\"0\",\"hashMode\":0,\"keyEncoding\":1,\"nonce\":\"0\",\"signature\":{\"data\":\"00ba8c733769a6470efde706b2aa682ce091b33e4888b413dbf2cab221061b883a64d5487f05008bf07e191c427d288b98b15131e84182e7bf7260c2b031cd1027\",\"type\":9},\"signer\":\"12016c066cb72c7098a01564eeadae379a266ec1\"}},\"anchorMode\":3,\"payload\":{\"contractAddress\":{\"hash160\":\"174c3f16b418d70de34138c95a68b5e50fa269bc\",\"type\":0,\"version\":22},\"contractName\":{\"content\":\"sbtc-alpha\",\"lengthPrefixBytes\":1,\"maxLengthBytes\":128,\"type\":2},\"functionArgs\":[{\"type\":1,\"value\":\"42\"},{\"address\":{\"hash160\":\"a46ff88886c2ef9762d970b4d2c63678835bd39d\",\"type\":0,\"version\":20},\"type\":5},{\"data\":\"\\u0004\\u0004\\u0004\\u0004\\u0004\\u0004\\u0004\\u0004\\u0004\\u0004\\u0004\\u0004\\u0004\\u0004\\u0004\\u0004\\u0004\\u0004\\u0004\\u0004\\u0004\\u0004\\u0004\\u0004\\u0004\\u0004\\u0004\\u0004\\u0004\\u0004\\u0004\\u0004\",\"type\":13}],\"functionName\":{\"content\":\"mint!\",\"lengthPrefixBytes\":1,\"maxLengthBytes\":128,\"type\":2},\"payloadType\":2,\"type\":8},\"postConditionMode\":2,\"postConditions\":{\"lengthPrefixBytes\":4,\"type\":7,\"values\":[]}}";
        let tx: StacksTransaction = serde_json::from_str(str).unwrap();
        let blockstack_tx = tx.to_blockstack_transaction().unwrap();
        blockstack_tx.verify().unwrap();
    }
}
