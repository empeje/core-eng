use blockstack_lib::{
    burnchains::Txid,
    chainstate::{
        burn::operations::{PegInOp, PegOutRequestOp},
        stacks::address::PoxAddress,
    },
    types::chainstate::{BurnchainHeaderHash, StacksAddress},
    util::{hash::Hash160, secp256k1::MessageSignature},
    vm::types::{PrincipalData, StandardPrincipalData},
};
use stacks_coordinator::{
    peg_wallet::{PegWalletAddress, StacksWallet as StacksWalletTrait},
    stacks_wallet::StacksWallet,
};

fn pox_address() -> PoxAddress {
    PoxAddress::Standard(StacksAddress::new(0, Hash160::from_data(&[0; 20])), None)
}

fn stacks_wallet() -> StacksWallet {
    StacksWallet::new(
        "..",
        "SP3FBR2AGK5H9QBDH3EEN6DF8EK8JY7RX8QJ5SVTE.sbtc_alpha".to_string(),
        "0001020304050607080910111213141516171819202122232425262728293031".to_string(),
    )
    .unwrap()
}

#[test]
fn stacks_mint_test() {
    let p = PegInOp {
        recipient: PrincipalData::Standard(StandardPrincipalData(0, [0; 20])),
        peg_wallet_address: pox_address(),
        amount: 0,
        memo: Vec::default(),
        txid: Txid([0; 32]),
        vtxindex: 0,
        block_height: 0,
        burn_header_hash: BurnchainHeaderHash([0; 32]),
    };
    let mut wallet = stacks_wallet();
    let _result = wallet.build_mint_transaction(&p);
    // assert_eq!(result, "Mint");
}

#[test]
fn stacks_burn_test() {
    let p = PegOutRequestOp {
        amount: 0,
        recipient: pox_address(),
        signature: MessageSignature([0; 65]),
        peg_wallet_address: pox_address(),
        fulfillment_fee: 0,
        memo: Vec::default(),
        txid: Txid([0; 32]),
        vtxindex: 0,
        block_height: 0,
        burn_header_hash: BurnchainHeaderHash([0; 32]),
    };
    let mut wallet = stacks_wallet();
    let _result = wallet.build_burn_transaction(&p);
    // assert_eq!(result, "Burn");
}

#[test]
fn stacks_set_wallet_address_test() {
    let p = PegWalletAddress([0; 32]);
    let mut wallet = stacks_wallet();
    let _result = wallet.build_set_address_transaction(p);
    // assert_eq!(result, "SetWalletAddress");
}
