use crate::connection::ConnectionManager;
use crate::error::{NetworkError, NetworkResult};
use crate::security::encryption::{self, EncryptedPacket, EncryptionProvider};
use crate::security::SecurityManager;
#[cfg(test)]
use crate::transport::Transport;
use std::net::SocketAddr;
use std::sync::{Arc, Weak};
use tokio::sync::RwLock;
use tracing::warn;

#[cfg(test)]
use std::collections::HashMap;
#[cfg(test)]
use tokio::sync::Mutex;

/// Helper that encapsulates LAN message encryption/decryption using the shared security manager.
#[derive(Clone, Default)]
pub struct LanCrypto {
    security: Option<Arc<SecurityManager>>,
    connections: Option<Weak<RwLock<ConnectionManager>>>,
    #[cfg(test)]
    overrides: Arc<Mutex<HashMap<SocketAddr, u8>>>,
}

impl LanCrypto {
    /// Build a new helper from the optional security manager and connection manager handle.
    pub fn new(
        security: Option<Arc<SecurityManager>>,
        connections: Option<Arc<RwLock<ConnectionManager>>>,
    ) -> Self {
        Self {
            security,
            connections: connections.map(|arc| Arc::downgrade(&arc)),
            #[cfg(test)]
            overrides: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Obtain the shared encryption provider if security is enabled.
    fn provider(&self) -> Option<Arc<EncryptionProvider>> {
        self.security.as_ref().map(|sec| sec.encryption_provider())
    }

    async fn session_key(&self, addr: SocketAddr) -> Option<[u8; 32]> {
        let security = match self.security.as_ref() {
            Some(sec) => Arc::clone(sec),
            None => return None,
        };

        if let Some(manager) = self.connections.as_ref().and_then(|weak| weak.upgrade()) {
            let guard = manager.read().await;
            if let Some(player_id) = guard.player_id_for_addr(addr).await {
                return match security.secure_session_key(player_id).await {
                    Ok(key) => Some(key),
                    Err(err) => {
                        warn!(
                            context = "lan",
                            %addr,
                            %player_id,
                            "No secure session for LAN peer: {}",
                            err
                        );
                        None
                    }
                };
            }
        }

        #[cfg(test)]
        if let Some(player_id) = self.test_override_player(addr).await {
            return match security.secure_session_key(player_id).await {
                Ok(key) => Some(key),
                Err(err) => {
                    warn!(
                        context = "lan",
                        %addr,
                        %player_id,
                        "No secure session for LAN peer: {}",
                        err
                    );
                    None
                }
            };
        }

        None
    }

    /// Encode a payload for transmission, returning an envelope that can be sent over UDP.
    pub async fn encode(&self, payload: &[u8], target: SocketAddr) -> Vec<u8> {
        let Some(provider) = self.provider() else {
            return encryption::encode_plain_envelope(payload);
        };

        let session_key = self.session_key(target).await;
        match provider.encrypt(payload, session_key).await {
            Ok(packet) => encryption::encode_encrypted_envelope(&packet),
            Err(err) => {
                warn!(context = "lan", %target, "Failed to encrypt LAN payload: {}", err);
                encryption::encode_plain_envelope(payload)
            }
        }
    }

    /// Decode an incoming envelope, yielding the plaintext payload.
    pub async fn decode(&self, bytes: &[u8], sender: SocketAddr) -> NetworkResult<Vec<u8>> {
        match encryption::decode_envelope(bytes) {
            Ok(encryption::Envelope::Plain(data)) => Ok(data.to_vec()),
            Ok(encryption::Envelope::Encrypted {
                key_id,
                nonce,
                payload,
            }) => {
                let security = self.security.as_ref().ok_or_else(|| {
                    NetworkError::security(
                        "Encrypted LAN payload received without security manager",
                    )
                })?;
                let provider = security.encryption_provider();
                let packet = EncryptedPacket {
                    key_id,
                    nonce,
                    payload: payload.to_vec(),
                };

                let result = if key_id == 0 {
                    let session_key = self.session_key(sender).await.ok_or_else(|| {
                        NetworkError::security("Missing secure session for encrypted LAN payload")
                    })?;
                    provider.decrypt_with_session(&packet, &session_key).await
                } else {
                    provider.decrypt(&packet).await
                };

                result.map_err(|err| {
                    NetworkError::security(format!(
                        "Failed to decrypt LAN payload from {}: {}",
                        sender, err
                    ))
                })
            }
            Err(err) => {
                if matches!(err, NetworkError::Security { .. }) {
                    return Ok(bytes.to_vec());
                }
                Err(err)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rustls::crypto::ring;

    fn sample_addr() -> SocketAddr {
        SocketAddr::from(([127, 0, 0, 1], 45678))
    }

    #[tokio::test]
    async fn plain_roundtrip_without_security() {
        let crypto = LanCrypto::default();
        let payload = b"lan-test";

        let encoded = crypto.encode(payload, sample_addr()).await;
        match encryption::decode_envelope(&encoded).expect("decode envelope") {
            encryption::Envelope::Plain(bytes) => assert_eq!(bytes, payload),
            encryption::Envelope::Encrypted { .. } => panic!("expected plain envelope"),
        }

        let decoded = crypto
            .decode(&encoded, sample_addr())
            .await
            .expect("decode should succeed");
        assert_eq!(decoded, payload);
    }

    #[tokio::test]
    async fn encrypted_roundtrip_with_session_key() {
        const LOCAL_ID: u8 = 0;
        const REMOTE_ID: u8 = 1;

        ring::default_provider().install_default().ok();

        let local = Arc::new(SecurityManager::new().expect("local security manager"));
        let remote = Arc::new(SecurityManager::new().expect("remote security manager"));

        let remote_name = "remote";
        let remote_token = local.generate_auth_token(remote_name);
        local
            .authenticate_player(
                REMOTE_ID,
                remote_name,
                remote_token,
                remote.identity_public_key().as_bytes().to_vec(),
            )
            .await
            .expect("authenticate remote");

        let local_name = "local";
        let local_token = remote.generate_auth_token(local_name);
        remote
            .authenticate_player(
                LOCAL_ID,
                local_name,
                local_token,
                local.identity_public_key().as_bytes().to_vec(),
            )
            .await
            .expect("authenticate local");

        let initiate = local
            .initiate_key_exchange(REMOTE_ID)
            .await
            .expect("initiate exchange");
        let response = remote
            .handle_key_exchange_initiate(initiate, LOCAL_ID)
            .await
            .expect("respond to exchange");
        let confirm = local
            .handle_key_exchange_response(response)
            .await
            .expect("handle response");
        remote
            .confirm_key_exchange(confirm)
            .await
            .expect("confirm exchange");

        let crypto = LanCrypto::new(Some(local.clone()), None);
        crypto.register_override(sample_addr(), REMOTE_ID).await;

        let session_key = local
            .secure_session_key(REMOTE_ID)
            .await
            .expect("local session key available");
        assert_eq!(session_key.len(), 32);

        let payload = b"lan-session";
        let encoded = crypto.encode(payload, sample_addr()).await;
        match encryption::decode_envelope(&encoded).expect("decode envelope") {
            encryption::Envelope::Plain(_) => panic!("expected encrypted envelope"),
            encryption::Envelope::Encrypted { key_id, .. } => {
                assert_eq!(key_id, 0, "session keys should produce provider key id 0");
            }
        }

        let decoded = crypto
            .decode(&encoded, sample_addr())
            .await
            .expect("decode via session key");
        assert_eq!(decoded, payload);
    }

    #[tokio::test]
    async fn encrypted_roundtrip_with_connection_manager() {
        const LOCAL_ID: u8 = 0;
        const REMOTE_ID: u8 = 1;

        ring::default_provider().install_default().ok();

        let local = Arc::new(SecurityManager::new().expect("local security manager"));
        let remote = Arc::new(SecurityManager::new().expect("remote security manager"));

        let remote_name = "remote";
        let remote_token = local.generate_auth_token(remote_name);
        local
            .authenticate_player(
                REMOTE_ID,
                remote_name,
                remote_token,
                remote.identity_public_key().as_bytes().to_vec(),
            )
            .await
            .expect("authenticate remote");

        let local_name = "local";
        let local_token = remote.generate_auth_token(local_name);
        remote
            .authenticate_player(
                LOCAL_ID,
                local_name,
                local_token,
                local.identity_public_key().as_bytes().to_vec(),
            )
            .await
            .expect("authenticate local");

        let initiate = local
            .initiate_key_exchange(REMOTE_ID)
            .await
            .expect("initiate exchange");
        let response = remote
            .handle_key_exchange_initiate(initiate, LOCAL_ID)
            .await
            .expect("respond to exchange");
        let confirm = local
            .handle_key_exchange_response(response)
            .await
            .expect("handle response");
        remote
            .confirm_key_exchange(confirm)
            .await
            .expect("confirm exchange");

        let mut manager = ConnectionManager::new_with_transport(Arc::new(
            Transport::new().await.expect("transport"),
        ))
        .await
        .expect("connection manager");
        manager.set_security_manager(local.clone());
        manager.register_test_peer(REMOTE_ID, sample_addr()).await;

        let manager = Arc::new(RwLock::new(manager));
        let crypto = LanCrypto::new(Some(local.clone()), Some(manager.clone()));

        let payload = b"lan-manager";
        let encoded = crypto.encode(payload, sample_addr()).await;
        match encryption::decode_envelope(&encoded).expect("decode envelope") {
            encryption::Envelope::Plain(_) => panic!("expected encrypted envelope"),
            encryption::Envelope::Encrypted { key_id, .. } => {
                assert_eq!(key_id, 0, "session keys should produce provider key id 0");
            }
        }

        let decoded = crypto
            .decode(&encoded, sample_addr())
            .await
            .expect("decode via connection manager mapping");
        assert_eq!(decoded, payload);
    }
}

#[cfg(test)]
impl LanCrypto {
    pub async fn register_override(&self, addr: SocketAddr, player_id: u8) {
        self.overrides.lock().await.insert(addr, player_id);
    }

    async fn test_override_player(&self, addr: SocketAddr) -> Option<u8> {
        self.overrides.lock().await.get(&addr).copied()
    }
}
