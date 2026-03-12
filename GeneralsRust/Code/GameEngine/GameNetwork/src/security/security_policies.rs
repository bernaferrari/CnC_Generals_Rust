//! Security policies for different game modes and scenarios
//!
//! This module provides configurable security policies that can be applied
//! based on game mode, network type, and security requirements.

use super::*;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Security policy configuration for different game modes
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityPolicyConfig {
    /// Policy name
    pub name: String,
    /// Policy description
    pub description: String,
    /// Security configuration
    pub security: SecurityConfig,
    /// Encryption configuration
    pub encryption: EncryptionConfig,
    /// Authentication configuration
    pub authentication: AuthConfig,
    /// Anti-cheat configuration
    pub anti_cheat: AntiCheatConfig,
    /// Key exchange configuration
    pub key_exchange: KeyExchangeConfig,
    /// Network security configuration
    pub network_security: NetworkSecurityConfig,
}

/// Predefined security policies
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SecurityPolicy {
    /// Maximum security - competitive/ranked matches
    Competitive,
    /// Balanced security - casual online matches
    CasualOnline,
    /// Minimal security - LAN/trusted network
    LanTrusted,
    /// Development/testing mode
    Development,
    /// Custom policy
    Custom,
}

impl Default for SecurityPolicy {
    fn default() -> Self {
        Self::CasualOnline
    }
}

/// Security policy manager
pub struct SecurityPolicyManager {
    /// Available policies
    policies: HashMap<SecurityPolicy, SecurityPolicyConfig>,
    /// Current active policy
    current_policy: SecurityPolicy,
}

impl SecurityPolicyManager {
    /// Create new security policy manager
    pub fn new() -> Self {
        let mut policies = HashMap::new();
        
        // Initialize predefined policies
        policies.insert(SecurityPolicy::Competitive, Self::create_competitive_policy());
        policies.insert(SecurityPolicy::CasualOnline, Self::create_casual_online_policy());
        policies.insert(SecurityPolicy::LanTrusted, Self::create_lan_trusted_policy());
        policies.insert(SecurityPolicy::Development, Self::create_development_policy());
        
        Self {
            policies,
            current_policy: SecurityPolicy::CasualOnline,
        }
    }

