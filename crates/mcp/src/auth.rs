//! MCP Authentication and Rate Limiting
//!
//! Provides API key authentication and rate limiting for MCP requests.

use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use tracing::{debug, warn};

/// API key entry with metadata
#[derive(Debug, Clone)]
pub struct ApiKeyEntry {
    /// The API key value
    pub key: String,
    /// Human-readable name for this key
    pub name: String,
    /// Maximum requests per minute
    pub requests_per_minute: u32,
    /// When the key was created
    pub created_at: chrono::DateTime<chrono::Utc>,
    /// Whether the key is active
    pub active: bool,
}

/// Rate limit tracking for a single key
#[derive(Debug)]
struct RateLimitEntry {
    /// Request timestamps (within the current window)
    requests: VecDeque<Instant>,
}

impl RateLimitEntry {
    fn new() -> Self {
        Self { requests: VecDeque::new() }
    }

    /// Drop timestamps that are outside the current rate-limit window.
    fn prune_expired(&mut self, now: Instant, window: Duration) {
        let window_start = now - window;
        while let Some(oldest) = self.requests.front() {
            if *oldest <= window_start {
                self.requests.pop_front();
            } else {
                break;
            }
        }
    }

    /// Clean old requests outside the window and check/add new request
    /// Returns Ok(count) if request is allowed, Err(retry_after_secs) if rate limited
    fn record_request(&mut self, window: Duration, limit: usize) -> Result<usize, u64> {
        let now = Instant::now();
        self.prune_expired(now, window);

        // Check limit BEFORE adding (prevents off-by-one error)
        if self.requests.len() >= limit {
            // Calculate retry after time
            let retry_after = self
                .requests
                .front()
                .map(|oldest| {
                    let elapsed = now.saturating_duration_since(*oldest);
                    window.saturating_sub(elapsed).as_secs().max(1)
                })
                .unwrap_or(window.as_secs().max(1));
            return Err(retry_after);
        }

        // Add new request
        self.requests.push_back(now);
        Ok(self.requests.len())
    }

    /// Get current request count in window
    fn count(&self, window: Duration) -> usize {
        let now = Instant::now();
        let window_start = now - window;
        self.requests.iter().filter(|&&t| t > window_start).count()
    }

    /// Reset the window
    fn reset(&mut self) {
        self.requests.clear();
    }
}

/// Authentication and rate limiting manager
#[derive(Debug, Clone)]
pub struct AuthManager {
    /// Valid API keys
    api_keys: Arc<RwLock<HashMap<String, ApiKeyEntry>>>,
    /// Rate limit tracking per key
    rate_limits: Arc<RwLock<HashMap<String, RateLimitEntry>>>,
    /// Rate limit window duration (default: 1 minute)
    rate_limit_window: Duration,
    /// Whether authentication is required
    auth_required: bool,
}

impl AuthManager {
    /// Create a new auth manager with no authentication required
    pub fn no_auth() -> Self {
        Self {
            api_keys: Arc::new(RwLock::new(HashMap::new())),
            rate_limits: Arc::new(RwLock::new(HashMap::new())),
            rate_limit_window: Duration::from_secs(60),
            auth_required: false,
        }
    }

    /// Create a new auth manager with the given API keys
    pub fn with_keys(api_keys: Vec<ApiKeyEntry>) -> Self {
        let keys: HashMap<String, ApiKeyEntry> =
            api_keys.into_iter().map(|entry| (entry.key.clone(), entry)).collect();

        Self {
            api_keys: Arc::new(RwLock::new(keys)),
            rate_limits: Arc::new(RwLock::new(HashMap::new())),
            rate_limit_window: Duration::from_secs(60),
            auth_required: true,
        }
    }

    /// Create auth manager from configuration
    pub fn from_config(config: &AuthConfig) -> Self {
        if !config.enabled || config.api_keys.is_empty() {
            return Self::no_auth();
        }

        let keys: Vec<ApiKeyEntry> = config
            .api_keys
            .iter()
            .map(|key_config| ApiKeyEntry {
                key: key_config.key.clone(),
                name: key_config.name.clone(),
                requests_per_minute: {
                    if key_config.requests_per_minute == 0 {
                        warn!(
                            key_name = %key_config.name,
                            "requests_per_minute=0 is invalid; clamping to 1"
                        );
                    }
                    key_config.requests_per_minute.max(1)
                },
                created_at: chrono::Utc::now(),
                active: true,
            })
            .collect();

        let mut manager = Self::with_keys(keys);
        manager.rate_limit_window = Duration::from_secs(config.rate_limit_window_secs.max(1));
        manager
    }

