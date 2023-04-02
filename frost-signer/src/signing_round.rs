use crate::signer::Signer as FrostSigner;
use hashbrown::HashMap;
use p256k1::ecdsa;
use rand_core::{CryptoRng, OsRng, RngCore};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use tracing::{debug, info};
pub use wtfrost;
use wtfrost::{
    common::{PolyCommitment, PublicNonce},
    v1, Scalar,
};

use crate::state_machine::{Error as StateMachineError, StateMachine, States};

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("InvalidPartyID")]
    InvalidPartyID,
    #[error("InvalidDkgPublicShare")]
    InvalidDkgPublicShare,
    #[error("InvalidDkgPrivateShares")]
    InvalidDkgPrivateShares(u32),
    #[error("InvalidNonceResponse")]
    InvalidNonceResponse,
    #[error("InvalidSignatureShare")]
    InvalidSignatureShare,
    #[error("State Machine Error: {0}")]
    StateMachineError(#[from] StateMachineError),
}

pub trait Signable {
    fn hash(&self, hasher: &mut Sha256);

    fn sign(&self, private_key: &Scalar) -> Result<Vec<u8>, ecdsa::Error> {
        let mut hasher = Sha256::new();

        self.hash(&mut hasher);

        let hash = hasher.finalize();
        match ecdsa::Signature::new(hash.as_slice(), private_key) {
            Ok(sig) => Ok(sig.to_bytes().to_vec()),
            Err(e) => Err(e),
        }
    }

    fn verify(&self, signature: &[u8], public_key: &ecdsa::PublicKey) -> bool {
        let mut hasher = Sha256::new();

        self.hash(&mut hasher);

        let hash = hasher.finalize();
        let sig = match ecdsa::Signature::try_from(signature) {
            Ok(sig) => sig,
            Err(_) => return false,
        };

        sig.verify(hash.as_slice(), public_key)
    }
}

pub struct SigningRound {
    pub dkg_id: u64,
    pub dkg_public_id: u64,
    pub sign_id: u64,
    pub sign_nonce_id: u64,
    pub threshold: usize,
    pub total: usize,
    pub signer: Signer,
    pub state: States,
    pub commitments: BTreeMap<u32, PolyCommitment>,
    pub shares: HashMap<u32, HashMap<usize, Scalar>>,
    pub public_nonces: Vec<PublicNonce>,
}

pub struct Signer {
    pub frost_signer: wtfrost::v1::Signer,
    pub signer_id: u32,
}

impl StateMachine for SigningRound {
    fn move_to(&mut self, state: States) -> Result<(), StateMachineError> {
        self.can_move_to(&state)?;
        self.state = state;
        Ok(())
    }