    /// Create competitive/ranked match security policy
    fn create_competitive_policy() -> SecurityPolicyConfig {
        SecurityPolicyConfig {
            name: "Competitive".to_string(),
            description: "Maximum security for competitive/ranked matches".to_string(),
            security: SecurityConfig {
                enable_command_signing: true,
                enable_encryption: true,
                enable_anti_cheat: true,
                max_commands_per_second: 25, // Stricter limit
                session_timeout_minutes: 30, // Shorter timeout
                enable_rate_limiting: true,
                max_auth_failures: 3, // Stricter auth
            },
            encryption: EncryptionConfig {
                algorithm: EncryptionAlgorithm::ChaCha20Poly1305,
                enable_integrity_check: true,
                key_rotation_interval: 1800, // 30 minutes
                max_packet_size: 32768, // Smaller packets
                enable_compression: false,
            },
            authentication: AuthConfig {
                method: AuthMethod::Ed25519Signature, // Stronger auth
                token_expiry_seconds: 1800, // 30 minutes
                allow_guest_access: false, // No guests in competitive
                max_auth_attempts: 3,
                auth_timeout_seconds: 15,
                refresh_token_expiry_seconds: 86400, // 1 day
                ..Default::default()
            },
            anti_cheat: AntiCheatConfig {
                enabled: true,
                methods: vec![
                    AntiCheatMethod::StatisticalAnalysis,
                    AntiCheatMethod::TimingAnalysis,
                    AntiCheatMethod::StateConsistency,
                    AntiCheatMethod::PatternRecognition,
                    AntiCheatMethod::MachineLearning,
                ],
                sensitivity: 0.9, // High sensitivity
                action_threshold: 0.7, // Lower threshold for action
                history_size: 2000,
                min_commands_for_analysis: 30, // Faster analysis
                analysis_window_seconds: 30,
                max_commands_per_second: 25.0,
                min_command_interval_ms: 5,
                learning_mode: false,
            },
            key_exchange: KeyExchangeConfig {
                algorithm: KeyExchangeAlgorithm::X25519Ed25519,
                session_timeout_seconds: 60, // 1 minute
                enable_pfs: true,
                max_concurrent_sessions: 50,
                pre_shared_key: None,
            },
            network_security: NetworkSecurityConfig {
                enable_rate_limiting: true,
                enable_ddos_protection: true,
                enable_ip_access_control: true,
                rate_limiting: RateLimitConfig {
                    requests_per_second_per_ip: 50,
                    requests_per_second_per_player: 25,
                    burst_capacity_per_ip: 75,
                    burst_capacity_per_player: 40,
                    window_duration_seconds: 60,
                    violation_threshold: 3,
                    block_duration_seconds: 600, // 10 minutes
                },
                ddos_protection: DDoSProtectionConfig {
                    max_connections_per_ip: 5, // Very restrictive
                    connection_rate_threshold: 15,
                    packet_rate_threshold: 500,
                    bandwidth_threshold: 5_000_000, // 5MB/s
                    anomaly_sensitivity: 0.9,
                    auto_block_duration_seconds: 7200, // 2 hours
                },
                connection_limits: ConnectionLimitsConfig {
                    max_total_connections: 500,
                    max_connections_per_subnet: 50,
                    subnet_mask_bits: 24,
                    connection_timeout_seconds: 15,
                    idle_timeout_seconds: 180, // 3 minutes
                },
                access_control: AccessControlConfig {
                    whitelist_mode: false,
                    enable_blacklist: true,
                    enable_geo_restrictions: true,
                    allowed_countries: vec![], // Would be configured per region
                    blocked_countries: vec![], // Would be configured per policy
                },
            },
        }
    }

    /// Create casual online match security policy
    fn create_casual_online_policy() -> SecurityPolicyConfig {
        SecurityPolicyConfig {
            name: "Casual Online".to_string(),
            description: "Balanced security for casual online matches".to_string(),
            security: SecurityConfig {
                enable_command_signing: true,
                enable_encryption: true,
                enable_anti_cheat: true,
                max_commands_per_second: 30,
                session_timeout_minutes: 60,
                enable_rate_limiting: true,
                max_auth_failures: 5,
            },
            encryption: EncryptionConfig {
                algorithm: EncryptionAlgorithm::ChaCha20Poly1305,
                enable_integrity_check: true,
                key_rotation_interval: 3600, // 1 hour
                max_packet_size: 65536,
                enable_compression: false,
            },
            authentication: AuthConfig {
                method: AuthMethod::JsonWebToken,
                token_expiry_seconds: 3600, // 1 hour
                allow_guest_access: true,
                max_auth_attempts: 5,
                auth_timeout_seconds: 30,
                refresh_token_expiry_seconds: 86400 * 7, // 7 days
                ..Default::default()
            },
            anti_cheat: AntiCheatConfig {
                enabled: true,
                methods: vec![
                    AntiCheatMethod::StatisticalAnalysis,
                    AntiCheatMethod::TimingAnalysis,
                    AntiCheatMethod::PatternRecognition,
                ],
                sensitivity: 0.7,
                action_threshold: 0.8,
                history_size: 1000,
                min_commands_for_analysis: 50,
                analysis_window_seconds: 60,
                max_commands_per_second: 30.0,
                min_command_interval_ms: 10,
                learning_mode: false,
            },
            key_exchange: KeyExchangeConfig {
                algorithm: KeyExchangeAlgorithm::X25519Ed25519,
                session_timeout_seconds: 300, // 5 minutes
                enable_pfs: true,
                max_concurrent_sessions: 100,
                pre_shared_key: None,
            },
            network_security: NetworkSecurityConfig {
                enable_rate_limiting: true,
                enable_ddos_protection: true,
                enable_ip_access_control: true,
                rate_limiting: RateLimitConfig::default(),
                ddos_protection: DDoSProtectionConfig::default(),
                connection_limits: ConnectionLimitsConfig::default(),
                access_control: AccessControlConfig::default(),
            },
        }
    }