    /// Validate an API key and check rate limits
    pub async fn validate_request(&self, api_key: Option<&str>) -> AuthResult {
        // If auth not required, allow all requests
        if !self.auth_required {
            return AuthResult::Allowed {
                key_name: "anonymous".to_string(),
                remaining_requests: u32::MAX,
            };
        }

        // Check if key was provided
        let key = match api_key.map(str::trim).filter(|k| !k.is_empty()) {
            Some(k) => k,
            None => {
                return AuthResult::Denied {
                    reason: "API key required".to_string(),
                    retry_after: None,
                };
            }
        };

        // Look up the key
        let keys = self.api_keys.read().await;
        let entry = match keys.get(key) {
            Some(e) => e.clone(),
            None => {
                return AuthResult::Denied {
                    reason: "Invalid API key".to_string(),
                    retry_after: None,
                };
            }
        };
        drop(keys);

        // Check if key is active
        if !entry.active {
            return AuthResult::Denied {
                reason: "API key deactivated".to_string(),
                retry_after: None,
            };
        }

        // Check rate limit
        let mut limits = self.rate_limits.write().await;
        let limit_entry = limits.entry(key.to_string()).or_insert_with(RateLimitEntry::new);

        let limit = entry.requests_per_minute.max(1) as usize;
        let request_count = match limit_entry.record_request(self.rate_limit_window, limit) {
            Ok(count) => count,
            Err(retry_after) => {
                warn!(
                    key_name = %entry.name,
                    limit = limit,
                    "Rate limit exceeded"
                );
                return AuthResult::Denied {
                    reason: "Rate limit exceeded".to_string(),
                    retry_after: Some(retry_after as u32),
                };
            }
        };

        let remaining = (limit - request_count) as u32;
        debug!(key_name = %entry.name, remaining = remaining, "Request allowed");

        AuthResult::Allowed { key_name: entry.name.clone(), remaining_requests: remaining }
    }

    /// Add a new API key
    pub async fn add_key(&self, entry: ApiKeyEntry) -> Result<(), String> {
        let mut keys = self.api_keys.write().await;
        if keys.contains_key(&entry.key) {
            return Err("API key already exists".to_string());
        }
        keys.insert(entry.key.clone(), entry);
        Ok(())
    }

    /// Revoke an API key
    pub async fn revoke_key(&self, key: &str) -> bool {
        let mut keys = self.api_keys.write().await;
        let removed = keys.remove(key).is_some();
        drop(keys);

        if removed {
            let mut limits = self.rate_limits.write().await;
            limits.remove(key);
        }

        removed
    }

    /// Rotate an API key in-place.
    ///
    /// This atomically revokes the old key and inserts the new key entry.
    /// Existing rate-limit state for the old key is dropped.
    pub async fn rotate_key(&self, old_key: &str, new_entry: ApiKeyEntry) -> Result<(), String> {
        if old_key.trim().is_empty() {
            return Err("old API key is required".to_string());
        }
        if new_entry.key.trim().is_empty() {
            return Err("new API key is required".to_string());
        }

        let mut keys = self.api_keys.write().await;
        if !keys.contains_key(old_key) {
            return Err("old API key does not exist".to_string());
        }
        if old_key != new_entry.key && keys.contains_key(&new_entry.key) {
            return Err("new API key already exists".to_string());
        }
        keys.remove(old_key);
        keys.insert(new_entry.key.clone(), new_entry.clone());
        drop(keys);

        let mut limits = self.rate_limits.write().await;
        limits.remove(old_key);
        limits.remove(&new_entry.key);
        Ok(())
    }

    /// List all API keys (without the actual key values)
    pub async fn list_keys(&self) -> Vec<ApiKeyInfo> {
        let keys = self.api_keys.read().await;
        keys.values()
            .map(|entry| ApiKeyInfo {
                name: entry.name.clone(),
                requests_per_minute: entry.requests_per_minute,
                created_at: entry.created_at,
                active: entry.active,
            })
            .collect()
    }

