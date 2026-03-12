//! Secure key exchange using modern 2025 cryptographic standards
//!
//! This module implements secure key exchange protocols using X25519 for key agreement
//! and Ed25519 for digital signatures, providing perfect forward secrecy and mutual
//! authentication for real-time gaming applications.

use crate::error::{NetworkError, NetworkResult};
use ed25519_dalek::{Signature, Signer, SigningKey, Verifier, VerifyingKey as Ed25519PublicKey};
use ring::{digest, rand::SystemRandom};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{info, warn};
use uuid::Uuid;
use x25519_dalek::{EphemeralSecret, PublicKey as X25519PublicKey};

/// Key exchange algorithm
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum KeyExchangeAlgorithm {
    /// X25519 with Ed25519 signatures (recommended)
    X25519Ed25519,
    /// No key exchange - use pre-shared keys
    PreShared,
}

impl Default for KeyExchangeAlgorithm {
    fn default() -> Self {
        Self::X25519Ed25519
    }
}

/// Key exchange configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeyExchangeConfig {
    /// Algorithm to use for key exchange
    pub algorithm: KeyExchangeAlgorithm,
    /// Session timeout for key exchange
    pub session_timeout_seconds: u64,
    /// Enable perfect forward secrecy
    pub enable_pfs: bool,
    /// Maximum concurrent key exchange sessions
    pub max_concurrent_sessions: usize,
    /// Pre-shared key (for testing/LAN mode)
    pub pre_shared_key: Option<Vec<u8>>,
}

impl Default for KeyExchangeConfig {
    fn default() -> Self {
        Self {
            algorithm: KeyExchangeAlgorithm::X25519Ed25519,
            session_timeout_seconds: 300, // 5 minutes
            enable_pfs: true,
            max_concurrent_sessions: 100,
            pre_shared_key: None,
        }
    }
}

/// Key exchange session state
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum KeyExchangeState {
    /// Session initialized
    Initialized,
    /// Handshake initiated
    HandshakeInitiated,
    /// Handshake response sent
    HandshakeResponse,
    /// Key exchange completed
    Completed,
    /// Session failed
    Failed,
    /// Session expired
    Expired,
}

/// Key exchange session
pub struct KeyExchangeSession {
    /// Session ID
    pub session_id: Uuid,
    /// Remote peer ID
    pub peer_id: u8,
    /// Current state
    pub state: KeyExchangeState,
    /// Created timestamp
    pub created_at: std::time::SystemTime,
    /// Last activity timestamp
    pub last_activity: std::time::SystemTime,
    /// Our ephemeral secret (X25519)
    ephemeral_secret: Option<EphemeralSecret>,
    /// Remote ephemeral public key
    remote_ephemeral_public: Option<X25519PublicKey>,
    /// Derived shared secret
    shared_secret: Option<[u8; 32]>,
    /// Session nonce for uniqueness
    session_nonce: [u8; 16],
}

impl std::fmt::Debug for KeyExchangeSession {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("KeyExchangeSession")
            .field("session_id", &self.session_id)
            .field("peer_id", &self.peer_id)
            .field("state", &self.state)
            .field("created_at", &self.created_at)
            .field("last_activity", &self.last_activity)
            .field("ephemeral_secret", &"<redacted>")
            .field(
                "remote_ephemeral_public",
                &self.remote_ephemeral_public.is_some(),
            )
            .field("shared_secret", &self.shared_secret.is_some())
            .field("session_nonce", &"<redacted>")
            .finish()
    }
}

impl KeyExchangeSession {
    /// Create new key exchange session
    pub fn new(session_id: Uuid, peer_id: u8) -> NetworkResult<Self> {
        let rng = SystemRandom::new();
        let mut session_nonce = [0u8; 16];
        ring::rand::SecureRandom::fill(&rng, &mut session_nonce).map_err(|e| {
            NetworkError::security(format!("failed to generate session nonce: {:?}", e))
        })?;

        Ok(Self {
            session_id,
            peer_id,
            state: KeyExchangeState::Initialized,
            created_at: std::time::SystemTime::now(),
            last_activity: std::time::SystemTime::now(),
            ephemeral_secret: None,
            remote_ephemeral_public: None,
            shared_secret: None,
            session_nonce,
        })
    }

