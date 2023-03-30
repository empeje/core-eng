use crate::make_contract_call::{
    AnchorMode, Authorization, ChainID, LengthPrefixedList, Payload, PostConditionMode,
    TransactionVersion,
};

use blockstack_lib::chainstate::stacks::StacksTransaction as BlockstackTransaction;

use serde::{Deserialize, Serialize};

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("Invalid Anchor Mode: {0}")]
    InvalidAnchorMode(u8),
}

/// Current type is compatible with stacks.js JSON
/// TODO: Find appropriate type
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
        // Ok(BlockstackTransaction {
        //     version: self.version,
        //     chain_id: self.chainId,
        //     auth: self.auth,
        //     anchor_mode,
        //     payload: self.payload,
        //     post_condition_mode: self.postConditionMode,
        //     post_conditions: self.postConditions,
        // })
        todo!()
    }
}
