//! Security and anti-cheat system for multiplayer gaming
//!
//! This module provides comprehensive security features including authentication,
//! encryption, command validation, and anti-cheat detection.

use crate::commands::{NetCommand, NetCommandType};
use crate::error::{NetworkError, NetworkResult};
use crate::security::encryption::EncryptionProvider;
use crate::security::key_exchange::{
    KeyExchangeConfig, KeyExchangeMessage, KeyExchangeProvider, KeyExchangeStats,
};
use chrono::{DateTime, Utc};
use ed25519_dalek::VerifyingKey as Ed25519VerifyingKey;
use ring::{constant_time, digest, hmac, rand, signature};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::convert::TryFrom;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, error, info, warn};
use uuid::Uuid;

pub mod anti_cheat;
pub mod auth;
pub mod encryption;
pub mod firewall;
pub mod key_exchange;
pub mod validation;
pub mod windows_firewall;

/// Security configuration
#[derive(Debug, Clone)]
pub struct SecurityConfig {
    /// Enable command signing
    pub enable_command_signing: bool,
    /// Enable packet encryption
    pub enable_encryption: bool,
    /// Enable anti-cheat detection
    pub enable_anti_cheat: bool,
    /// Maximum command frequency per second
    pub max_commands_per_second: u32,
    /// Session timeout duration
    pub session_timeout_minutes: u32,
    /// Enable rate limiting
    pub enable_rate_limiting: bool,
    /// Maximum failed authentication attempts
    pub max_auth_failures: u32,
    /// Key exchange configuration
    pub key_exchange: KeyExchangeConfig,
}

impl Default for SecurityConfig {
    fn default() -> Self {
        Self {
            enable_command_signing: true,
            enable_encryption: true,
            enable_anti_cheat: true,
            max_commands_per_second: 30,
            session_timeout_minutes: 60,
            enable_rate_limiting: true,
            max_auth_failures: 5,
            key_exchange: KeyExchangeConfig::default(),
        }
    }
}

/// Player authentication information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlayerAuth {
    /// Player unique identifier
    pub player_id: u8,
    /// Authentication token
    pub auth_token: String,
    /// Session ID
    pub session_id: Uuid,
    /// Player name/username
    pub username: String,
    /// Authentication timestamp
    pub authenticated_at: DateTime<Utc>,
    /// Last activity timestamp
    pub last_activity: DateTime<Utc>,
    /// Authentication level/permissions
    pub auth_level: AuthLevel,
    /// Public key for signature verification
    pub public_key: Vec<u8>,
    /// Negotiated secure transport session, if any
    #[serde(skip_serializing_if = "Option::is_none")]
    pub secure_session: Option<SecureSession>,
}

/// Established secure session metadata for encrypted transport
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecureSession {
    /// Key exchange session identifier
    pub session_id: Uuid,
    /// Shared encryption key derived via key exchange
    pub shared_key: Option<[u8; 32]>,
    /// Timestamp when the session was established
    pub established_at: Option<DateTime<Utc>>,
}

/// Authentication levels
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[repr(u8)]
pub enum AuthLevel {
    /// Guest/unauthenticated user
    Guest = 0,
    /// Regular authenticated player
    Player = 1,
    /// Moderator with additional privileges
    Moderator = 2,
    /// Administrator with full privileges
    Administrator = 3,
}

/// Security violation types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SecurityViolation {
    /// Invalid command signature
    InvalidSignature {
        player_id: u8,
        command_type: NetCommandType,
        expected_hash: String,
        actual_hash: String,
    },
    /// Rate limiting violation
    RateLimitExceeded {
        player_id: u8,
        commands_per_second: u32,
        limit: u32,
    },
    /// Impossible command timing
    ImpossibleTiming {
        player_id: u8,
        command_type: NetCommandType,
        time_delta_ms: u64,
    },
    /// Command validation failure
    ValidationFailure {
        player_id: u8,
        command_type: NetCommandType,
        reason: String,
    },
    /// Suspicious behavior pattern
    SuspiciousBehavior {
        player_id: u8,
        pattern: String,
        confidence: f64,
    },
    /// Authentication failure
    AuthenticationFailure {
        player_id: u8,
        reason: String,
        attempts: u32,
    },
    /// Encryption/decryption failure
    CryptographicFailure { player_id: u8, operation: String },
}