    /// Create LAN/trusted network security policy
    fn create_lan_trusted_policy() -> SecurityPolicyConfig {
        SecurityPolicyConfig {
            name: "LAN Trusted".to_string(),
            description: "Minimal security for LAN/trusted networks".to_string(),
            security: SecurityConfig {
                enable_command_signing: false, // Disabled for performance
                enable_encryption: false,      // Disabled for LAN
                enable_anti_cheat: true,       // Still enabled but relaxed
                max_commands_per_second: 60,   // Higher limit
                session_timeout_minutes: 240,  // 4 hours
                enable_rate_limiting: false,   // Disabled for LAN
                max_auth_failures: 10,
            },
            encryption: EncryptionConfig {
                algorithm: EncryptionAlgorithm::None, // No encryption
                enable_integrity_check: false,
                key_rotation_interval: 0, // Disabled
                max_packet_size: 131072, // 128KB
                enable_compression: true, // Can enable for LAN
            },
            authentication: AuthConfig {
                method: AuthMethod::Guest, // Relaxed auth
                token_expiry_seconds: 86400, // 24 hours
                allow_guest_access: true,
                max_auth_attempts: 10,
                auth_timeout_seconds: 60,
                refresh_token_expiry_seconds: 86400 * 30, // 30 days
                ..Default::default()
            },
            anti_cheat: AntiCheatConfig {
                enabled: true,
                methods: vec![
                    AntiCheatMethod::StatisticalAnalysis, // Reduced methods
                    AntiCheatMethod::TimingAnalysis,
                ],
                sensitivity: 0.5, // Lower sensitivity
                action_threshold: 0.9, // Higher threshold
                history_size: 500,
                min_commands_for_analysis: 100,
                analysis_window_seconds: 120,
                max_commands_per_second: 60.0,
                min_command_interval_ms: 2,
                learning_mode: true, // Enable learning mode
            },
            key_exchange: KeyExchangeConfig {
                algorithm: KeyExchangeAlgorithm::PreShared, // Use pre-shared keys
                session_timeout_seconds: 3600,
                enable_pfs: false, // Disabled for simplicity
                max_concurrent_sessions: 200,
                pre_shared_key: Some(b"trusted_lan_key_2025".to_vec()),
            },
            network_security: NetworkSecurityConfig {
                enable_rate_limiting: false,
                enable_ddos_protection: false, // Not needed on LAN
                enable_ip_access_control: false,
                rate_limiting: RateLimitConfig {
                    requests_per_second_per_ip: 200,
                    requests_per_second_per_player: 100,
                    ..Default::default()
                },
                ddos_protection: DDoSProtectionConfig {
                    max_connections_per_ip: 50, // Higher for LAN
                    ..Default::default()
                },
                connection_limits: ConnectionLimitsConfig {
                    max_total_connections: 2000,
                    max_connections_per_subnet: 1000,
                    connection_timeout_seconds: 60,
                    idle_timeout_seconds: 1200, // 20 minutes
                    ..Default::default()
                },
                access_control: AccessControlConfig {
                    whitelist_mode: false,
                    enable_blacklist: false, // Disabled for LAN
                    enable_geo_restrictions: false,
                    allowed_countries: vec![],
                    blocked_countries: vec![],
                },
            },
        }
    }