    /// Check if session is expired
    pub fn is_expired(&self, timeout_seconds: u64) -> bool {
        match self.last_activity.elapsed() {
            Ok(elapsed) => elapsed.as_secs() >= timeout_seconds,
            Err(_) => true, // Clock went backwards, consider expired
        }
    }

    /// Update last activity
    pub fn update_activity(&mut self) {
        self.last_activity = std::time::SystemTime::now();
    }

    /// Get derived encryption key material
    pub fn get_encryption_key(&self) -> Option<[u8; 32]> {
        self.shared_secret
    }
}

/// Key exchange handshake messages
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum KeyExchangeMessage {
    /// Initiate handshake
    Initiate {
        session_id: Uuid,
        client_public_key: Vec<u8>, // X25519 public key
        client_identity: Vec<u8>,   // Ed25519 public key
        session_nonce: [u8; 16],
        signature: Vec<u8>, // Ed25519 signature
    },
    /// Respond to handshake
    Response {
        session_id: Uuid,
        server_public_key: Vec<u8>, // X25519 public key
        server_identity: Vec<u8>,   // Ed25519 public key
        session_nonce: [u8; 16],
        signature: Vec<u8>, // Ed25519 signature
    },
    /// Confirm key exchange completion
    Confirm {
        session_id: Uuid,
        confirmation_hash: Vec<u8>, // Hash of shared secret + session data
        signature: Vec<u8>,         // Ed25519 signature
    },
    /// Key exchange error
    Error {
        session_id: Uuid,
        error_message: String,
    },
}

/// Secure key exchange provider
pub struct KeyExchangeProvider {
    /// Configuration
    config: KeyExchangeConfig,
    /// Our long-term identity keypair (Ed25519)
    identity_keypair: SigningKey,
    /// Active key exchange sessions
    sessions: Arc<RwLock<HashMap<Uuid, KeyExchangeSession>>>,
    /// Trusted peer identities (peer_id -> Ed25519 public key)
    trusted_identities: Arc<RwLock<HashMap<u8, Ed25519PublicKey>>>,
}

impl KeyExchangeProvider {
    /// Create new key exchange provider
    pub fn new() -> NetworkResult<Self> {
        Self::with_config(KeyExchangeConfig::default())
    }

    /// Create key exchange provider with configuration
    pub fn with_config(config: KeyExchangeConfig) -> NetworkResult<Self> {
        let rng = SystemRandom::new();

        // Generate long-term identity keypair
        let mut identity_seed = [0u8; 32];
        ring::rand::SecureRandom::fill(&rng, &mut identity_seed).map_err(|e| {
            NetworkError::security(format!("failed to generate identity seed: {:?}", e))
        })?;

        let identity_keypair = SigningKey::from_bytes(&identity_seed);

        info!(
            "Key exchange provider initialized with algorithm: {:?}",
            config.algorithm
        );
        info!(
            "Identity public key: {}",
            hex::encode(identity_keypair.verifying_key().as_bytes())
        );

        Ok(Self {
            config,
            identity_keypair,
            sessions: Arc::new(RwLock::new(HashMap::new())),
            trusted_identities: Arc::new(RwLock::new(HashMap::new())),
        })
    }

    /// Get our identity public key
    pub fn get_identity_public_key(&self) -> Ed25519PublicKey {
        self.identity_keypair.verifying_key().clone()
    }

    /// Add trusted peer identity
    pub async fn add_trusted_identity(&self, peer_id: u8, public_key: Ed25519PublicKey) {
        let mut identities = self.trusted_identities.write().await;
        identities.insert(peer_id, public_key);
        info!(
            "Added trusted identity for peer {}: {}",
            peer_id,
            hex::encode(public_key.as_bytes())
        );
    }

    /// Remove trusted peer identity
    pub async fn remove_trusted_identity(&self, peer_id: u8) -> bool {
        let mut identities = self.trusted_identities.write().await;
        identities.remove(&peer_id).is_some()
    }