/// Security event for logging and analysis
#[derive(Debug, Clone)]
pub struct SecurityEvent {
    /// Event unique identifier
    pub event_id: Uuid,
    /// Violation details
    pub violation: SecurityViolation,
    /// Event timestamp
    pub timestamp: DateTime<Utc>,
    /// Severity level
    pub severity: SecuritySeverity,
    /// Whether action was taken
    pub action_taken: Option<SecurityAction>,
}

/// Security severity levels
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[repr(u8)]
pub enum SecuritySeverity {
    /// Low severity - informational
    Info = 0,
    /// Medium severity - warning
    Warning = 1,
    /// High severity - likely violation
    High = 2,
    /// Critical severity - definite violation
    Critical = 3,
}

/// Security actions that can be taken
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SecurityAction {
    /// Log the event only
    LogOnly,
    /// Issue warning to player
    Warning,
    /// Temporarily restrict player commands
    Restrict { duration_minutes: u32 },
    /// Kick player from game
    Kick,
    /// Ban player temporarily
    TempBan { duration_hours: u32 },
    /// Permanently ban player
    PermBan,
}

/// Command rate tracking
#[derive(Debug, Clone)]
struct CommandRateTracker {
    commands_in_window: Vec<DateTime<Utc>>,
    window_duration_seconds: u64,
}

impl CommandRateTracker {
    fn new(window_duration_seconds: u64) -> Self {
        Self {
            commands_in_window: Vec::new(),
            window_duration_seconds,
        }
    }

    fn add_command(&mut self, timestamp: DateTime<Utc>) -> u32 {
        self.commands_in_window.push(timestamp);

        // Remove old commands outside window
        let cutoff = timestamp - chrono::Duration::seconds(self.window_duration_seconds as i64);
        self.commands_in_window
            .retain(|&cmd_time| cmd_time >= cutoff);

        self.commands_in_window.len() as u32
    }

    #[cfg(test)]
    fn get_rate(&self) -> u32 {
        self.commands_in_window.len() as u32
    }
}

/// Main security manager
pub struct SecurityManager {
    /// Configuration
    config: SecurityConfig,

    /// Player authentication data
    player_auth: Arc<RwLock<HashMap<u8, PlayerAuth>>>,

    /// Command rate trackers per player
    rate_trackers: Arc<RwLock<HashMap<u8, CommandRateTracker>>>,

    /// Security events log
    security_events: Arc<RwLock<Vec<SecurityEvent>>>,

    /// Banned players
    banned_players: Arc<RwLock<HashSet<u8>>>,

    /// Failed authentication attempts
    auth_failures: Arc<RwLock<HashMap<u8, u32>>>,

    /// Cryptographic keys
    signing_key: signature::Ed25519KeyPair,
    hmac_key: hmac::Key,

    /// Key exchange provider
    key_exchange: Arc<KeyExchangeProvider>,

    /// Symmetric encryption provider used for packet confidentiality
    encryption: Arc<EncryptionProvider>,

    /// Anti-cheat detector
    anti_cheat: Arc<AntiCheatDetector>,
}

impl SecurityManager {
    /// Create new security manager
    pub fn new() -> NetworkResult<Self> {
        Self::with_config(SecurityConfig::default())
    }

    /// Create security manager with configuration
    pub fn with_config(config: SecurityConfig) -> NetworkResult<Self> {
        // Generate cryptographic keys
        let rng = rand::SystemRandom::new();
        let signing_key = signature::Ed25519KeyPair::generate_pkcs8(&rng).map_err(|e| {
            NetworkError::security(format!("failed to generate signing key: {:?}", e))
        })?;
        let signing_key = signature::Ed25519KeyPair::from_pkcs8(signing_key.as_ref())
            .map_err(|e| NetworkError::security(format!("failed to parse signing key: {:?}", e)))?;

        let hmac_key = hmac::Key::generate(hmac::HMAC_SHA256, &rng)
            .map_err(|e| NetworkError::security(format!("failed to generate HMAC key: {:?}", e)))?;

        let key_exchange = Arc::new(KeyExchangeProvider::with_config(
            config.key_exchange.clone(),
        )?);
        let encryption = Arc::new(EncryptionProvider::new()?);

        Ok(Self {
            config,
            player_auth: Arc::new(RwLock::new(HashMap::new())),
            rate_trackers: Arc::new(RwLock::new(HashMap::new())),
            security_events: Arc::new(RwLock::new(Vec::new())),
            banned_players: Arc::new(RwLock::new(HashSet::new())),
            auth_failures: Arc::new(RwLock::new(HashMap::new())),
            signing_key,
            hmac_key,
            key_exchange,
            encryption,
            anti_cheat: Arc::new(AntiCheatDetector::new()),
        })
    }

