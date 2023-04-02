use p256k1::ecdsa;

pub fn parse_public_key(public_key: &str) -> ecdsa::PublicKey {
    ecdsa::PublicKey::try_from(public_key)
        .expect("failed to parse coordinator_public_key from config")
}

pub fn parse_public_keys(public_keys: &[String]) -> Vec<ecdsa::PublicKey> {
    public_keys
        .iter()
        .map(|s| ecdsa::PublicKey::try_from(s.as_str()).expect("failed to parse ecdsa::PublicKey"))
        .collect::<Vec<ecdsa::PublicKey>>()
}