    /// Initiate key exchange with peer
    pub async fn initiate_key_exchange(&self, peer_id: u8) -> NetworkResult<KeyExchangeMessage> {
        if self.config.algorithm == KeyExchangeAlgorithm::PreShared {
            return Err(NetworkError::security(
                "key exchange not needed with pre-shared keys",
            ));
        }

        // Check session limit
        {
            let sessions = self.sessions.read().await;
            if sessions.len() >= self.config.max_concurrent_sessions {
                return Err(NetworkError::resource_exhausted(
                    "too many concurrent key exchange sessions",
                ));
            }
        }

        // Create new session
        let session_id = Uuid::new_v4();
        let mut session = KeyExchangeSession::new(session_id, peer_id)?;

        // Generate ephemeral keypair
        let ephemeral_secret = EphemeralSecret::random_from_rng(&mut rand::rngs::OsRng);
        let ephemeral_public = X25519PublicKey::from(&ephemeral_secret);

        session.ephemeral_secret = Some(ephemeral_secret);
        session.state = KeyExchangeState::HandshakeInitiated;
        session.update_activity();

        // Create signed initiate message
        let client_public_key = ephemeral_public.as_bytes().to_vec();
        let client_identity = self.identity_keypair.verifying_key().as_bytes().to_vec();
        let session_nonce = session.session_nonce;

        // Sign the handshake data
        let signature_data = self.create_initiate_signature_data(
            &session_id,
            &client_public_key,
            &client_identity,
            &session_nonce,
        )?;
        let signature = self
            .identity_keypair
            .sign(&signature_data)
            .to_bytes()
            .to_vec();

        // Store session
        {
            let mut sessions = self.sessions.write().await;
            sessions.insert(session_id, session);
        }

        info!(
            "Initiated key exchange with peer {} (session: {})",
            peer_id, session_id
        );

        Ok(KeyExchangeMessage::Initiate {
            session_id,
            client_public_key,
            client_identity,
            session_nonce,
            signature,
        })
    }

    /// Handle key exchange initiate message
    pub async fn handle_initiate(
        &self,
        message: KeyExchangeMessage,
        peer_id: u8,
    ) -> NetworkResult<KeyExchangeMessage> {
        if let KeyExchangeMessage::Initiate {
            session_id,
            client_public_key,
            client_identity,
            session_nonce,
            signature,
        } = message
        {
            // Verify client identity
            let client_ed25519_public = Ed25519PublicKey::try_from(client_identity.as_slice())
                .map_err(|e| {
                    NetworkError::security(format!("invalid client identity key: {:?}", e))
                })?;

            // Check if client is trusted
            {
                let identities = self.trusted_identities.read().await;
                if let Some(trusted_key) = identities.get(&peer_id) {
                    if trusted_key.as_bytes() != client_ed25519_public.as_bytes() {
                        return Err(NetworkError::security("client identity not trusted"));
                    }
                } else {
                    warn!(
                        "Peer {} not in trusted identities, proceeding with caution",
                        peer_id
                    );
                }
            }

            // Verify signature
            let signature_data = self.create_initiate_signature_data(
                &session_id,
                &client_public_key,
                &client_identity,
                &session_nonce,
            )?;
            let signature = Signature::try_from(signature.as_slice())
                .map_err(|e| NetworkError::security(format!("invalid signature: {:?}", e)))?;

            client_ed25519_public
                .verify(&signature_data, &signature)
                .map_err(|e| {
                    NetworkError::security(format!("signature verification failed: {}", e))
                })?;

            // Create response session
            let mut session = KeyExchangeSession::new(session_id, peer_id)?;
            session.state = KeyExchangeState::HandshakeResponse;
            session.update_activity();

            // Store client's ephemeral public key
            let client_ephemeral_public = X25519PublicKey::from(
                <[u8; 32]>::try_from(client_public_key.as_slice())
                    .map_err(|_| NetworkError::security("invalid client public key length"))?,
            );
            session.remote_ephemeral_public = Some(client_ephemeral_public);

            // Generate our ephemeral keypair
            let ephemeral_secret = EphemeralSecret::random_from_rng(&mut rand::rngs::OsRng);
            let ephemeral_public = X25519PublicKey::from(&ephemeral_secret);

            // Derive shared secret
            let shared_secret = ephemeral_secret.diffie_hellman(&client_ephemeral_public);
            session.shared_secret = Some(*shared_secret.as_bytes());
            // Note: ephemeral secret is consumed and not stored for security

            // Create response message
            let server_public_key = ephemeral_public.as_bytes().to_vec();
            let server_identity = self.identity_keypair.verifying_key().as_bytes().to_vec();
            let server_nonce = session.session_nonce;

            // Sign response
            let response_signature_data = self.create_response_signature_data(
                &session_id,
                &server_public_key,
                &server_identity,
                &server_nonce,
            )?;
            let response_signature = self
                .identity_keypair
                .sign(&response_signature_data)
                .to_bytes()
                .to_vec();

            // Store session
            {
                let mut sessions = self.sessions.write().await;
                sessions.insert(session_id, session);
            }

            info!(
                "Responded to key exchange from peer {} (session: {})",
                peer_id, session_id
            );

            Ok(KeyExchangeMessage::Response {
                session_id,
                server_public_key,
                server_identity,
                session_nonce: server_nonce,
                signature: response_signature,
            })
        } else {
            Err(NetworkError::security("expected initiate message"))
        }
    }