    /// Authenticate a player
    pub async fn authenticate_player<S, T>(
        &self,
        player_id: u8,
        username: S,
        auth_token: T,
        public_key: Vec<u8>,
    ) -> NetworkResult<PlayerAuth>
    where
        S: Into<String>,
        T: Into<String>,
    {
        let username: String = username.into();
        let auth_token: String = auth_token.into();
        // Check if player is banned
        {
            let banned = self.banned_players.read().await;
            if banned.contains(&player_id) {
                return Err(NetworkError::security(format!(
                    "player {} is banned",
                    player_id
                )));
            }
        }

        // Validate authentication token (in real implementation, this would verify with auth server)
        if !self.validate_auth_token(&username, &auth_token).await? {
            // Record authentication failure
            {
                let mut failures = self.auth_failures.write().await;
                let count = failures.entry(player_id).or_insert(0);
                *count += 1;

                if *count >= self.config.max_auth_failures {
                    // Too many failures, ban player temporarily
                    let mut banned = self.banned_players.write().await;
                    banned.insert(player_id);

                    self.log_security_event(
                        SecurityViolation::AuthenticationFailure {
                            player_id,
                            reason: "too many failed attempts".to_string(),
                            attempts: *count,
                        },
                        SecuritySeverity::High,
                        Some(SecurityAction::TempBan { duration_hours: 1 }),
                    )
                    .await;
                }
            }

            return Err(NetworkError::security("invalid authentication token"));
        }

        // Clear any previous authentication failures
        {
            let mut failures = self.auth_failures.write().await;
            failures.remove(&player_id);
        }

        // Create player authentication record
        let player_auth = PlayerAuth {
            player_id,
            auth_token,
            session_id: Uuid::new_v4(),
            username,
            authenticated_at: Utc::now(),
            last_activity: Utc::now(),
            auth_level: AuthLevel::Player, // Default level
            public_key,
            secure_session: None,
        };

        // Store authentication
        {
            let mut auth_map = self.player_auth.write().await;
            auth_map.insert(player_id, player_auth.clone());
        }

        if let Ok(bytes) = <[u8; 32]>::try_from(player_auth.public_key.as_slice()) {
            if let Ok(public_key) = Ed25519VerifyingKey::from_bytes(&bytes) {
                self.key_exchange
                    .add_trusted_identity(player_id, public_key)
                    .await;
            } else {
                warn!("Failed to parse public key for player {}", player_id);
            }
        } else {
            warn!(
                "Invalid public key length for player {}: {} bytes",
                player_id,
                player_auth.public_key.len()
            );
        }

        info!(
            "Authenticated player {}: {}",
            player_id, player_auth.username
        );
        Ok(player_auth)
    }

    /// Deauthenticate and clean up state for a player.
    pub async fn deauthenticate_player(&self, player_id: u8) {
        self.player_auth.write().await.remove(&player_id);
        self.rate_trackers.write().await.remove(&player_id);
        let _ = self.key_exchange.remove_trusted_identity(player_id).await;
    }

    async fn record_secure_session(
        &self,
        player_id: u8,
        session_id: Uuid,
        shared_key: Option<[u8; 32]>,
    ) -> NetworkResult<()> {
        let mut auth_map = self.player_auth.write().await;
        let auth = auth_map
            .get_mut(&player_id)
            .ok_or_else(|| NetworkError::security("player not authenticated"))?;

        auth.secure_session = Some(SecureSession {
            session_id,
            shared_key,
            established_at: shared_key.map(|_| Utc::now()),
        });
        Ok(())
    }

    /// Validate authentication token
    async fn validate_auth_token(&self, username: &str, token: &str) -> NetworkResult<bool> {
        if username.is_empty() || token.is_empty() {
            return Ok(false);
        }

        let expected = self.generate_auth_token(username);
        Ok(constant_time::verify_slices_are_equal(expected.as_bytes(), token.as_bytes()).is_ok())
    }

