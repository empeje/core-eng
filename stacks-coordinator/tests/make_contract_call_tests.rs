use blockstack_lib::vm::{
    types::{ASCIIData, PrincipalData},
    Value,
};
use stacks_coordinator::make_contract_call::{MakeContractCall, SignedContractCallOptions, ANY};

#[test]
fn make_contract_call_test() {
    let mut c = MakeContractCall::new("..").unwrap();

    // Generate some fake data to test the call
    let amount = Value::UInt(42);
    let principal = Value::from(
        PrincipalData::parse_standard_principal("SM2J6ZY48GV1EZ5V2V5RB9MP66SW86PYKKQVX8X0G")
            .unwrap(),
    );
    let tx_id = Value::from(ASCIIData {
        data: vec![0x04; 32],
    });
    let function_args: Vec<Value> = vec![amount, principal, tx_id];

    let input = SignedContractCallOptions::new(
        "SPBMRFRPPGCDE3F384WCJPK8PQJGZ8K9QKK7F59X",
        "sbtc-alpha",
        "mint!",
        &function_args,
        ANY,
        "0001020304050607080910111213141516171819202122232425262728293031",
    )
    .with_fee(0);
    {
        let input_s = serde_json::to_string(&input).unwrap();
        println!("{input_s}");
    }
    let t = c.call(&input).unwrap();
    let s = serde_json::to_string(&t).unwrap();
    let expected = "{\"version\":0,\"chainId\":1,\"auth\":{\"authType\":4,\"spendingCondition\":{\"fee\":\"0\",\"hashMode\":0,\"keyEncoding\":1,\"nonce\":\"0\",\"signature\":{\"data\":\"00ba8c733769a6470efde706b2aa682ce091b33e4888b413dbf2cab221061b883a64d5487f05008bf07e191c427d288b98b15131e84182e7bf7260c2b031cd1027\",\"type\":9},\"signer\":\"12016c066cb72c7098a01564eeadae379a266ec1\"}},\"anchorMode\":3,\"payload\":{\"contractAddress\":{\"hash160\":\"174c3f16b418d70de34138c95a68b5e50fa269bc\",\"type\":0,\"version\":22},\"contractName\":{\"content\":\"sbtc-alpha\",\"lengthPrefixBytes\":1,\"maxLengthBytes\":128,\"type\":2},\"functionArgs\":[{\"type\":1,\"value\":\"42\"},{\"address\":{\"hash160\":\"a46ff88886c2ef9762d970b4d2c63678835bd39d\",\"type\":0,\"version\":20},\"type\":5},{\"data\":\"\\u0004\\u0004\\u0004\\u0004\\u0004\\u0004\\u0004\\u0004\\u0004\\u0004\\u0004\\u0004\\u0004\\u0004\\u0004\\u0004\\u0004\\u0004\\u0004\\u0004\\u0004\\u0004\\u0004\\u0004\\u0004\\u0004\\u0004\\u0004\\u0004\\u0004\\u0004\\u0004\",\"type\":13}],\"functionName\":{\"content\":\"mint!\",\"lengthPrefixBytes\":1,\"maxLengthBytes\":128,\"type\":2},\"payloadType\":2,\"type\":8},\"postConditionMode\":2,\"postConditions\":{\"lengthPrefixBytes\":4,\"type\":7,\"values\":[]}}";
    assert_eq!(s, expected);
}
