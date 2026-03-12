//! Authentication services

use serde::{Deserialize, Serialize};

/// Authentication token
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthToken {
    /// Token value
    pub token: String,
    /// Expiration time (Unix timestamp)
    pub expires_at: u64,
    /// User ID
    pub user_id: String,
}

impl AuthToken {
    /// Create a new auth token
    pub fn new(token: String, user_id: String, expires_at: u64) -> Self {
        Self {
            token,
            expires_at,
            user_id,
        }
    }

    /// Check if token is expired
    pub fn is_expired(&self) -> bool {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        now >= self.expires_at
    }
}

impl Default for AuthToken {
    fn default() -> Self {
        Self {
            token: "default_token".to_string(),
            user_id: "guest".to_string(),
            expires_at: u64::MAX, // Never expires for default
        }
    }
}

/// Auth service
pub struct AuthService;

impl AuthService {
    /// Create new auth service
    pub fn new() -> Self {
        Self
    }
}

impl Default for AuthService {
    fn default() -> Self {
        Self::new()
    }
}