    /// Generate an authentication token for the given username.
    pub fn generate_auth_token(&self, username: &str) -> String {
        let tag = hmac::sign(&self.hmac_key, username.as_bytes());
        hex::encode(tag.as_ref())
    }

    /// Retrieve the public identity key used for secure handshakes and signatures.
    pub fn identity_public_key(&self) -> Ed25519VerifyingKey {
        self.key_exchange.get_identity_public_key()
    }

    /// Begin a key exchange with a peer.
    pub async fn initiate_key_exchange(&self, peer_id: u8) -> NetworkResult<KeyExchangeMessage> {
        let message = self.key_exchange.initiate_key_exchange(peer_id).await?;

        if let KeyExchangeMessage::Initiate { session_id, .. } = &message {
            self.record_secure_session(peer_id, *session_id, None)
                .await?;
        }

        Ok(message)
    }

    /// Handle a key exchange initiation from a peer.
    pub async fn handle_key_exchange_initiate(
        &self,
        message: KeyExchangeMessage,
        peer_id: u8,
    ) -> NetworkResult<KeyExchangeMessage> {
        let response = self.key_exchange.handle_initiate(message, peer_id).await?;

        if let KeyExchangeMessage::Response { session_id, .. } = &response {
            self.record_secure_session(peer_id, *session_id, None)
                .await?;
        }

        Ok(response)
    }

    /// Handle a key exchange response from the remote peer.
    pub async fn handle_key_exchange_response(
        &self,
        message: KeyExchangeMessage,
    ) -> NetworkResult<KeyExchangeMessage> {
        let confirm = self.key_exchange.handle_response(message).await?;

        if let KeyExchangeMessage::Confirm { session_id, .. } = &confirm {
            if let Some(peer_id) = self.key_exchange.peer_for_session(*session_id).await {
                let shared_key = self.key_exchange.get_session_key(*session_id).await?;
                self.record_secure_session(peer_id, *session_id, Some(shared_key))
                    .await?;
            }
        }

        Ok(confirm)
    }

    /// Confirm a completed key exchange.
    pub async fn confirm_key_exchange(&self, message: KeyExchangeMessage) -> NetworkResult<()> {
        let session_id = if let KeyExchangeMessage::Confirm { session_id, .. } = &message {
            Some(*session_id)
        } else {
            None
        };

        self.key_exchange.handle_confirm(message).await?;

        if let Some(session_id) = session_id {
            if let Some(peer_id) = self.key_exchange.peer_for_session(session_id).await {
                let shared_key = self.key_exchange.get_session_key(session_id).await?;
                self.record_secure_session(peer_id, session_id, Some(shared_key))
                    .await?;
            }
        }

        Ok(())
    }

    /// Retrieve the derived session key for a completed exchange.
    pub async fn key_exchange_session_key(&self, session_id: Uuid) -> NetworkResult<[u8; 32]> {
        self.key_exchange.get_session_key(session_id).await
    }

    /// Fetch the active secure session key for a player, if established.
    pub async fn secure_session_key(&self, player_id: u8) -> NetworkResult<[u8; 32]> {
        let auth_map = self.player_auth.read().await;
        let auth = auth_map
            .get(&player_id)
            .ok_or_else(|| NetworkError::security("player not authenticated"))?;

        auth.secure_session
            .as_ref()
            .and_then(|sess| sess.shared_key)
            .ok_or_else(|| NetworkError::security("secure session not established"))
    }

    /// Statistics about ongoing key exchanges.
    pub async fn key_exchange_stats(&self) -> KeyExchangeStats {
        self.key_exchange.get_stats().await
    }

    /// Access the shared encryption provider used for packet confidentiality.
    pub fn encryption_provider(&self) -> Arc<EncryptionProvider> {
        Arc::clone(&self.encryption)
    }