    /// Handle key exchange response message
    pub async fn handle_response(
        &self,
        message: KeyExchangeMessage,
    ) -> NetworkResult<KeyExchangeMessage> {
        if let KeyExchangeMessage::Response {
            session_id,
            server_public_key,
            server_identity,
            session_nonce,
            signature,
        } = message
        {
            // Get our session
            let mut sessions = self.sessions.write().await;
            let session = sessions
                .get_mut(&session_id)
                .ok_or_else(|| NetworkError::security("unknown session"))?;

            if session.state != KeyExchangeState::HandshakeInitiated {
                return Err(NetworkError::security("invalid session state for response"));
            }

            // Verify server identity
            let server_ed25519_public = Ed25519PublicKey::try_from(server_identity.as_slice())
                .map_err(|e| {
                    NetworkError::security(format!("invalid server identity key: {:?}", e))
                })?;

            // Check if server is trusted
            {
                let identities = self.trusted_identities.read().await;
                if let Some(trusted_key) = identities.get(&session.peer_id) {
                    if trusted_key.as_bytes() != server_ed25519_public.as_bytes() {
                        return Err(NetworkError::security("server identity not trusted"));
                    }
                } else {
                    warn!(
                        "Server peer {} not in trusted identities, proceeding with caution",
                        session.peer_id
                    );
                }
            }

            // Verify signature
            let signature_data = self.create_response_signature_data(
                &session_id,
                &server_public_key,
                &server_identity,
                &session_nonce,
            )?;
            let signature = Signature::try_from(signature.as_slice())
                .map_err(|e| NetworkError::security(format!("invalid signature: {:?}", e)))?;

            server_ed25519_public
                .verify(&signature_data, &signature)
                .map_err(|e| {
                    NetworkError::security(format!("signature verification failed: {}", e))
                })?;

            // Derive shared secret
            let server_ephemeral_public = X25519PublicKey::from(
                <[u8; 32]>::try_from(server_public_key.as_slice())
                    .map_err(|_| NetworkError::security("invalid server public key length"))?,
            );

            let ephemeral_secret = session
                .ephemeral_secret
                .take()
                .ok_or_else(|| NetworkError::security("no ephemeral secret"))?;

            let shared_secret = ephemeral_secret.diffie_hellman(&server_ephemeral_public);
            session.shared_secret = Some(*shared_secret.as_bytes());
            session.remote_ephemeral_public = Some(server_ephemeral_public);
            session.state = KeyExchangeState::Completed;
            session.update_activity();

            // Create confirmation message
            let confirmation_hash =
                self.create_confirmation_hash(&session_id, shared_secret.as_bytes())?;
            let confirm_signature_data =
                self.create_confirm_signature_data(&session_id, &confirmation_hash)?;
            let confirm_signature = self
                .identity_keypair
                .sign(&confirm_signature_data)
                .to_bytes()
                .to_vec();

            info!(
                "Completed key exchange with server (session: {})",
                session_id
            );

            Ok(KeyExchangeMessage::Confirm {
                session_id,
                confirmation_hash,
                signature: confirm_signature,
            })
        } else {
            Err(NetworkError::security("expected response message"))
        }
    }