    fn can_move_to(&self, state: &States) -> Result<(), StateMachineError> {
        let prev_state = &self.state;
        let accepted = match state {
            States::Idle => true,
            States::DkgPublicDistribute => {
                prev_state == &States::Idle
                    || prev_state == &States::DkgPublicGather
                    || prev_state == &States::DkgPrivateDistribute
            }
            States::DkgPublicGather => prev_state == &States::DkgPublicDistribute,
            States::DkgPrivateDistribute => prev_state == &States::DkgPublicGather,
            States::DkgPrivateGather => prev_state == &States::DkgPrivateDistribute,
            States::SignGather => prev_state == &States::Idle,
            States::Signed => prev_state == &States::SignGather,
        };
        if accepted {
            info!("state change from {:?} to {:?}", prev_state, state);
            Ok(())
        } else {
            Err(StateMachineError::BadStateChange(format!(
                "{:?} to {:?}",
                prev_state, state
            )))
        }
    }
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub enum DkgStatus {
    Success,
    Failure(String),
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub enum MessageTypes {
    DkgBegin(DkgBegin),
    DkgPrivateBegin(DkgBegin),
    DkgEnd(DkgEnd),
    DkgPublicEnd(DkgEnd),
    DkgQuery(DkgQuery),
    DkgQueryResponse(DkgQueryResponse),
    DkgPublicShare(DkgPublicShare),
    DkgPrivateShares(DkgPrivateShares),
    NonceRequest(NonceRequest),
    NonceResponse(NonceResponse),
    SignShareRequest(SignatureShareRequest),
    SignShareResponse(SignatureShareResponse),
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct DkgPublicShare {
    pub dkg_id: u64,
    pub dkg_public_id: u64,
    pub party_id: u32,
    pub public_share: PolyCommitment,
}

impl Signable for DkgPublicShare {
    fn hash(&self, hasher: &mut Sha256) {
        hasher.update("DKG_PUBLIC_SHARE".as_bytes());
        hasher.update(self.dkg_id.to_be_bytes());
        hasher.update(self.dkg_public_id.to_be_bytes());
        hasher.update(self.party_id.to_be_bytes());
        for a in &self.public_share.A {
            hasher.update(a.compress().as_bytes());
        }
    }
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct DkgPrivateShares {
    pub dkg_id: u64,
    pub key_id: u32,
    pub private_shares: HashMap<usize, Scalar>,
}

impl Signable for DkgPrivateShares {
    fn hash(&self, hasher: &mut Sha256) {
        hasher.update("DKG_PRIVATE_SHARES".as_bytes());
        hasher.update(self.dkg_id.to_be_bytes());
        hasher.update(self.key_id.to_be_bytes());
        for (id, share) in &self.private_shares {
            hasher.update(id.to_be_bytes());
            hasher.update(share.to_bytes());
        }
    }
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct DkgBegin {
    pub dkg_id: u64, //TODO: Strong typing for this, alternatively introduce a type alias
}

impl Signable for DkgBegin {
    fn hash(&self, hasher: &mut Sha256) {
        hasher.update("DKG_BEGIN".as_bytes());
        hasher.update(self.dkg_id.to_be_bytes());
    }
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct DkgEnd {
    pub dkg_id: u64,
    pub signer_id: usize,
    pub status: DkgStatus,
}

impl Signable for DkgEnd {
    fn hash(&self, hasher: &mut Sha256) {
        hasher.update("DKG_END".as_bytes());
        hasher.update(self.dkg_id.to_be_bytes());
        hasher.update(self.signer_id.to_be_bytes());
    }
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct DkgQuery {}

impl Signable for DkgQuery {
    fn hash(&self, hasher: &mut Sha256) {
        hasher.update("DKG_QUERY".as_bytes());
    }
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct DkgQueryResponse {
    pub dkg_id: u64,
    pub public_share: PolyCommitment,
}

impl Signable for DkgQueryResponse {
    fn hash(&self, hasher: &mut Sha256) {
        hasher.update("DKG_QUERY_RESPONSE".as_bytes());
        hasher.update(self.dkg_id.to_be_bytes());
        hasher.update(self.public_share.id.id.to_bytes());
        for a in &self.public_share.A {
            hasher.update(a.compress().as_bytes());
        }
    }
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct NonceRequest {
    pub dkg_id: u64,
    pub sign_id: u64,
    pub sign_nonce_id: u64,
}

impl Signable for NonceRequest {
    fn hash(&self, hasher: &mut Sha256) {
        hasher.update("NONCE_REQUEST".as_bytes());
        hasher.update(self.dkg_id.to_be_bytes());
        hasher.update(self.sign_id.to_be_bytes());
        hasher.update(self.sign_nonce_id.to_be_bytes());
    }
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct NonceResponse {
    pub dkg_id: u64,
    pub sign_id: u64,
    pub sign_nonce_id: u64,
    pub party_id: u32,
    pub nonce: PublicNonce,
}

impl Signable for NonceResponse {
    fn hash(&self, hasher: &mut Sha256) {
        hasher.update("NONCE_RESPONSE".as_bytes());
        hasher.update(self.dkg_id.to_be_bytes());
        hasher.update(self.sign_id.to_be_bytes());
        hasher.update(self.sign_nonce_id.to_be_bytes());
        hasher.update(self.party_id.to_be_bytes());
        hasher.update(self.nonce.D.compress().as_bytes());
        hasher.update(self.nonce.E.compress().as_bytes());
    }
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct SignatureShareRequest {
    pub dkg_id: u64,
    pub sign_id: u64,
    pub correlation_id: u64,
    pub party_id: u32,
    pub nonces: Vec<(u32, PublicNonce)>,
    pub message: Vec<u8>,
}

impl Signable for SignatureShareRequest {
    fn hash(&self, hasher: &mut Sha256) {
        hasher.update("SIGNATURE_SHARE_REQUEST".as_bytes());
        hasher.update(self.dkg_id.to_be_bytes());
        hasher.update(self.sign_id.to_be_bytes());
        hasher.update(self.correlation_id.to_be_bytes());
        hasher.update(self.party_id.to_be_bytes());

        for (id, nonce) in &self.nonces {
            hasher.update(id.to_be_bytes());
            hasher.update(nonce.D.compress().as_bytes());
            hasher.update(nonce.E.compress().as_bytes());
        }

        hasher.update(self.message.as_slice());
    }
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct SignatureShareResponse {
    pub dkg_id: u64,
    pub sign_id: u64,
    pub correlation_id: u64,
    pub party_id: u32,
    pub signature_share: wtfrost::v1::SignatureShare,
}

impl Signable for SignatureShareResponse {
    fn hash(&self, hasher: &mut Sha256) {
        hasher.update("SIGNATURE_SHARE_RESPONSE".as_bytes());
        hasher.update(self.dkg_id.to_be_bytes());
        hasher.update(self.sign_id.to_be_bytes());
        hasher.update(self.correlation_id.to_be_bytes());
        hasher.update(self.party_id.to_be_bytes());
        hasher.update(self.signature_share.id.to_be_bytes());
        hasher.update(self.signature_share.z_i.to_bytes());
    }
}

impl SigningRound {
    pub fn new(
        threshold: usize,
        total: usize,
        signer_id: u32,
        key_ids: Vec<usize>,
    ) -> SigningRound {
        assert!(threshold <= total);
        let mut rng = OsRng::default();
        let frost_signer = v1::Signer::new(&key_ids, total, threshold, &mut rng);
        let signer = Signer {
            frost_signer,
            signer_id,
        };

        SigningRound {
            dkg_id: 1,
            dkg_public_id: 1,
            sign_id: 1,
            sign_nonce_id: 1,
            threshold,
            total,
            signer,
            state: States::Idle,
            commitments: BTreeMap::new(),
            shares: HashMap::new(),
            public_nonces: vec![],
        }
    }

    fn reset<T: RngCore + CryptoRng>(&mut self, dkg_id: u64, rng: &mut T) {
        self.dkg_id = dkg_id;
        self.dkg_public_id = 1;
        self.commitments.clear();
        self.shares.clear();
        self.public_nonces.clear();
        self.signer.frost_signer.reset_polys(rng);
    }

    pub fn process(&mut self, message: MessageTypes) -> Result<Vec<MessageTypes>, Error> {
        let out_msgs = match message {
            MessageTypes::DkgBegin(dkg_begin) => self.dkg_begin(dkg_begin),
            MessageTypes::DkgPrivateBegin(_) => self.dkg_private_begin(),
            MessageTypes::DkgPublicShare(dkg_public_shares) => {
                self.dkg_public_share(dkg_public_shares)
            }
            MessageTypes::DkgPrivateShares(dkg_private_shares) => {
                self.dkg_private_shares(dkg_private_shares)
            }
            MessageTypes::SignShareRequest(sign_share_request) => {
                self.sign_share_request(sign_share_request)
            }
            MessageTypes::NonceRequest(nonce_request) => self.nonce_request(nonce_request),
            _ => Ok(vec![]), // TODO
        };

        match out_msgs {
            Ok(mut out) => {
                if self.public_shares_done() {
                    debug!(
                        "public_shares_done==true. commitments {}",
                        self.commitments.len()
                    );
                    let dkg_end_msgs = self.dkg_public_ended()?;
                    out.push(dkg_end_msgs);
                    self.move_to(States::DkgPrivateDistribute)?;
                } else if self.can_dkg_end() {
                    debug!(
                        "can_dkg_end==true. shares {} commitments {}",
                        self.shares.len(),
                        self.commitments.len()
                    );
                    let dkg_end_msgs = self.dkg_ended()?;
                    out.push(dkg_end_msgs);
                    self.move_to(States::Idle)?;
                }
                Ok(out)
            }
            Err(e) => Err(e),
        }
    }

    fn dkg_public_ended(&mut self) -> Result<MessageTypes, Error> {
        let dkg_end = DkgEnd {
            dkg_id: self.dkg_id,
            signer_id: self.signer.signer_id as usize,
            status: DkgStatus::Success,
        };
        let dkg_end = MessageTypes::DkgPublicEnd(dkg_end);
        info!(
            "DKG_END round #{} signer_id {}",
            self.dkg_id, self.signer.signer_id
        );
        Ok(dkg_end)
    }

    fn dkg_ended(&mut self) -> Result<MessageTypes, Error> {
        for party in &mut self.signer.frost_signer.parties {
            let commitments: Vec<PolyCommitment> = self.commitments.clone().into_values().collect();
            let mut shares: HashMap<usize, Scalar> = HashMap::new();
            for (key_id, key_shares) in &self.shares {
                info!(
                    "building shares with k: {} v: key_shares[{}] len {} keys: {:?}",
                    key_id,
                    party.id,
                    key_shares.len(),
                    key_shares.keys()
                );
                shares.insert(*key_id as usize, key_shares[&party.id]);
            }
            info!(
                "party{}.compute_secret shares_for_id:{:?}",
                party.id,
                shares.keys()
            );
            if let Err(secret_error) = party.compute_secret(shares, &commitments) {
                let dkg_end = DkgEnd {
                    dkg_id: self.dkg_id,
                    signer_id: self.signer.signer_id as usize,
                    status: DkgStatus::Failure(secret_error.to_string()),
                };
                return Ok(MessageTypes::DkgEnd(dkg_end));
            }
            info!("Party #{} group key {}", party.id, party.group_key);
        }
        let dkg_end = DkgEnd {
            dkg_id: self.dkg_id,
            signer_id: self.signer.signer_id as usize,
            status: DkgStatus::Success,
        };
        let dkg_end = MessageTypes::DkgEnd(dkg_end);
        info!(
            "DKG_END round #{} signer_id {}",
            self.dkg_id, self.signer.signer_id
        );
        Ok(dkg_end)
    }

    fn public_shares_done(&self) -> bool {
        debug!(
            "public_shares_done state {:?} commitments {}",
            self.state,
            self.commitments.len(),
        );
        self.state == States::DkgPublicGather && self.commitments.len() == self.total
    }

    fn can_dkg_end(&self) -> bool {
        debug!(
            "can_dkg_end state {:?} commitments {} shares {}",
            self.state,
            self.commitments.len(),
            self.shares.len()
        );
        self.state == States::DkgPrivateGather
            && self.commitments.len() == self.total
            && self.shares.len() == self.total
    }

    fn nonce_request(&mut self, nonce_request: NonceRequest) -> Result<Vec<MessageTypes>, Error> {
        let mut rng = OsRng::default();
        let mut msgs = vec![];
        for party in &mut self.signer.frost_signer.parties {
            let response = NonceResponse {
                dkg_id: nonce_request.dkg_id,
                sign_id: nonce_request.sign_id,
                sign_nonce_id: nonce_request.sign_nonce_id,
                party_id: party.id as u32,
                nonce: party.gen_nonce(&mut rng),
            };

            let response = MessageTypes::NonceResponse(response);

            info!(
                "nonce request with dkg_id {:?}. response sent from party_id {}",
                nonce_request.dkg_id, party.id
            );
            msgs.push(response);
        }
        Ok(msgs)
    }

    fn sign_share_request(
        &mut self,
        sign_request: SignatureShareRequest,
    ) -> Result<Vec<MessageTypes>, Error> {
        let mut msgs = vec![];
        let party_id: usize = sign_request
            .party_id
            .try_into()
            .map_err(|_| Error::InvalidPartyID)?;
        if let Some(party) = self
            .signer
            .frost_signer
            .parties
            .iter()
            .find(|p| p.id == party_id)
        {
            //let party_nonces = &self.public_nonces;
            let signer_ids: Vec<usize> = sign_request
                .nonces
                .iter()
                .map(|(id, _)| *id as usize)
                .collect();
            let signer_nonces: Vec<PublicNonce> =
                sign_request.nonces.iter().map(|(_, n)| n.clone()).collect();
            let share = party.sign(&sign_request.message, &signer_ids, &signer_nonces);

            let response = SignatureShareResponse {
                dkg_id: sign_request.dkg_id,
                sign_id: sign_request.sign_id,
                correlation_id: sign_request.correlation_id,
                party_id: sign_request.party_id,
                signature_share: share,
            };

            let response = MessageTypes::SignShareResponse(response);

            msgs.push(response);
        } else {
            debug!("SignShareRequest for {} dropped.", sign_request.party_id);
        }
        Ok(msgs)
    }

    fn dkg_begin(&mut self, dkg_begin: DkgBegin) -> Result<Vec<MessageTypes>, Error> {
        let mut rng = OsRng::default();

        self.reset(dkg_begin.dkg_id, &mut rng);
        self.move_to(States::DkgPublicDistribute)?;

        let _party_state = self.signer.frost_signer.save();

        self.dkg_public_begin()
    }

    fn dkg_public_begin(&mut self) -> Result<Vec<MessageTypes>, Error> {
        let mut rng = OsRng::default();
        let mut msgs = vec![];
        for party in &self.signer.frost_signer.parties {
            info!(
                "sending dkg round #{} public commitment for party #{}",
                self.dkg_id, party.id
            );

            let public_share = DkgPublicShare {
                dkg_id: self.dkg_id,
                dkg_public_id: self.dkg_public_id,
                party_id: party.id as u32,
                public_share: party.get_poly_commitment(&mut rng),
            };

            let public_share = MessageTypes::DkgPublicShare(public_share);
            msgs.push(public_share);
        }

        self.move_to(States::DkgPublicGather)?;
        Ok(msgs)
    }

    fn dkg_private_begin(&mut self) -> Result<Vec<MessageTypes>, Error> {
        let mut msgs = vec![];
        for party in &self.signer.frost_signer.parties {
            info!("sending dkg private share for party #{}", party.id);
            let private_shares = DkgPrivateShares {
                dkg_id: self.dkg_id,
                key_id: party.id as u32,
                private_shares: party.get_shares(),
            };

            let private_shares = MessageTypes::DkgPrivateShares(private_shares);
            msgs.push(private_shares);
        }

        self.move_to(States::DkgPrivateGather)?;
        Ok(msgs)
    }

    fn dkg_public_share(
        &mut self,
        dkg_public_share: DkgPublicShare,
    ) -> Result<Vec<MessageTypes>, Error> {
        self.commitments
            .insert(dkg_public_share.party_id, dkg_public_share.public_share);
        info!(
            "received party #{} PUBLIC commitments {}/{}",
            dkg_public_share.party_id,
            self.commitments.len(),
            self.total
        );
        Ok(vec![])
    }

    fn dkg_private_shares(
        &mut self,
        dkg_private_shares: DkgPrivateShares,
    ) -> Result<Vec<MessageTypes>, Error> {
        let shares_clone = dkg_private_shares.private_shares.clone();
        self.shares
            .insert(dkg_private_shares.key_id, dkg_private_shares.private_shares);
        info!(
            "received party #{} PRIVATE shares {}/{} {:?}",
            dkg_private_shares.key_id,
            self.shares.len(),
            self.total,
            shares_clone.keys(),
        );
        Ok(vec![])
    }
}

impl From<&FrostSigner> for SigningRound {
    fn from(signer: &FrostSigner) -> Self {
        let signer_id = signer.signer_id;
        assert!(signer_id > 0 && signer_id as usize <= signer.config.total_signers);
        let party_ids = vec![(signer_id * 2 - 2) as usize, (signer_id * 2 - 1) as usize]; // make two party_ids based on signer_id

        assert!(signer.config.keys_threshold <= signer.config.total_keys);
        let mut rng = OsRng::default();
        let frost_signer = v1::Signer::new(
            &party_ids,
            signer.config.total_keys,
            signer.config.keys_threshold,
            &mut rng,
        );

        SigningRound {
            dkg_id: 1,
            dkg_public_id: 1,
            sign_id: 1,
            sign_nonce_id: 1,
            threshold: signer.config.keys_threshold,
            total: signer.config.total_keys,
            signer: Signer {
                frost_signer,
                signer_id,
            },
            state: States::Idle,
            commitments: BTreeMap::new(),
            shares: HashMap::new(),
            public_nonces: vec![],
        }
    }
}

#[cfg(test)]
mod test {
    use hashbrown::HashMap;
    use rand_core::{CryptoRng, OsRng, RngCore};
    use wtfrost::{common::PolyCommitment, schnorr::ID, Scalar};

    use crate::signing_round::{
        DkgPrivateShares, DkgPublicShare, DkgStatus, MessageTypes, SigningRound,
    };
    use crate::state_machine::States;

    fn get_rng() -> impl RngCore + CryptoRng {
        let rnd = OsRng::default();
        //rand::rngs::StdRng::seed_from_u64(rnd.next_u64()) // todo: fix trait `rand_core::RngCore` is not implemented for `StdRng`
        rnd
    }

    #[test]
    fn dkg_public_share() {
        let mut rnd = get_rng();
        let mut signing_round = SigningRound::new(1, 1, 1, vec![1]);
        let public_share = DkgPublicShare {
            dkg_id: 0,
            party_id: 0,
            public_share: PolyCommitment {
                id: ID::new(&Scalar::new(), &Scalar::new(), &mut rnd),
                A: vec![],
            },
            dkg_public_id: 0,
        };
        signing_round.dkg_public_share(public_share).unwrap();
        assert_eq!(1, signing_round.commitments.len())
    }

    #[test]
    fn dkg_private_shares() {
        let mut signing_round = SigningRound::new(1, 1, 1, vec![1]);
        let mut private_shares = DkgPrivateShares {
            dkg_id: 0,
            key_id: 0,
            private_shares: HashMap::new(),
        };
        private_shares.private_shares.insert(1, Scalar::new());
        signing_round.dkg_private_shares(private_shares).unwrap();
        assert_eq!(1, signing_round.shares.len())
    }

    #[test]
    fn public_shares_done() {
        let mut rnd = get_rng();
        let mut signing_round = SigningRound::new(1, 1, 1, vec![1]);
        // publich_shares_done starts out as false
        assert_eq!(false, signing_round.public_shares_done());

        // meet the conditions for all public keys received
        signing_round.state = States::DkgPublicGather;
        signing_round.commitments.insert(
            1,
            PolyCommitment {
                id: ID::new(&Scalar::new(), &Scalar::new(), &mut rnd),
                A: vec![],
            },
        );

        // public_shares_done should be true
        assert!(signing_round.public_shares_done());
    }

    #[test]
    fn can_dkg_end() {
        let mut rnd = get_rng();
        let mut signing_round = SigningRound::new(1, 1, 1, vec![1]);
        // can_dkg_end starts out as false
        assert_eq!(false, signing_round.can_dkg_end());

        // meet the conditions for DKG_END
        signing_round.state = States::DkgPrivateGather;
        signing_round.commitments.insert(
            1,
            PolyCommitment {
                id: ID::new(&Scalar::new(), &Scalar::new(), &mut rnd),
                A: vec![],
            },
        );
        let shares: HashMap<usize, Scalar> = HashMap::new();
        signing_round.shares.insert(1, shares);

        // can_dkg_end should be true
        assert!(signing_round.can_dkg_end());
    }

    #[test]
    fn dkg_ended() {
        let mut signing_round = SigningRound::new(1, 1, 1, vec![1]);
        match signing_round.dkg_ended() {
            Ok(dkg_end) => match dkg_end {
                MessageTypes::DkgEnd(dkg_end) => match dkg_end.status {
                    DkgStatus::Failure(_) => assert!(true),
                    _ => assert!(false),
                },
                _ => assert!(false),
            },
            _ => assert!(false),
        }
    }
}