    /// Validate and process a command
    pub async fn validate_command(&self, command: &NetCommand) -> NetworkResult<()> {
        // Check if player is authenticated
        let player_auth = {
            let auth_map = self.player_auth.read().await;
            auth_map.get(&command.player_id).cloned()
        };

        let player_auth = player_auth.ok_or_else(|| {
            NetworkError::security(format!("player {} not authenticated", command.player_id))
        })?;

        // Check rate limiting
        if self.config.enable_rate_limiting {
            self.check_rate_limit(command).await?;
        }

        // Verify command signature
        if self.config.enable_command_signing {
            self.verify_command_signature(command, &player_auth).await?;
        }

        // Anti-cheat validation
        if self.config.enable_anti_cheat {
            self.anti_cheat.validate_command(command).await?;
        }

        // Update last activity
        {
            let mut auth_map = self.player_auth.write().await;
            if let Some(auth) = auth_map.get_mut(&command.player_id) {
                auth.last_activity = Utc::now();
            }
        }

        Ok(())
    }

    /// Check command rate limiting
    async fn check_rate_limit(&self, command: &NetCommand) -> NetworkResult<()> {
        let mut trackers = self.rate_trackers.write().await;
        let tracker = trackers.entry(command.player_id).or_insert_with(|| {
            CommandRateTracker::new(1) // 1 second window
        });

        let current_rate = tracker.add_command(command.timestamp);

        if current_rate > self.config.max_commands_per_second {
            self.log_security_event(
                SecurityViolation::RateLimitExceeded {
                    player_id: command.player_id,
                    commands_per_second: current_rate,
                    limit: self.config.max_commands_per_second,
                },
                SecuritySeverity::Warning,
                Some(SecurityAction::Warning),
            )
            .await;

            return Err(NetworkError::rate_limited(format!(
                "rate limit exceeded: {} commands/sec (max: {})",
                current_rate, self.config.max_commands_per_second
            )));
        }

        Ok(())
    }

    /// Verify command signature
    async fn verify_command_signature(
        &self,
        command: &NetCommand,
        player_auth: &PlayerAuth,
    ) -> NetworkResult<()> {
        if let Some(signature_bytes) = &command.signature {
            // Create command hash for signature verification
            let command_data = self.create_command_hash(command)?;

            // Verify signature using player's public key
            let public_key =
                signature::UnparsedPublicKey::new(&signature::ED25519, &player_auth.public_key);

            match public_key.verify(&command_data, signature_bytes) {
                Ok(()) => Ok(()),
                Err(_) => {
                    self.log_security_event(
                        SecurityViolation::InvalidSignature {
                            player_id: command.player_id,
                            command_type: command.command_type,
                            expected_hash: hex::encode(&command_data),
                            actual_hash: hex::encode(signature_bytes),
                        },
                        SecuritySeverity::High,
                        Some(SecurityAction::Restrict {
                            duration_minutes: 5,
                        }),
                    )
                    .await;

                    Err(NetworkError::security("invalid command signature"))
                }
            }
        } else {
            Err(NetworkError::security("missing command signature"))
        }
    }

    /// Create deterministic hash of command for signature verification
    fn create_command_hash(&self, command: &NetCommand) -> NetworkResult<Vec<u8>> {
        // Create canonical representation of command
        let mut data = Vec::new();

        // Add command fields in deterministic order
        data.extend_from_slice(&(command.command_type as u8).to_le_bytes());
        data.extend_from_slice(&command.player_id.to_le_bytes());
        data.extend_from_slice(&command.execution_frame.to_le_bytes());
        data.extend_from_slice(&command.sequence.to_le_bytes());
        data.extend_from_slice(command.id.as_bytes());

        // Add payload hash
        let payload_data = bincode::serialize(&command.payload)
            .map_err(|e| NetworkError::security(format!("failed to serialize payload: {}", e)))?;
        data.extend_from_slice(&payload_data);

        // Calculate SHA-256 hash
        let hash = digest::digest(&digest::SHA256, &data);
        Ok(hash.as_ref().to_vec())
    }

    /// Sign a command with the server's private key
    pub fn sign_command(&self, command: &mut NetCommand) -> NetworkResult<()> {
        let command_data = self.create_command_hash(command)?;
        let signature = self.signing_key.sign(&command_data);
        command.signature = Some(signature.as_ref().to_vec());
        Ok(())
    }