    /// Handle key exchange confirmation message
    pub async fn handle_confirm(&self, message: KeyExchangeMessage) -> NetworkResult<()> {
        if let KeyExchangeMessage::Confirm {
            session_id,
            confirmation_hash,
            signature,
        } = message
        {
            let mut sessions = self.sessions.write().await;
            let session = sessions
                .get_mut(&session_id)
                .ok_or_else(|| NetworkError::security("unknown session"))?;

            if session.state != KeyExchangeState::HandshakeResponse {
                return Err(NetworkError::security("invalid session state for confirm"));
            }

            // Get client identity for verification
            let identities = self.trusted_identities.read().await;
            let client_identity = identities
                .get(&session.peer_id)
                .ok_or_else(|| NetworkError::security("unknown client identity"))?;

            // Verify signature
            let signature_data =
                self.create_confirm_signature_data(&session_id, &confirmation_hash)?;
            let signature = Signature::try_from(signature.as_slice())
                .map_err(|e| NetworkError::security(format!("invalid signature: {:?}", e)))?;

            client_identity
                .verify(&signature_data, &signature)
                .map_err(|e| {
                    NetworkError::security(format!("signature verification failed: {}", e))
                })?;

            // Verify confirmation hash
            let shared_secret = session
                .shared_secret
                .ok_or_else(|| NetworkError::security("no shared secret"))?;
            let expected_hash = self.create_confirmation_hash(&session_id, &shared_secret)?;

            if confirmation_hash != expected_hash {
                return Err(NetworkError::security("confirmation hash mismatch"));
            }

            session.state = KeyExchangeState::Completed;
            session.update_activity();

            info!(
                "Key exchange confirmed and completed (session: {})",
                session_id
            );
            Ok(())
        } else {
            Err(NetworkError::security("expected confirm message"))
        }
    }

    /// Get completed session key material
    pub async fn get_session_key(&self, session_id: Uuid) -> NetworkResult<[u8; 32]> {
        let sessions = self.sessions.read().await;
        let session = sessions
            .get(&session_id)
            .ok_or_else(|| NetworkError::security("unknown session"))?;

        if session.state != KeyExchangeState::Completed {
            return Err(NetworkError::security("session not completed"));
        }

        session
            .shared_secret
            .ok_or_else(|| NetworkError::security("no shared secret available"))
    }

    /// Lookup the peer identifier associated with a session id.
    pub async fn peer_for_session(&self, session_id: Uuid) -> Option<u8> {
        let sessions = self.sessions.read().await;
        sessions.get(&session_id).map(|session| session.peer_id)
    }

    /// Get session for peer
    pub async fn get_session_for_peer(&self, peer_id: u8) -> Option<Uuid> {
        let sessions = self.sessions.read().await;
        for (session_id, session) in sessions.iter() {
            if session.peer_id == peer_id && session.state == KeyExchangeState::Completed {
                return Some(*session_id);
            }
        }
        None
    }

    /// Clean up expired sessions
    pub async fn cleanup_expired_sessions(&self) -> usize {
        let mut sessions = self.sessions.write().await;
        let initial_count = sessions.len();

        sessions.retain(|_, session| !session.is_expired(self.config.session_timeout_seconds));

        let removed = initial_count - sessions.len();
        if removed > 0 {
            info!("Cleaned up {} expired key exchange sessions", removed);
        }
        removed
    }

    /// Create signature data for initiate message
    fn create_initiate_signature_data(
        &self,
        session_id: &Uuid,
        client_public_key: &[u8],
        client_identity: &[u8],
        session_nonce: &[u8; 16],
    ) -> NetworkResult<Vec<u8>> {
        let mut data = Vec::new();
        data.extend_from_slice(b"INITIATE");
        data.extend_from_slice(session_id.as_bytes());
        data.extend_from_slice(client_public_key);
        data.extend_from_slice(client_identity);
        data.extend_from_slice(session_nonce);
        Ok(data)
    }

    /// Create signature data for response message
    fn create_response_signature_data(
        &self,
        session_id: &Uuid,
        server_public_key: &[u8],
        server_identity: &[u8],
        session_nonce: &[u8; 16],
    ) -> NetworkResult<Vec<u8>> {
        let mut data = Vec::new();
        data.extend_from_slice(b"RESPONSE");
        data.extend_from_slice(session_id.as_bytes());
        data.extend_from_slice(server_public_key);
        data.extend_from_slice(server_identity);
        data.extend_from_slice(session_nonce);
        Ok(data)
    }

    /// Create signature data for confirm message
    fn create_confirm_signature_data(
        &self,
        session_id: &Uuid,
        confirmation_hash: &[u8],
    ) -> NetworkResult<Vec<u8>> {
        let mut data = Vec::new();
        data.extend_from_slice(b"CONFIRM");
        data.extend_from_slice(session_id.as_bytes());
        data.extend_from_slice(confirmation_hash);
        Ok(data)
    }