    /// Create development/testing security policy
    fn create_development_policy() -> SecurityPolicyConfig {
        SecurityPolicyConfig {
            name: "Development".to_string(),
            description: "Development/testing mode with extensive logging".to_string(),
            security: SecurityConfig {
                enable_command_signing: true,
                enable_encryption: true,
                enable_anti_cheat: true,
                max_commands_per_second: 100, // Very high for testing
                session_timeout_minutes: 480, // 8 hours
                enable_rate_limiting: false,  // Disabled for development
                max_auth_failures: 999, // Essentially unlimited
            },
            encryption: EncryptionConfig {
                algorithm: EncryptionAlgorithm::ChaCha20Poly1305,
                enable_integrity_check: true,
                key_rotation_interval: 300, // 5 minutes for testing
                max_packet_size: 262144, // 256KB for testing
                enable_compression: true,
            },
            authentication: AuthConfig {
                method: AuthMethod::Guest, // Easy for development
                token_expiry_seconds: 86400, // 24 hours
                allow_guest_access: true,
                max_auth_attempts: 999,
                auth_timeout_seconds: 120,
                refresh_token_expiry_seconds: 86400 * 365, // 1 year
                ..Default::default()
            },
            anti_cheat: AntiCheatConfig {
                enabled: true,
                methods: vec![
                    AntiCheatMethod::StatisticalAnalysis,
                    AntiCheatMethod::TimingAnalysis,
                    AntiCheatMethod::StateConsistency,
                    AntiCheatMethod::PatternRecognition,
                ],
                sensitivity: 0.3, // Very low for testing
                action_threshold: 0.95, // Very high threshold
                history_size: 5000, // Large for analysis
                min_commands_for_analysis: 10,
                analysis_window_seconds: 300,
                max_commands_per_second: 100.0,
                min_command_interval_ms: 1,
                learning_mode: true, // Always learning in dev
            },
            key_exchange: KeyExchangeConfig {
                algorithm: KeyExchangeAlgorithm::X25519Ed25519,
                session_timeout_seconds: 600,
                enable_pfs: true,
                max_concurrent_sessions: 1000,
                pre_shared_key: None,
            },
            network_security: NetworkSecurityConfig {
                enable_rate_limiting: false,
                enable_ddos_protection: false, // Disabled for development
                enable_ip_access_control: false,
                rate_limiting: RateLimitConfig {
                    requests_per_second_per_ip: 1000,
                    requests_per_second_per_player: 500,
                    violation_threshold: 999,
                    ..Default::default()
                },
                ddos_protection: DDoSProtectionConfig {
                    max_connections_per_ip: 100,
                    connection_rate_threshold: 1000,
                    anomaly_sensitivity: 0.1, // Very low
                    ..Default::default()
                },
                connection_limits: ConnectionLimitsConfig {
                    max_total_connections: 10000,
                    max_connections_per_subnet: 5000,
                    connection_timeout_seconds: 300,
                    idle_timeout_seconds: 3600,
                    ..Default::default()
                },
                access_control: AccessControlConfig {
                    whitelist_mode: false,
                    enable_blacklist: false,
                    enable_geo_restrictions: false,
                    allowed_countries: vec![],
                    blocked_countries: vec![],
                },
            },
        }
    }

    /// Get current active policy
    pub fn get_current_policy(&self) -> SecurityPolicy {
        self.current_policy
    }

    /// Set active policy
    pub fn set_policy(&mut self, policy: SecurityPolicy) -> Result<(), String> {
        if self.policies.contains_key(&policy) {
            self.current_policy = policy;
            Ok(())
        } else {
            Err(format!("Policy {:?} not found", policy))
        }
    }

    /// Get policy configuration
    pub fn get_policy_config(&self, policy: SecurityPolicy) -> Option<&SecurityPolicyConfig> {
        self.policies.get(&policy)
    }

    /// Get current policy configuration
    pub fn get_current_config(&self) -> Option<&SecurityPolicyConfig> {
        self.policies.get(&self.current_policy)
    }

    /// Add custom policy
    pub fn add_custom_policy(&mut self, config: SecurityPolicyConfig) -> SecurityPolicy {
        let policy = SecurityPolicy::Custom;
        self.policies.insert(policy, config);
        policy
    }

    /// List available policies
    pub fn list_policies(&self) -> Vec<(SecurityPolicy, &str)> {
        self.policies.iter()
            .map(|(policy, config)| (*policy, config.name.as_str()))
            .collect()
    }