    /// Log security event
    async fn log_security_event(
        &self,
        violation: SecurityViolation,
        severity: SecuritySeverity,
        action: Option<SecurityAction>,
    ) {
        let event = SecurityEvent {
            event_id: Uuid::new_v4(),
            violation: violation.clone(),
            timestamp: Utc::now(),
            severity,
            action_taken: action.clone(),
        };

        // Log based on severity
        match severity {
            SecuritySeverity::Info => debug!("Security event: {:?}", violation),
            SecuritySeverity::Warning => warn!("Security violation: {:?}", violation),
            SecuritySeverity::High => error!("High severity security violation: {:?}", violation),
            SecuritySeverity::Critical => error!("CRITICAL security violation: {:?}", violation),
        }

        // Store event
        {
            let mut events = self.security_events.write().await;
            events.push(event);

            // Limit event history
            if events.len() > 10000 {
                events.drain(0..1000); // Remove oldest 1000 events
            }
        }

        // Take action if specified
        if let Some(action) = action {
            self.take_security_action(violation, action).await;
        }
    }

    /// Take security action
    async fn take_security_action(&self, violation: SecurityViolation, action: SecurityAction) {
        let player_id = self.get_player_id_from_violation(&violation);

        match action {
            SecurityAction::LogOnly => {
                // Already logged above
            }
            SecurityAction::Warning => {
                info!("Issued warning to player {}", player_id);
                // In real implementation, would send warning message to player
            }
            SecurityAction::Restrict { duration_minutes } => {
                info!(
                    "Restricted player {} for {} minutes",
                    player_id, duration_minutes
                );
                // In real implementation, would add restriction to player record
            }
            SecurityAction::Kick => {
                info!("Kicking player {}", player_id);
                // In real implementation, would initiate disconnect sequence
            }
            SecurityAction::TempBan { duration_hours } => {
                info!(
                    "Temporarily banned player {} for {} hours",
                    player_id, duration_hours
                );
                let mut banned = self.banned_players.write().await;
                banned.insert(player_id);
                // In real implementation, would also set expiration time
            }
            SecurityAction::PermBan => {
                info!("Permanently banned player {}", player_id);
                let mut banned = self.banned_players.write().await;
                banned.insert(player_id);
            }
        }
    }

    /// Extract player ID from security violation
    fn get_player_id_from_violation(&self, violation: &SecurityViolation) -> u8 {
        match violation {
            SecurityViolation::InvalidSignature { player_id, .. } => *player_id,
            SecurityViolation::RateLimitExceeded { player_id, .. } => *player_id,
            SecurityViolation::ImpossibleTiming { player_id, .. } => *player_id,
            SecurityViolation::ValidationFailure { player_id, .. } => *player_id,
            SecurityViolation::SuspiciousBehavior { player_id, .. } => *player_id,
            SecurityViolation::AuthenticationFailure { player_id, .. } => *player_id,
            SecurityViolation::CryptographicFailure { player_id, .. } => *player_id,
        }
    }

    /// Check if player is authenticated
    pub async fn is_player_authenticated(&self, player_id: u8) -> bool {
        let auth_map = self.player_auth.read().await;
        auth_map.contains_key(&player_id)
    }

    /// Get player authentication info
    pub async fn get_player_auth(&self, player_id: u8) -> Option<PlayerAuth> {
        let auth_map = self.player_auth.read().await;
        auth_map.get(&player_id).cloned()
    }

    /// Remove player authentication
    pub async fn logout_player(&self, player_id: u8) {
        let mut auth_map = self.player_auth.write().await;
        auth_map.remove(&player_id);

        let mut trackers = self.rate_trackers.write().await;
        trackers.remove(&player_id);

        info!("Player {} logged out", player_id);
    }

    /// Get security statistics
    pub async fn get_security_stats(&self) -> SecurityStats {
        let events = self.security_events.read().await;
        let auth_count = self.player_auth.read().await.len();
        let banned_count = self.banned_players.read().await.len();

        let mut event_counts = HashMap::new();
        for event in events.iter() {
            let key = std::mem::discriminant(&event.violation);
            *event_counts.entry(format!("{:?}", key)).or_insert(0) += 1;
        }

        SecurityStats {
            authenticated_players: auth_count,
            banned_players: banned_count,
            total_security_events: events.len(),
            event_counts,
        }
    }

    /// Clean up expired sessions and old events
    pub async fn cleanup(&self) -> NetworkResult<()> {
        let now = Utc::now();
        let session_timeout = chrono::Duration::minutes(self.config.session_timeout_minutes as i64);

        // Remove expired sessions
        {
            let mut auth_map = self.player_auth.write().await;
            auth_map
                .retain(|_, auth| now.signed_duration_since(auth.last_activity) < session_timeout);
        }

        // Clean up old security events
        {
            let mut events = self.security_events.write().await;
            let cutoff = now - chrono::Duration::hours(24); // Keep 24 hours
            events.retain(|event| event.timestamp > cutoff);
        }

        Ok(())
    }
}

