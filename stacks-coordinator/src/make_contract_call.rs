use std::path::Path;

use crate::stacks_transaction::StacksTransaction;
use serde::Serialize;
use yarpc::{dispatch_command::DispatchCommand, js::Js, rpc::Rpc};

use blockstack_lib::{
    chainstate::stacks::TransactionPostConditionMode,
    vm::{database::ClaritySerializable, Value},
};

pub type ClarityValue = String;

pub type PostCondition = serde_json::Value;

// number | string | bigint | Uint8Array | BN;
pub type IntegerType = String;

pub type StacksNetworkNameOrStacksNetwork = String;

pub type BooleanOrClarityAbi = serde_json::Value;

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("IO Error: {0}")]
    IO(#[from] std::io::Error),
    #[error("Invalid Path: {0}")]
    InvalidPath(std::path::PathBuf),
}

#[allow(non_snake_case)]
#[derive(Serialize, Debug)]
pub struct SignedContractCallOptions {
    pub contractAddress: String,

    pub contractName: String,

    pub functionName: String,

    pub functionArgs: Vec<ClarityValue>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub fee: Option<IntegerType>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub feeEstimateApiUrl: Option<String>,

    pub nonce: IntegerType,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub network: Option<StacksNetworkNameOrStacksNetwork>,

    pub anchorMode: AnchorMode,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub postConditionMode: Option<PostConditionMode>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub postConditions: Option<PostCondition>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub validateWithAbi: Option<BooleanOrClarityAbi>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub sponsored: Option<bool>,

    pub senderKey: String,
}

impl SignedContractCallOptions {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        contract_address: impl Into<String>,
        contract_name: impl Into<String>,
        function_name: impl Into<String>,
        function_args: &[Value],
        anchor_mode: AnchorMode,
        sender_key: impl Into<String>,
        nonce: u64,
        network: impl Into<String>,
    ) -> Self {
        Self {
            contractAddress: contract_address.into(),
            contractName: contract_name.into(),
            functionName: function_name.into(),
            functionArgs: function_args
                .iter()
                .map(ClaritySerializable::serialize)
                .collect(),
            fee: None,
            feeEstimateApiUrl: None,
            nonce: nonce.to_string(),
            network: Some(network.into()),
            anchorMode: anchor_mode,
            postConditionMode: Some(TransactionPostConditionMode::Allow as u8),
            postConditions: Some(Vec::<serde_json::Value>::new().into()),
            validateWithAbi: Some(false.into()),
            sponsored: Some(false),
            senderKey: sender_key.into(),
        }
    }

    pub fn with_fee(mut self, fee: u128) -> Self {
        self.fee = Some(fee.to_string());
        self
    }
}

pub type TransactionVersion = u8;

pub type ChainID = u32;

pub type Authorization = serde_json::Value;

pub type AnchorMode = u8;

pub const ON_CHAIN_ONLY: AnchorMode = 1;
pub const OFF_CHAIN_ONLY: AnchorMode = 2;
pub const ANY: AnchorMode = 3;

pub type Payload = serde_json::Value;

pub type PostConditionMode = u8;

pub type LengthPrefixedList = serde_json::Value;

pub struct MakeContractCall(Js);

impl MakeContractCall {
    pub fn call(&mut self, input: &SignedContractCallOptions) -> Result<StacksTransaction, Error> {
        Ok(self
            .0
            .call(&DispatchCommand("makeContractCall".to_string(), input))?)
    }

    pub fn new(path: &str) -> Result<Self, Error> {
        let file_name = Path::new(path).join("yarpc/js/stacks/transactions.ts");
        Ok(Self(Js::new(
            file_name
                .clone()
                .to_str()
                .ok_or_else(|| Error::InvalidPath(file_name))?,
        )?))
    }
}