    /// Check if authentication is required
    pub fn is_auth_required(&self) -> bool {
        self.auth_required
    }
}

/// Result of authentication/authorization check
#[derive(Debug, Clone)]
pub enum AuthResult {
    /// Request is allowed
    Allowed {
        /// Name of the API key used
        key_name: String,
        /// Remaining requests in the current window
        remaining_requests: u32,
    },
    /// Request is denied
    Denied {
        /// Reason for denial
        reason: String,
        /// Seconds to wait before retry (for rate limiting)
        retry_after: Option<u32>,
    },
}

impl AuthResult {
    /// Check if the request is allowed
    pub fn is_allowed(&self) -> bool {
        matches!(self, AuthResult::Allowed { .. })
    }

    /// Get the remaining requests if allowed
    pub fn remaining(&self) -> Option<u32> {
        match self {
            AuthResult::Allowed { remaining_requests, .. } => Some(*remaining_requests),
            _ => None,
        }
    }

    /// Get the denial reason if denied
    pub fn denial_reason(&self) -> Option<&str> {
        match self {
            AuthResult::Denied { reason, .. } => Some(reason),
            _ => None,
        }
    }

    /// Get retry after seconds if rate limited
    pub fn retry_after(&self) -> Option<u32> {
        match self {
            AuthResult::Denied { retry_after, .. } => *retry_after,
            _ => None,
        }
    }
}

/// Public API key info (without the key value)
#[derive(Debug, Clone)]
pub struct ApiKeyInfo {
    pub name: String,
    pub requests_per_minute: u32,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub active: bool,
}

/// Configuration for authentication
#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct AuthConfig {
    /// Whether authentication is enabled
    #[serde(default)]
    pub enabled: bool,
    /// Rate limit window in seconds (default: 60)
    #[serde(default = "default_rate_limit_window")]
    pub rate_limit_window_secs: u64,
    /// API keys
    #[serde(default)]
    pub api_keys: Vec<ApiKeyConfig>,
}

impl Default for AuthConfig {
    fn default() -> Self {
        Self { enabled: false, rate_limit_window_secs: 60, api_keys: Vec::new() }
    }
}

/// Configuration for a single API key
#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct ApiKeyConfig {
    /// The API key value (should be a secure random string)
    pub key: String,
    /// Human-readable name
    pub name: String,
    /// Maximum requests per minute (default: 60)
    #[serde(default = "default_requests_per_minute")]
    pub requests_per_minute: u32,
}

fn default_rate_limit_window() -> u64 {
    60
}

fn default_requests_per_minute() -> u32 {
    60
}