/// Security statistics
#[derive(Debug, Clone)]
pub struct SecurityStats {
    pub authenticated_players: usize,
    pub banned_players: usize,
    pub total_security_events: usize,
    pub event_counts: HashMap<String, u32>,
}

/// Anti-cheat detection system
pub struct AntiCheatDetector {
    /// Player behavior patterns
    player_patterns: Arc<RwLock<HashMap<u8, PlayerBehaviorPattern>>>,
}

/// Player behavior pattern tracking
#[derive(Debug, Clone)]
struct PlayerBehaviorPattern {
    command_timings: Vec<DateTime<Utc>>,
    command_types: HashMap<NetCommandType, u32>,
    suspicious_score: f64,
    last_update: DateTime<Utc>,
}

impl AntiCheatDetector {
    fn new() -> Self {
        Self {
            player_patterns: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    async fn validate_command(&self, command: &NetCommand) -> NetworkResult<()> {
        // Update player behavior pattern
        {
            let mut patterns = self.player_patterns.write().await;
            let pattern =
                patterns
                    .entry(command.player_id)
                    .or_insert_with(|| PlayerBehaviorPattern {
                        command_timings: Vec::new(),
                        command_types: HashMap::new(),
                        suspicious_score: 0.0,
                        last_update: Utc::now(),
                    });

            // Track command timing
            pattern.command_timings.push(command.timestamp);
            if pattern.command_timings.len() > 100 {
                pattern.command_timings.remove(0);
            }

            // Track command types
            *pattern
                .command_types
                .entry(command.command_type)
                .or_insert(0) += 1;
            pattern.last_update = Utc::now();

            // Analyze for suspicious patterns
            self.analyze_behavior_pattern(command.player_id, pattern)
                .await?;
        }

        Ok(())
    }

    async fn analyze_behavior_pattern(
        &self,
        player_id: u8,
        pattern: &mut PlayerBehaviorPattern,
    ) -> NetworkResult<()> {
        let mut suspicious_indicators = Vec::new();

        // Check for impossible timing (commands too fast)
        if pattern.command_timings.len() >= 2 {
            let recent_timings: Vec<_> = pattern
                .command_timings
                .iter()
                .rev()
                .take(5)
                .cloned()
                .collect();
            for window in recent_timings.windows(2) {
                let time_diff = window[0]
                    .signed_duration_since(window[1])
                    .num_milliseconds();
                if time_diff < 10 {
                    // Less than 10ms between commands is suspicious
                    suspicious_indicators.push("impossible_timing");
                    break;
                }
            }
        }

        // Check for repetitive patterns
        if let Some(&game_command_count) = pattern.command_types.get(&NetCommandType::GameCommand) {
            let total_commands: u32 = pattern.command_types.values().sum();
            if total_commands > 50 && game_command_count as f64 / total_commands as f64 > 0.95 {
                suspicious_indicators.push("repetitive_commands");
            }
        }

        // Update suspicious score
        let new_indicators = suspicious_indicators.len() as f64 * 0.1;
        pattern.suspicious_score = (pattern.suspicious_score * 0.9) + new_indicators;

        // Threshold for action
        if pattern.suspicious_score > 0.7 {
            return Err(NetworkError::anti_cheat(format!(
                "suspicious behavior detected for player {}: indicators={:?}, score={:.2}",
                player_id, suspicious_indicators, pattern.suspicious_score
            )));
        }

        Ok(())
    }
}

// Add hex dependency for encoding
use hex;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::CommandPayload;

    #[tokio::test]
    async fn test_security_manager_creation() {
        let security_manager = SecurityManager::new().unwrap();

        let stats = security_manager.get_security_stats().await;
        assert_eq!(stats.authenticated_players, 0);
        assert_eq!(stats.banned_players, 0);
        assert_eq!(stats.total_security_events, 0);
    }

    #[tokio::test]
    async fn test_player_authentication() {
        let security_manager = SecurityManager::new().unwrap();

        let token = security_manager.generate_auth_token("test_user");
        let auth_result = security_manager
            .authenticate_player(0, "test_user", token, vec![1; 32])
            .await;

        assert!(auth_result.is_ok());
        let auth = auth_result.unwrap();
        assert_eq!(auth.player_id, 0);
        assert_eq!(auth.username, "test_user");
        assert_eq!(auth.auth_level, AuthLevel::Player);

        // Check that player is now authenticated
        assert!(security_manager.is_player_authenticated(0).await);
    }

    #[tokio::test]
    async fn test_rate_limiting() {
        let config = SecurityConfig {
            max_commands_per_second: 2,
            enable_command_signing: false,
            enable_anti_cheat: false,
            ..Default::default()
        };
        let security_manager = SecurityManager::with_config(config).unwrap();

        // Authenticate player first
        let token = security_manager.generate_auth_token("test_user");
        security_manager
            .authenticate_player(0, "test_user", token, vec![1; 32])
            .await
            .unwrap();

        // First command should pass
        let mut command1 =
            NetCommand::new(NetCommandType::KeepAlive, 0, 0, CommandPayload::KeepAlive);
        security_manager.sign_command(&mut command1).unwrap();
        assert!(security_manager.validate_command(&command1).await.is_ok());

        // Second command should pass
        let mut command2 =
            NetCommand::new(NetCommandType::KeepAlive, 0, 0, CommandPayload::KeepAlive);
        security_manager.sign_command(&mut command2).unwrap();
        assert!(security_manager.validate_command(&command2).await.is_ok());

        // Third command should fail due to rate limiting
        let mut command3 =
            NetCommand::new(NetCommandType::KeepAlive, 0, 0, CommandPayload::KeepAlive);
        security_manager.sign_command(&mut command3).unwrap();
        assert!(security_manager.validate_command(&command3).await.is_err());
    }

    #[tokio::test]
    async fn test_secure_session_handshake() {
        let host_manager = SecurityManager::new().unwrap();
        let client_manager = SecurityManager::new().unwrap();

        let host_identity = host_manager.identity_public_key();
        let client_identity = client_manager.identity_public_key();

        let host_token = host_manager.generate_auth_token("ClientPlayer");
        host_manager
            .authenticate_player(
                1,
                "ClientPlayer",
                host_token,
                client_identity.as_bytes().to_vec(),
            )
            .await
            .unwrap();

        let client_token = client_manager.generate_auth_token("HostPlayer");
        client_manager
            .authenticate_player(
                0,
                "HostPlayer",
                client_token,
                host_identity.as_bytes().to_vec(),
            )
            .await
            .unwrap();

        let initiate = client_manager.initiate_key_exchange(0).await.unwrap();
        let response = host_manager
            .handle_key_exchange_initiate(initiate, 1)
            .await
            .unwrap();
        let confirm = client_manager
            .handle_key_exchange_response(response)
            .await
            .unwrap();
        host_manager
            .confirm_key_exchange(confirm)
            .await
            .expect("host confirm");

        let client_shared = client_manager
            .secure_session_key(0)
            .await
            .unwrap_or_else(|err| panic!("client secure session: {:?}", err));
        let host_shared = host_manager
            .secure_session_key(1)
            .await
            .unwrap_or_else(|err| panic!("host secure session: {:?}", err));

        assert_eq!(client_shared, host_shared);

        let host_sessions = host_manager.player_auth.read().await;
        let session_meta = host_sessions
            .get(&1)
            .and_then(|auth| auth.secure_session.clone())
            .expect("expected secure session metadata for peer");
        assert!(session_meta.established_at.is_some());
    }

    #[test]
    fn test_security_config_default() {
        let config = SecurityConfig::default();
        assert!(config.enable_command_signing);
        assert!(config.enable_encryption);
        assert!(config.enable_anti_cheat);
        assert_eq!(config.max_commands_per_second, 30);
    }

    #[test]
    fn test_command_rate_tracker() {
        let mut tracker = CommandRateTracker::new(1);
        let now = Utc::now();

        // Add commands
        assert_eq!(tracker.add_command(now), 1);
        assert_eq!(tracker.add_command(now), 2);
        assert_eq!(tracker.get_rate(), 2);

        // Add command after window expires
        let later = now + chrono::Duration::seconds(2);
        assert_eq!(tracker.add_command(later), 1);
    }
}
