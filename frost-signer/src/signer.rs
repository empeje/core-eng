use crate::config::Config;
use crate::net::{Error as HttpNetError, HttpNet, HttpNetListen, Message, Net, NetListen};
use crate::signing_round::{Error as SigningRoundError, MessageTypes, Signable, SigningRound};
use crate::util::{parse_public_key, parse_public_keys};
use p256k1::ecdsa;
use serde::Deserialize;
use std::sync::mpsc;
use std::sync::mpsc::{Receiver, Sender};
use std::thread::spawn;
use std::{thread, time};
use wtfrost::Scalar;

// on-disk format for frost save data
#[derive(Clone, Deserialize, Default, Debug)]
pub struct Signer {
    pub config: Config,
    pub signer_id: u32,
}

impl Signer {
    pub fn new(config: Config, signer_id: u32) -> Self {
        Self { config, signer_id }
    }

    pub fn start_p2p_sync(&mut self) -> Result<(), Error> {
        let signer_public_keys = parse_public_keys(&self.config.signer_public_keys);
        let key_public_keys = parse_public_keys(&self.config.key_public_keys);
        let coordinator_public_key = parse_public_key(&self.config.coordinator_public_key);

        //Create http relay
        let net: HttpNet = HttpNet::new(self.config.http_relay_url.clone());
        let net_queue = HttpNetListen::new(net.clone(), vec![]);
        // thread coordination
        let (tx, rx): (Sender<Message>, Receiver<Message>) = mpsc::channel();

        // start p2p sync
        let id = self.signer_id;
        spawn(move || {
            poll_loop(
                net_queue,
                tx,
                id,
                signer_public_keys,
                key_public_keys,
                coordinator_public_key,
            )
        });

        // listen to p2p messages
        self.start_signing_round(&net, rx)
    }

    fn start_signing_round(&self, net: &HttpNet, rx: Receiver<Message>) -> Result<(), Error> {
        let network_private_key = Scalar::try_from(self.config.network_private_key.as_str())
            .expect("failed to parse network_private_key from config");
        let mut round = SigningRound::from(self);
        loop {
            // Retreive a message from coordinator
            let inbound = rx.recv()?; // blocking
            let outbounds = round.process(inbound.msg)?;
            for out in outbounds {
                let msg = Message {
                    msg: out.clone(),
                    sig: match out {
                        MessageTypes::DkgBegin(msg) | MessageTypes::DkgPrivateBegin(msg) => {
                            msg.sign(&network_private_key).expect("").to_vec()
                        }
                        MessageTypes::DkgEnd(msg) | MessageTypes::DkgPublicEnd(msg) => {
                            msg.sign(&network_private_key).expect("").to_vec()
                        }
                        MessageTypes::DkgQuery(msg) => {
                            msg.sign(&network_private_key).expect("").to_vec()
                        }
                        MessageTypes::DkgQueryResponse(msg) => {
                            msg.sign(&network_private_key).expect("").to_vec()
                        }
                        MessageTypes::DkgPublicShare(msg) => {
                            msg.sign(&network_private_key).expect("").to_vec()
                        }
                        MessageTypes::DkgPrivateShares(msg) => {
                            msg.sign(&network_private_key).expect("").to_vec()
                        }
                        MessageTypes::NonceRequest(msg) => {
                            msg.sign(&network_private_key).expect("").to_vec()
                        }
                        MessageTypes::NonceResponse(msg) => {
                            msg.sign(&network_private_key).expect("").to_vec()
                        }
                        MessageTypes::SignShareRequest(msg) => {
                            msg.sign(&network_private_key).expect("").to_vec()
                        }
                        MessageTypes::SignShareResponse(msg) => {
                            msg.sign(&network_private_key).expect("").to_vec()
                        }
                    },
                };
                net.send_message(msg)?;
            }
        }
    }
}

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("Http Network Error: {0}")]
    HttpNetError(#[from] HttpNetError),

    #[error("Signing Round Error: {0}")]
    SigningRoundError(#[from] SigningRoundError),

    #[error("Failed to retrieve message: {0}")]
    RecvError(#[from] mpsc::RecvError),

    #[error("Failed to send message")]
    SendError,
}

impl From<mpsc::SendError<Message>> for Error {
    fn from(_: mpsc::SendError<Message>) -> Error {
        Error::SendError
    }
}

fn poll_loop(
    mut net: HttpNetListen,
    tx: Sender<Message>,
    id: u32,
    signer_public_keys: Vec<ecdsa::PublicKey>,
    key_public_keys: Vec<ecdsa::PublicKey>,
    coordinator_public_key: ecdsa::PublicKey,
) -> Result<(), Error> {
    const BASE_TIMEOUT: u64 = 2;
    const MAX_TIMEOUT: u64 = 128;
    let mut timeout = BASE_TIMEOUT;
    loop {
        net.poll(id);
        match net.next_message() {
            None => {
                timeout = if timeout == 0 {
                    BASE_TIMEOUT
                } else if timeout >= MAX_TIMEOUT {
                    MAX_TIMEOUT
                } else {
                    timeout * 2
                };
            }
            Some(m) => {
                timeout = 0;
                match &m.msg {
                    MessageTypes::DkgBegin(msg) | MessageTypes::DkgPrivateBegin(msg) => {
                        assert!(msg.verify(&m.sig, &coordinator_public_key))
                    }
                    MessageTypes::DkgEnd(msg) | MessageTypes::DkgPublicEnd(msg) => {
                        assert!(msg.verify(&m.sig, &signer_public_keys[msg.signer_id - 1]))
                    }
                    MessageTypes::DkgPublicShare(msg) => {
                        assert!(msg.verify(&m.sig, &key_public_keys[msg.party_id as usize]))
                    }
                    MessageTypes::DkgPrivateShares(msg) => {
                        assert!(msg.verify(&m.sig, &key_public_keys[msg.key_id as usize]))
                    }
                    MessageTypes::DkgQuery(msg) => {
                        assert!(msg.verify(&m.sig, &coordinator_public_key))
                    }
                    MessageTypes::DkgQueryResponse(msg) => {
                        let key_id = msg.public_share.id.id.get_u32();
                        assert!(msg.verify(&m.sig, &key_public_keys[key_id as usize - 1]));
                    }
                    MessageTypes::NonceRequest(msg) => {
                        assert!(msg.verify(&m.sig, &coordinator_public_key))
                    }
                    MessageTypes::NonceResponse(msg) => {
                        assert!(msg.verify(&m.sig, &key_public_keys[msg.party_id as usize]))
                    }
                    MessageTypes::SignShareRequest(msg) => {
                        assert!(msg.verify(&m.sig, &coordinator_public_key))
                    }
                    MessageTypes::SignShareResponse(msg) => {
                        assert!(msg.verify(&m.sig, &key_public_keys[msg.party_id as usize]))
                    }
                }

                tx.send(m)?;
            }
        };
        thread::sleep(time::Duration::from_millis(timeout));
    }
}