/// Generate a new cryptographically secure API key
pub fn generate_api_key() -> String {
    use rand::rngs::OsRng;
    use rand::RngCore;

    const CHARSET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789";
    const KEY_LEN: usize = 32;

    let mut key = String::with_capacity(KEY_LEN);
    let mut bytes = vec![0u8; KEY_LEN];

    // Use OsRng for cryptographically secure randomness
    OsRng.fill_bytes(&mut bytes);

    for b in bytes {
        key.push(CHARSET[b as usize % CHARSET.len()] as char);
    }
    key
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_no_auth_mode() {
        let auth = AuthManager::no_auth();

        let result = auth.validate_request(None).await;
        assert!(result.is_allowed());
        assert_eq!(result.remaining(), Some(u32::MAX));
    }

    #[tokio::test]
    async fn test_auth_required_no_key() {
        let key = ApiKeyEntry {
            key: "test_key_123".to_string(),
            name: "Test Key".to_string(),
            requests_per_minute: 10,
            created_at: chrono::Utc::now(),
            active: true,
        };

        let auth = AuthManager::with_keys(vec![key]);

        let result = auth.validate_request(None).await;
        assert!(!result.is_allowed());
        assert_eq!(result.denial_reason(), Some("API key required"));
    }

    #[tokio::test]
    async fn test_invalid_key() {
        let key = ApiKeyEntry {
            key: "test_key_123".to_string(),
            name: "Test Key".to_string(),
            requests_per_minute: 10,
            created_at: chrono::Utc::now(),
            active: true,
        };

        let auth = AuthManager::with_keys(vec![key]);

        let result = auth.validate_request(Some("wrong_key")).await;
        assert!(!result.is_allowed());
        assert_eq!(result.denial_reason(), Some("Invalid API key"));
    }

    #[tokio::test]
    async fn test_blank_key_treated_as_missing() {
        let key = ApiKeyEntry {
            key: "test_key_123".to_string(),
            name: "Test Key".to_string(),
            requests_per_minute: 10,
            created_at: chrono::Utc::now(),
            active: true,
        };

        let auth = AuthManager::with_keys(vec![key]);
        let result = auth.validate_request(Some("   ")).await;
        assert!(!result.is_allowed());
        assert_eq!(result.denial_reason(), Some("API key required"));
    }

    #[tokio::test]
    async fn test_rate_limiting() {
        let key = ApiKeyEntry {
            key: "test_key_123".to_string(),
            name: "Test Key".to_string(),
            requests_per_minute: 2,
            created_at: chrono::Utc::now(),
            active: true,
        };

        let auth = AuthManager::with_keys(vec![key]);

        // First 2 requests should succeed
        let result1 = auth.validate_request(Some("test_key_123")).await;
        assert!(result1.is_allowed());

        let result2 = auth.validate_request(Some("test_key_123")).await;
        assert!(result2.is_allowed());

        // Third request should be rate limited
        let result3 = auth.validate_request(Some("test_key_123")).await;
        assert!(!result3.is_allowed());
        assert_eq!(result3.denial_reason(), Some("Rate limit exceeded"));
        assert!(result3.retry_after().is_some());
    }

    #[tokio::test]
    async fn test_zero_requests_per_minute_is_clamped_to_one() {
        let auth = AuthManager::from_config(&AuthConfig {
            enabled: true,
            rate_limit_window_secs: 60,
            api_keys: vec![ApiKeyConfig {
                key: "zero-rpm".to_string(),
                name: "Zero RPM".to_string(),
                requests_per_minute: 0,
            }],
        });

        let first = auth.validate_request(Some("zero-rpm")).await;
        assert!(first.is_allowed());
        let second = auth.validate_request(Some("zero-rpm")).await;
        assert!(!second.is_allowed());
        assert_eq!(second.denial_reason(), Some("Rate limit exceeded"));
    }

    #[tokio::test]
    async fn test_rotate_key_replaces_old_key() {
        let old_key = ApiKeyEntry {
            key: "old-key".to_string(),
            name: "Old Key".to_string(),
            requests_per_minute: 2,
            created_at: chrono::Utc::now(),
            active: true,
        };
        let auth = AuthManager::with_keys(vec![old_key]);

        let old_allowed = auth.validate_request(Some("old-key")).await;
        assert!(old_allowed.is_allowed());

        let rotated = ApiKeyEntry {
            key: "new-key".to_string(),
            name: "New Key".to_string(),
            requests_per_minute: 10,
            created_at: chrono::Utc::now(),
            active: true,
        };
        auth.rotate_key("old-key", rotated).await.expect("rotate key");

        let old_denied = auth.validate_request(Some("old-key")).await;
        assert!(!old_denied.is_allowed());
        assert_eq!(old_denied.denial_reason(), Some("Invalid API key"));

        let new_allowed = auth.validate_request(Some("new-key")).await;
        assert!(new_allowed.is_allowed());
    }

    #[test]
    fn test_generate_api_key() {
        let key1 = generate_api_key();
        let key2 = generate_api_key();

        assert_eq!(key1.len(), 32);
        assert_eq!(key2.len(), 32);
        assert_ne!(key1, key2); // Should be random
    }

    #[test]
    fn test_rate_limit_entry_prunes_expired_requests_from_front() {
        let mut entry = RateLimitEntry::new();
        let now = Instant::now();
        let window = Duration::from_secs(60);

        entry.requests.push_back(now - Duration::from_secs(120));
        entry.requests.push_back(now - Duration::from_secs(61));
        entry.requests.push_back(now - Duration::from_secs(5));

        entry.prune_expired(now, window);

        assert_eq!(entry.requests.len(), 1);
        let remaining = entry.requests.front().expect("remaining request");
        assert!(*remaining > now - window);
    }
}
