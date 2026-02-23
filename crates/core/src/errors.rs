use thiserror::Error;

use crate::{domain::quote::QuoteStatus, flows::FlowTransitionError};

#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum DomainError {
    #[error("invalid quote transition from {from:?} to {to:?}")]
    InvalidQuoteTransition { from: QuoteStatus, to: QuoteStatus },
    #[error(transparent)]
    FlowTransition(#[from] FlowTransitionError),
    #[error("domain invariant violation: {0}")]
    InvariantViolation(String),
}

#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum ApplicationError {
    #[error(transparent)]
    Domain(#[from] DomainError),
    #[error("persistence failure: {0}")]
    Persistence(String),
    #[error("integration failure: {0}")]
    Integration(String),
    #[error("configuration failure: {0}")]
    Configuration(String),
}

#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum InterfaceError {
    #[error("bad request: {message}")]
    BadRequest { message: String, correlation_id: String },
    #[error("service unavailable: {message}")]
    ServiceUnavailable { message: String, correlation_id: String },
    #[error("internal error: {message}")]
    Internal { message: String, correlation_id: String },
}

impl InterfaceError {
    pub fn user_message(&self) -> &'static str {
        match self {
            Self::BadRequest { .. } => {
                "The request could not be processed. Check inputs and try again."
            }
            Self::ServiceUnavailable { .. } => {
                "The service is temporarily unavailable. Please retry shortly."
            }
            Self::Internal { .. } => "An unexpected internal error occurred.",
        }
    }
}

impl ApplicationError {
    pub fn into_interface(self, correlation_id: impl Into<String>) -> InterfaceError {
        let correlation_id = correlation_id.into();
        let mut mapped = InterfaceError::from(self);
        match &mut mapped {
            InterfaceError::BadRequest { correlation_id: id, .. }
            | InterfaceError::ServiceUnavailable { correlation_id: id, .. }
            | InterfaceError::Internal { correlation_id: id, .. } => *id = correlation_id,
        }
        mapped
    }
}

impl From<ApplicationError> for InterfaceError {
    fn from(value: ApplicationError) -> Self {
        match value {
            ApplicationError::Domain(DomainError::InvalidQuoteTransition { .. })
            | ApplicationError::Domain(DomainError::FlowTransition(_))
            | ApplicationError::Domain(DomainError::InvariantViolation(_)) => Self::BadRequest {
                message: "domain validation failed".to_owned(),
                correlation_id: "unassigned".to_owned(),
            },
            ApplicationError::Persistence(message) | ApplicationError::Integration(message) => {
                Self::ServiceUnavailable { message, correlation_id: "unassigned".to_owned() }
            }
            ApplicationError::Configuration(message) => {
                Self::Internal { message, correlation_id: "unassigned".to_owned() }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::errors::{ApplicationError, DomainError, InterfaceError};

    #[test]
    fn domain_error_maps_to_bad_request_interface_error() {
        let interface = ApplicationError::from(DomainError::InvariantViolation(
            "missing required field".to_owned(),
        ))
        .into_interface("req-1");

        assert!(matches!(
            interface,
            InterfaceError::BadRequest {
                ref correlation_id,
                ..
            } if correlation_id == "req-1"
        ));
    }

    #[test]
    fn bad_request_has_user_safe_message() {
        let interface = ApplicationError::from(DomainError::InvariantViolation(
            "missing required field".to_owned(),
        ))
        .into_interface("req-2");

        assert_eq!(
            interface.user_message(),
            "The request could not be processed. Check inputs and try again."
        );
    }

    #[test]
    fn persistence_error_maps_to_service_unavailable() {
        let interface = ApplicationError::Persistence("database lock timeout".to_owned())
            .into_interface("req-3");

        assert!(matches!(interface, InterfaceError::ServiceUnavailable { .. }));
        assert_eq!(
            interface.user_message(),
            "The service is temporarily unavailable. Please retry shortly."
        );
    }

    #[test]
    fn configuration_error_maps_to_internal() {
        let interface =
            ApplicationError::Configuration("invalid app token".to_owned()).into_interface("req-4");

        assert!(matches!(interface, InterfaceError::Internal { .. }));
        assert_eq!(interface.user_message(), "An unexpected internal error occurred.");
    }
}
