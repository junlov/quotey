use serde::{Deserialize, Serialize};

/// Product surface where authentication is evaluated.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AuthChannel {
    Slack,
    Mcp,
    Portal,
    MobilePwa,
    Cli,
}

/// Concrete mechanism used to authenticate a request.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AuthMethod {
    None,
    ApiKey,
    LinkToken,
    Password,
    WebAuthn,
    OAuthBearer,
}

/// Assurance tier derived from the chosen method.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AuthStrength {
    Anonymous,
    Possession,
    PossessionAndKnowledge,
    PossessionAndBiometric,
    FederatedIdentity,
}

impl AuthStrength {
    pub fn is_high_assurance(self) -> bool {
        matches!(self, Self::PossessionAndBiometric | Self::FederatedIdentity)
    }
}

/// Canonical actor identity for audit and authorization checks.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuthPrincipal {
    /// Stable actor identifier (`user:<id>`, `portal:<email>`, `mcp:<key-name>`).
    pub actor_id: String,
    /// Human-readable display name when available.
    pub display_name: Option<String>,
}

/// Shared request authentication context carried through deterministic engines.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuthContext {
    pub channel: AuthChannel,
    pub method: AuthMethod,
    pub strength: AuthStrength,
    pub principal: AuthPrincipal,
    /// Credential/session fingerprint for replay detection and audit joins.
    pub token_fingerprint: Option<String>,
    pub session_id: Option<String>,
}

/// Canonical authentication failure reason codes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AuthErrorCode {
    MissingCredential,
    InvalidCredential,
    CredentialExpired,
    CredentialRevoked,
    UnauthorizedScope,
    UnsupportedMethod,
    RateLimited,
}

impl AuthErrorCode {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::MissingCredential => "missing_credential",
            Self::InvalidCredential => "invalid_credential",
            Self::CredentialExpired => "credential_expired",
            Self::CredentialRevoked => "credential_revoked",
            Self::UnauthorizedScope => "unauthorized_scope",
            Self::UnsupportedMethod => "unsupported_method",
            Self::RateLimited => "rate_limited",
        }
    }
}

/// Transport-agnostic authentication error payload.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuthError {
    pub code: AuthErrorCode,
    pub message: String,
    pub retry_after_seconds: Option<u32>,
}

impl AuthError {
    pub fn new(code: AuthErrorCode, message: impl Into<String>) -> Self {
        Self { code, message: message.into(), retry_after_seconds: None }
    }

    pub fn with_retry_after(mut self, retry_after_seconds: u32) -> Self {
        self.retry_after_seconds = Some(retry_after_seconds.max(1));
        self
    }

    pub fn http_status(&self) -> u16 {
        match self.code {
            AuthErrorCode::MissingCredential
            | AuthErrorCode::InvalidCredential
            | AuthErrorCode::CredentialExpired
            | AuthErrorCode::CredentialRevoked => 401,
            AuthErrorCode::UnauthorizedScope => 403,
            AuthErrorCode::UnsupportedMethod => 400,
            AuthErrorCode::RateLimited => 429,
        }
    }

    pub fn is_retryable(&self) -> bool {
        matches!(
            self.code,
            AuthErrorCode::RateLimited
                | AuthErrorCode::CredentialExpired
                | AuthErrorCode::UnsupportedMethod
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn high_assurance_strengths_are_explicit() {
        assert!(AuthStrength::PossessionAndBiometric.is_high_assurance());
        assert!(AuthStrength::FederatedIdentity.is_high_assurance());
        assert!(!AuthStrength::Possession.is_high_assurance());
    }

    #[test]
    fn auth_error_status_mapping_is_deterministic() {
        assert_eq!(AuthError::new(AuthErrorCode::MissingCredential, "missing").http_status(), 401);
        assert_eq!(
            AuthError::new(AuthErrorCode::UnauthorizedScope, "forbidden").http_status(),
            403
        );
        assert_eq!(AuthError::new(AuthErrorCode::RateLimited, "slow down").http_status(), 429);
        assert_eq!(
            AuthError::new(AuthErrorCode::UnsupportedMethod, "bad method").http_status(),
            400
        );
    }

    #[test]
    fn auth_error_codes_have_stable_canonical_strings() {
        assert_eq!(AuthErrorCode::MissingCredential.as_str(), "missing_credential");
        assert_eq!(AuthErrorCode::InvalidCredential.as_str(), "invalid_credential");
        assert_eq!(AuthErrorCode::CredentialExpired.as_str(), "credential_expired");
        assert_eq!(AuthErrorCode::CredentialRevoked.as_str(), "credential_revoked");
        assert_eq!(AuthErrorCode::UnauthorizedScope.as_str(), "unauthorized_scope");
        assert_eq!(AuthErrorCode::UnsupportedMethod.as_str(), "unsupported_method");
        assert_eq!(AuthErrorCode::RateLimited.as_str(), "rate_limited");
    }

    #[test]
    fn rate_limit_error_retry_after_is_clamped() {
        let err = AuthError::new(AuthErrorCode::RateLimited, "rate limited").with_retry_after(0);
        assert_eq!(err.retry_after_seconds, Some(1));
        assert!(err.is_retryable());
    }
}
