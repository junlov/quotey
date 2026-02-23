use std::collections::HashMap;

use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ApproverAuthority {
    pub role: String,
    pub role_rank: u8,
    pub max_discount_pct: Decimal,
    pub allowed_account_tiers: Vec<String>,
}

impl ApproverAuthority {
    fn allows_account_tier(&self, account_tier: &str) -> bool {
        if self.allowed_account_tiers.is_empty() {
            return true;
        }

        let tier_key = normalize_key(account_tier);
        self.allowed_account_tiers
            .iter()
            .map(|tier| normalize_key(tier))
            .any(|tier| tier == "*" || tier == "all" || tier == tier_key)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ApprovalValidationInput {
    pub approver_user_id: String,
    pub approver_role: String,
    pub required_role: String,
    pub requested_discount_pct: Decimal,
    pub account_tier: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ApprovalValidationFailure {
    UnknownApproverRole {
        approver_role: String,
    },
    UnknownRequiredRole {
        required_role: String,
    },
    InsufficientRoleAuthority {
        approver_role: String,
        required_role: String,
    },
    DiscountLimitExceeded {
        approver_role: String,
        requested_discount_pct: Decimal,
        max_discount_pct: Decimal,
    },
    AccountTierNotAllowed {
        approver_role: String,
        account_tier: String,
    },
}

impl ApprovalValidationFailure {
    fn reason(&self) -> String {
        match self {
            Self::UnknownApproverRole { approver_role } => {
                format!("unknown approver role `{approver_role}`")
            }
            Self::UnknownRequiredRole { required_role } => {
                format!("unknown required role `{required_role}`")
            }
            Self::InsufficientRoleAuthority { approver_role, required_role } => {
                format!(
                    "approver role `{approver_role}` does not satisfy required role `{required_role}`"
                )
            }
            Self::DiscountLimitExceeded {
                approver_role,
                requested_discount_pct,
                max_discount_pct,
            } => {
                format!(
                    "requested discount {requested_discount_pct}% exceeds `{approver_role}` limit {max_discount_pct}%"
                )
            }
            Self::AccountTierNotAllowed { approver_role, account_tier } => {
                format!(
                    "approver role `{approver_role}` cannot approve account tier `{account_tier}`"
                )
            }
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ApprovalValidationResult {
    pub allowed: bool,
    pub reason: String,
    pub failure: Option<ApprovalValidationFailure>,
}

impl ApprovalValidationResult {
    fn allow(reason: impl Into<String>) -> Self {
        Self { allowed: true, reason: reason.into(), failure: None }
    }

    fn deny(failure: ApprovalValidationFailure) -> Self {
        Self { allowed: false, reason: failure.reason(), failure: Some(failure) }
    }
}

#[derive(Clone, Debug, Default)]
pub struct ApprovalValidator {
    authorities: HashMap<String, ApproverAuthority>,
}

impl ApprovalValidator {
    pub fn new(authorities: Vec<ApproverAuthority>) -> Self {
        let authorities = authorities
            .into_iter()
            .map(|authority| (normalize_key(&authority.role), authority))
            .collect();

        Self { authorities }
    }

    pub fn validate(&self, input: &ApprovalValidationInput) -> ApprovalValidationResult {
        let approver_key = normalize_key(&input.approver_role);
        let required_key = normalize_key(&input.required_role);

        let Some(approver_authority) = self.authorities.get(&approver_key) else {
            return ApprovalValidationResult::deny(
                ApprovalValidationFailure::UnknownApproverRole {
                    approver_role: input.approver_role.clone(),
                },
            );
        };

        let Some(required_authority) = self.authorities.get(&required_key) else {
            return ApprovalValidationResult::deny(
                ApprovalValidationFailure::UnknownRequiredRole {
                    required_role: input.required_role.clone(),
                },
            );
        };

        if approver_authority.role_rank < required_authority.role_rank {
            return ApprovalValidationResult::deny(
                ApprovalValidationFailure::InsufficientRoleAuthority {
                    approver_role: input.approver_role.clone(),
                    required_role: input.required_role.clone(),
                },
            );
        }

        if input.requested_discount_pct > approver_authority.max_discount_pct {
            return ApprovalValidationResult::deny(
                ApprovalValidationFailure::DiscountLimitExceeded {
                    approver_role: input.approver_role.clone(),
                    requested_discount_pct: input.requested_discount_pct,
                    max_discount_pct: approver_authority.max_discount_pct,
                },
            );
        }

        if !approver_authority.allows_account_tier(&input.account_tier) {
            return ApprovalValidationResult::deny(
                ApprovalValidationFailure::AccountTierNotAllowed {
                    approver_role: input.approver_role.clone(),
                    account_tier: input.account_tier.clone(),
                },
            );
        }

        ApprovalValidationResult::allow(format!(
            "approver `{}` is authorized for `{}` at {}% discount on `{}` tier",
            input.approver_user_id,
            input.required_role,
            input.requested_discount_pct,
            input.account_tier
        ))
    }
}

fn normalize_key(raw: &str) -> String {
    raw.trim().to_ascii_lowercase()
}

#[cfg(test)]
mod tests {
    use rust_decimal::Decimal;

    use super::{
        ApprovalValidationFailure, ApprovalValidationInput, ApprovalValidator, ApproverAuthority,
    };

    fn validator() -> ApprovalValidator {
        ApprovalValidator::new(vec![
            ApproverAuthority {
                role: "sales_rep".to_string(),
                role_rank: 1,
                max_discount_pct: Decimal::new(500, 2),
                allowed_account_tiers: vec!["smb".to_string()],
            },
            ApproverAuthority {
                role: "sales_manager".to_string(),
                role_rank: 2,
                max_discount_pct: Decimal::new(2000, 2),
                allowed_account_tiers: vec!["smb".to_string(), "mid_market".to_string()],
            },
            ApproverAuthority {
                role: "vp_sales".to_string(),
                role_rank: 3,
                max_discount_pct: Decimal::new(3000, 2),
                allowed_account_tiers: vec!["*".to_string()],
            },
        ])
    }

    #[test]
    fn allows_approval_when_role_discount_and_tier_are_valid() {
        let result = validator().validate(&ApprovalValidationInput {
            approver_user_id: "u-manager".to_string(),
            approver_role: "sales_manager".to_string(),
            required_role: "sales_manager".to_string(),
            requested_discount_pct: Decimal::new(1500, 2),
            account_tier: "mid_market".to_string(),
        });

        assert!(result.allowed);
        assert!(result.failure.is_none());
    }

    #[test]
    fn denies_unknown_approver_role() {
        let result = validator().validate(&ApprovalValidationInput {
            approver_user_id: "u-unknown".to_string(),
            approver_role: "intern".to_string(),
            required_role: "sales_manager".to_string(),
            requested_discount_pct: Decimal::new(1000, 2),
            account_tier: "smb".to_string(),
        });

        assert_eq!(
            result.failure,
            Some(ApprovalValidationFailure::UnknownApproverRole {
                approver_role: "intern".to_string(),
            })
        );
    }

    #[test]
    fn denies_when_role_authority_is_below_required_role() {
        let result = validator().validate(&ApprovalValidationInput {
            approver_user_id: "u-rep".to_string(),
            approver_role: "sales_rep".to_string(),
            required_role: "sales_manager".to_string(),
            requested_discount_pct: Decimal::new(500, 2),
            account_tier: "smb".to_string(),
        });

        assert_eq!(
            result.failure,
            Some(ApprovalValidationFailure::InsufficientRoleAuthority {
                approver_role: "sales_rep".to_string(),
                required_role: "sales_manager".to_string(),
            })
        );
    }

    #[test]
    fn denies_when_discount_exceeds_role_limit() {
        let result = validator().validate(&ApprovalValidationInput {
            approver_user_id: "u-manager".to_string(),
            approver_role: "sales_manager".to_string(),
            required_role: "sales_manager".to_string(),
            requested_discount_pct: Decimal::new(2100, 2),
            account_tier: "mid_market".to_string(),
        });

        assert_eq!(
            result.failure,
            Some(ApprovalValidationFailure::DiscountLimitExceeded {
                approver_role: "sales_manager".to_string(),
                requested_discount_pct: Decimal::new(2100, 2),
                max_discount_pct: Decimal::new(2000, 2),
            })
        );
    }

    #[test]
    fn denies_when_account_tier_is_not_permitted_for_role() {
        let result = validator().validate(&ApprovalValidationInput {
            approver_user_id: "u-manager".to_string(),
            approver_role: "sales_manager".to_string(),
            required_role: "sales_manager".to_string(),
            requested_discount_pct: Decimal::new(1900, 2),
            account_tier: "enterprise".to_string(),
        });

        assert_eq!(
            result.failure,
            Some(ApprovalValidationFailure::AccountTierNotAllowed {
                approver_role: "sales_manager".to_string(),
                account_tier: "enterprise".to_string(),
            })
        );
    }

    #[test]
    fn wildcard_account_tier_allows_any_tier() {
        let result = validator().validate(&ApprovalValidationInput {
            approver_user_id: "u-vp".to_string(),
            approver_role: "vp_sales".to_string(),
            required_role: "sales_manager".to_string(),
            requested_discount_pct: Decimal::new(2500, 2),
            account_tier: "enterprise".to_string(),
        });

        assert!(result.allowed);
    }
}