    /// Create confirmation hash
    fn create_confirmation_hash(
        &self,
        session_id: &Uuid,
        shared_secret: &[u8],
    ) -> NetworkResult<Vec<u8>> {
        let mut data = Vec::new();
        data.extend_from_slice(session_id.as_bytes());
        data.extend_from_slice(shared_secret);
        data.extend_from_slice(b"KEY_EXCHANGE_COMPLETE");

        let hash = digest::digest(&digest::SHA256, &data);
        Ok(hash.as_ref().to_vec())
    }

    /// Get key exchange statistics
    pub async fn get_stats(&self) -> KeyExchangeStats {
        let sessions = self.sessions.read().await;
        let identities = self.trusted_identities.read().await;

        let mut state_counts = HashMap::new();
        for session in sessions.values() {
            *state_counts.entry(session.state).or_insert(0) += 1;
        }

        KeyExchangeStats {
            algorithm: self.config.algorithm,
            total_sessions: sessions.len(),
            trusted_identities: identities.len(),
            state_counts,
        }
    }
}

/// Key exchange statistics
#[derive(Debug, Clone)]
pub struct KeyExchangeStats {
    /// Current algorithm
    pub algorithm: KeyExchangeAlgorithm,
    /// Total active sessions
    pub total_sessions: usize,
    /// Number of trusted identities
    pub trusted_identities: usize,
    /// Session state counts
    pub state_counts: HashMap<KeyExchangeState, usize>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_key_exchange_provider_creation() {
        let provider = KeyExchangeProvider::new().unwrap();
        let stats = provider.get_stats().await;
        assert_eq!(stats.algorithm, KeyExchangeAlgorithm::X25519Ed25519);
        assert_eq!(stats.total_sessions, 0);
    }

    #[tokio::test]
    async fn test_complete_key_exchange() {
        // Setup client and server
        let client_provider = KeyExchangeProvider::new().unwrap();
        let server_provider = KeyExchangeProvider::new().unwrap();

        // Exchange identity keys
        let client_identity = client_provider.get_identity_public_key();
        let server_identity = server_provider.get_identity_public_key();

        client_provider
            .add_trusted_identity(1, server_identity)
            .await;
        server_provider
            .add_trusted_identity(0, client_identity)
            .await;

        // Step 1: Client initiates
        let initiate_msg = client_provider.initiate_key_exchange(1).await.unwrap();

        // Step 2: Server responds
        let response_msg = server_provider
            .handle_initiate(initiate_msg, 0)
            .await
            .unwrap();

        // Step 3: Client confirms
        let confirm_msg = client_provider.handle_response(response_msg).await.unwrap();

        // Step 4: Server handles confirmation
        server_provider.handle_confirm(confirm_msg).await.unwrap();

        // Both should have completed sessions
        let client_stats = client_provider.get_stats().await;
        let server_stats = server_provider.get_stats().await;

        assert!(
            client_stats
                .state_counts
                .get(&KeyExchangeState::Completed)
                .unwrap_or(&0)
                > &0
        );
        assert!(
            server_stats
                .state_counts
                .get(&KeyExchangeState::Completed)
                .unwrap_or(&0)
                > &0
        );
    }

    #[test]
    fn test_key_exchange_session() {
        let session_id = Uuid::new_v4();
        let session = KeyExchangeSession::new(session_id, 1).unwrap();

        assert_eq!(session.session_id, session_id);
        assert_eq!(session.peer_id, 1);
        assert_eq!(session.state, KeyExchangeState::Initialized);
        assert!(!session.is_expired(300)); // 5 minutes
    }

    #[tokio::test]
    async fn test_session_cleanup() {
        let mut config = KeyExchangeConfig::default();
        config.session_timeout_seconds = 1; // 1 second for testing

        let provider = KeyExchangeProvider::with_config(config).unwrap();

        // Create a session
        let _msg = provider.initiate_key_exchange(1).await.unwrap();

        let initial_stats = provider.get_stats().await;
        assert_eq!(initial_stats.total_sessions, 1);

        // Wait for expiration
        tokio::time::sleep(std::time::Duration::from_secs(2)).await;

        // Clean up
        let removed = provider.cleanup_expired_sessions().await;
        assert_eq!(removed, 1);

        let final_stats = provider.get_stats().await;
        assert_eq!(final_stats.total_sessions, 0);
    }
}