    /// Create security manager with current policy
    pub fn create_security_manager(&self) -> Result<SecurityManager, NetworkError> {
        if let Some(config) = self.get_current_config() {
            SecurityManager::with_config(config.security.clone())
        } else {
            Err(NetworkError::configuration("no active security policy"))
        }
    }

    /// Validate policy configuration
    pub fn validate_policy(&self, policy: SecurityPolicy) -> Result<Vec<String>, String> {
        let config = self.get_policy_config(policy)
            .ok_or_else(|| format!("Policy {:?} not found", policy))?;

        let mut warnings = Vec::new();

        // Check for potentially insecure configurations
        if !config.security.enable_encryption && policy != SecurityPolicy::LanTrusted {
            warnings.push("Encryption is disabled - data will be sent in plaintext".to_string());
        }

        if !config.security.enable_anti_cheat {
            warnings.push("Anti-cheat is disabled - cheating detection unavailable".to_string());
        }

        if config.authentication.allow_guest_access && policy == SecurityPolicy::Competitive {
            warnings.push("Guest access enabled in competitive mode".to_string());
        }

        if config.network_security.rate_limiting.requests_per_second_per_ip > 200 {
            warnings.push("Very high rate limit may allow abuse".to_string());
        }

        // Check for configuration conflicts
        if config.security.enable_encryption && config.encryption.algorithm == EncryptionAlgorithm::None {
            warnings.push("Encryption enabled but algorithm set to None".to_string());
        }

        if config.key_exchange.algorithm == KeyExchangeAlgorithm::PreShared && config.key_exchange.pre_shared_key.is_none() {
            warnings.push("Pre-shared key algorithm selected but no key provided".to_string());
        }

        Ok(warnings)
    }

    /// Get policy recommendations based on game mode
    pub fn get_policy_recommendation(&self, players: u32, is_public: bool, is_competitive: bool) -> SecurityPolicy {
        match (is_competitive, is_public, players) {
            (true, _, _) => SecurityPolicy::Competitive,
            (false, true, _) => SecurityPolicy::CasualOnline,
            (false, false, n) if n <= 8 => SecurityPolicy::LanTrusted,
            (false, false, _) => SecurityPolicy::CasualOnline,
        }
    }
}

impl Default for SecurityPolicyManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_policy_manager_creation() {
        let manager = SecurityPolicyManager::new();
        assert_eq!(manager.get_current_policy(), SecurityPolicy::CasualOnline);
        
        let policies = manager.list_policies();
        assert!(policies.len() >= 4); // At least the 4 predefined policies
    }

    #[test]
    fn test_policy_switching() {
        let mut manager = SecurityPolicyManager::new();
        
        assert!(manager.set_policy(SecurityPolicy::Competitive).is_ok());
        assert_eq!(manager.get_current_policy(), SecurityPolicy::Competitive);
        
        assert!(manager.set_policy(SecurityPolicy::LanTrusted).is_ok());
        assert_eq!(manager.get_current_policy(), SecurityPolicy::LanTrusted);
    }

    #[test]
    fn test_policy_validation() {
        let manager = SecurityPolicyManager::new();
        
        let warnings = manager.validate_policy(SecurityPolicy::LanTrusted).unwrap();
        // LAN trusted policy should have some warnings about disabled security
        assert!(!warnings.is_empty());
        
        let warnings = manager.validate_policy(SecurityPolicy::Competitive).unwrap();
        // Competitive policy should have minimal warnings
        println!("Competitive policy warnings: {:?}", warnings);
    }

    #[test]
    fn test_policy_recommendations() {
        let manager = SecurityPolicyManager::new();
        
        assert_eq!(manager.get_policy_recommendation(8, true, true), SecurityPolicy::Competitive);
        assert_eq!(manager.get_policy_recommendation(4, true, false), SecurityPolicy::CasualOnline);
        assert_eq!(manager.get_policy_recommendation(4, false, false), SecurityPolicy::LanTrusted);
        assert_eq!(manager.get_policy_recommendation(16, false, false), SecurityPolicy::CasualOnline);
    }
}