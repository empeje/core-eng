use bitcoin::{consensus::Encodable, hashes::hex::ToHex};
pub trait BitcoinNode {
    fn broadcast_transaction(&self, tx: &BitcoinTransaction) -> Result<(), Error>;
}

pub type BitcoinTransaction = bitcoin::Transaction;

pub struct LocalhostBitcoinNode {
    bitcoind_api: String,
}

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("IO Error: {0}")]
    IOError(#[from] std::io::Error),
    #[error("RPC call returned an invalid response: {0}")]
    InvalidResponse(String),
}

impl BitcoinNode for LocalhostBitcoinNode {
    fn broadcast_transaction(&self, tx: &BitcoinTransaction) -> Result<(), Error> {
        let mut writer = std::io::Cursor::new(vec![]);
        tx.consensus_encode(&mut writer)?;
        let raw_tx = writer.into_inner().to_hex();

        self.rpc(&self.bitcoind_api, "sendrawtransaction", vec![raw_tx])?;
        Ok(())
    }
}

impl LocalhostBitcoinNode {
    pub fn new(bitcoind_api: String) -> LocalhostBitcoinNode {
        Self { bitcoind_api }
    }

    fn rpc(
        &self,
        url: &str,
        method: &str,
        params: impl ureq::serde::Serialize,
    ) -> Result<serde_json::Value, Error> {
        let rpc = ureq::json!({"jsonrpc": "1.0", "id": "stx", "method": method, "params": params});
        let response = ureq::post(url).send_json(&rpc).map_err(|e| {
            let err_str = e.to_string();
            let error_json = serde_json::json!({ "error": &err_str });
            let err_obj_opt = match e.into_response() {
                Some(r) => r.into_json::<serde_json::Value>().unwrap_or(error_json),
                None => error_json,
            };
            Error::InvalidResponse(err_obj_opt.to_string())
        })?;
        let json = response.into_json::<serde_json::Value>()?;
        let result = json
            .as_object()
            .ok_or_else(|| Error::InvalidResponse("Invalid JSON object".to_string()))?;
        let result_str = result.get("result").ok_or_else(|| {
            Error::InvalidResponse("Expected 'result' entry but none found.".to_string())
        })?;
        Ok(result_str.clone())
    }
}
