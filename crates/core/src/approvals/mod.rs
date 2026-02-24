use std::collections::{HashMap, HashSet};

use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use thiserror::Error;

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

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RoutingApprover {
    pub user_id: String,
    pub role: String,
    pub role_rank: u8,
    pub manager_id: Option<String>,
    pub max_discount_pct: Decimal,
    pub max_deal_value: Decimal,
    pub allowed_account_tiers: Vec<String>,
    pub allowed_product_categories: Vec<String>,
}

impl RoutingApprover {
    fn allows_account_tier(&self, account_tier: &str) -> bool {
        contains_with_wildcard(&self.allowed_account_tiers, account_tier)
    }

    fn allows_product_category(&self, product_category: &str) -> bool {
        contains_with_wildcard(&self.allowed_product_categories, product_category)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ApprovalRoutingInput {
    pub requester_user_id: String,
    pub required_role: String,
    pub requested_discount_pct: Decimal,
    pub deal_value: Decimal,
    pub account_tier: String,
    pub product_category: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RoutingRule {
    pub id: String,
    pub required_role: String,
    pub account_tier: Option<String>,
    pub product_category: Option<String>,
    pub min_deal_value: Option<Decimal>,
    pub min_discount_pct: Option<Decimal>,
    pub priority: i32,
}

impl RoutingRule {
    fn matches(&self, input: &ApprovalRoutingInput) -> bool {
        if normalize_key(&self.required_role) != normalize_key(&input.required_role) {
            return false;
        }

        if let Some(account_tier) = &self.account_tier {
            if !contains_key(account_tier, &input.account_tier) {
                return false;
            }
        }

        if let Some(product_category) = &self.product_category {
            if !contains_key(product_category, &input.product_category) {
                return false;
            }
        }

        if let Some(min_deal_value) = self.min_deal_value {
            if input.deal_value < min_deal_value {
                return false;
            }
        }

        if let Some(min_discount_pct) = self.min_discount_pct {
            if input.requested_discount_pct < min_discount_pct {
                return false;
            }
        }

        true
    }

    fn specificity(&self) -> usize {
        usize::from(self.account_tier.is_some())
            + usize::from(self.product_category.is_some())
            + usize::from(self.min_deal_value.is_some())
            + usize::from(self.min_discount_pct.is_some())
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RoutingDecision {
    pub selected_approver_user_id: String,
    pub primary_approver_user_id: String,
    pub matched_rule_id: Option<String>,
    pub escalation_reason: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Error)]
pub enum RoutingError {
    #[error("unknown required role `{required_role}`")]
    UnknownRequiredRole { required_role: String },
    #[error(
        "no eligible approver for role `{required_role}` at account tier `{account_tier}` and product category `{product_category}`"
    )]
    NoEligibleApprover { required_role: String, account_tier: String, product_category: String },
    #[error("primary approver `{primary_approver_user_id}` unavailable and no fallback or escalation path found")]
    NoAvailableApprover { primary_approver_user_id: String },
}

pub trait CalendarAvailabilityClient {
    fn is_available(&self, user_id: &str) -> Result<bool, String>;
}

#[derive(Clone, Debug, Default)]
pub struct InMemoryCalendarAvailabilityClient {
    unavailable_user_ids: HashSet<String>,
}

impl InMemoryCalendarAvailabilityClient {
    pub fn with_unavailable_users(unavailable_user_ids: Vec<String>) -> Self {
        Self {
            unavailable_user_ids: unavailable_user_ids
                .into_iter()
                .map(|user_id| normalize_key(&user_id))
                .collect(),
        }
    }
}

impl CalendarAvailabilityClient for InMemoryCalendarAvailabilityClient {
    fn is_available(&self, user_id: &str) -> Result<bool, String> {
        Ok(!self.unavailable_user_ids.contains(&normalize_key(user_id)))
    }
}

#[derive(Clone, Debug)]
pub struct RoutingEngine<C> {
    approvers_by_user: HashMap<String, RoutingApprover>,
    rules: Vec<RoutingRule>,
    calendar_client: C,
}

impl<C> RoutingEngine<C>
where
    C: CalendarAvailabilityClient,
{
    pub fn new(
        approvers: Vec<RoutingApprover>,
        rules: Vec<RoutingRule>,
        calendar_client: C,
    ) -> Self {
        let approvers_by_user = approvers
            .into_iter()
            .map(|approver| (normalize_key(&approver.user_id), approver))
            .collect();
        Self { approvers_by_user, rules, calendar_client }
    }

    pub fn route(&self, input: &ApprovalRoutingInput) -> Result<RoutingDecision, RoutingError> {
        let matched_rule = self.match_rule(input);
        let effective_required_role = matched_rule
            .as_ref()
            .map(|rule| rule.required_role.clone())
            .unwrap_or_else(|| input.required_role.clone());
        let required_rank = self.required_role_rank(&effective_required_role).ok_or_else(|| {
            RoutingError::UnknownRequiredRole { required_role: effective_required_role.clone() }
        })?;

        let mut eligible: Vec<&RoutingApprover> = self
            .approvers_by_user
            .values()
            .filter(|approver| {
                approver.role_rank >= required_rank
                    && approver.requested_discount_allows(input.requested_discount_pct)
                    && approver.max_deal_value >= input.deal_value
                    && approver.allows_account_tier(&input.account_tier)
                    && approver.allows_product_category(&input.product_category)
            })
            .collect();

        if eligible.is_empty() {
            return Err(RoutingError::NoEligibleApprover {
                required_role: effective_required_role,
                account_tier: input.account_tier.clone(),
                product_category: input.product_category.clone(),
            });
        }

        eligible.sort_by(|left, right| {
            left.role_rank.cmp(&right.role_rank).then_with(|| left.user_id.cmp(&right.user_id))
        });

        let primary = self.select_primary(&eligible, &input.requester_user_id);
        let primary_user_id = primary.user_id.clone();
        if self.is_available(&primary_user_id) {
            return Ok(RoutingDecision {
                selected_approver_user_id: primary_user_id.clone(),
                primary_approver_user_id: primary_user_id,
                matched_rule_id: matched_rule.map(|rule| rule.id),
                escalation_reason: None,
            });
        }

        if let Some(fallback) = eligible
            .iter()
            .filter(|approver| approver.role_rank == primary.role_rank)
            .filter(|approver| approver.user_id != primary.user_id)
            .find(|approver| self.is_available(&approver.user_id))
        {
            return Ok(RoutingDecision {
                selected_approver_user_id: fallback.user_id.clone(),
                primary_approver_user_id: primary_user_id,
                matched_rule_id: matched_rule.map(|rule| rule.id),
                escalation_reason: Some("primary_unavailable_same_rank_fallback".to_owned()),
            });
        }

        if let Some(escalated) = eligible
            .iter()
            .filter(|approver| approver.role_rank > primary.role_rank)
            .find(|approver| self.is_available(&approver.user_id))
        {
            return Ok(RoutingDecision {
                selected_approver_user_id: escalated.user_id.clone(),
                primary_approver_user_id: primary_user_id,
                matched_rule_id: matched_rule.map(|rule| rule.id),
                escalation_reason: Some("primary_unavailable_escalated".to_owned()),
            });
        }

        Err(RoutingError::NoAvailableApprover { primary_approver_user_id: primary.user_id.clone() })
    }

    fn match_rule(&self, input: &ApprovalRoutingInput) -> Option<RoutingRule> {
        let mut matches: Vec<RoutingRule> =
            self.rules.iter().filter(|rule| rule.matches(input)).cloned().collect();
        matches.sort_by(|left, right| {
            left.priority
                .cmp(&right.priority)
                .then_with(|| right.specificity().cmp(&left.specificity()))
                .then_with(|| left.id.cmp(&right.id))
        });
        matches.into_iter().next()
    }

    fn required_role_rank(&self, required_role: &str) -> Option<u8> {
        let required_role = normalize_key(required_role);
        self.approvers_by_user
            .values()
            .filter(|approver| normalize_key(&approver.role) == required_role)
            .map(|approver| approver.role_rank)
            .max()
    }

    fn select_primary<'a>(
        &self,
        eligible: &[&'a RoutingApprover],
        requester_user_id: &str,
    ) -> &'a RoutingApprover {
        let min_rank = eligible.iter().map(|approver| approver.role_rank).min().unwrap_or(0);
        let same_rank: Vec<&RoutingApprover> =
            eligible.iter().copied().filter(|approver| approver.role_rank == min_rank).collect();
        let manager_chain = self.manager_chain(requester_user_id);

        if let Some(manager) = manager_chain
            .iter()
            .find_map(|manager_id| {
                same_rank.iter().find(|approver| normalize_key(&approver.user_id) == *manager_id)
            })
            .copied()
        {
            return manager;
        }

        same_rank
            .into_iter()
            .min_by(|left, right| left.user_id.cmp(&right.user_id))
            .unwrap_or(eligible[0])
    }

    fn manager_chain(&self, requester_user_id: &str) -> Vec<String> {
        let mut chain = Vec::new();
        let mut current = normalize_key(requester_user_id);
        let mut visited = HashSet::new();

        loop {
            if !visited.insert(current.clone()) {
                break;
            }

            let Some(approver) = self.approvers_by_user.get(&current) else {
                break;
            };
            let Some(manager_id) = &approver.manager_id else {
                break;
            };

            let manager_key = normalize_key(manager_id);
            chain.push(manager_key.clone());
            current = manager_key;
        }

        chain
    }

    fn is_available(&self, user_id: &str) -> bool {
        self.calendar_client.is_available(user_id).unwrap_or(false)
    }
}

trait DiscountAuthority {
    fn requested_discount_allows(&self, requested_discount_pct: Decimal) -> bool;
}

impl DiscountAuthority for RoutingApprover {
    fn requested_discount_allows(&self, requested_discount_pct: Decimal) -> bool {
        requested_discount_pct <= self.max_discount_pct
    }
}

fn normalize_key(raw: &str) -> String {
    raw.trim().to_ascii_lowercase()
}

fn contains_key(candidate: &str, value: &str) -> bool {
    let candidate = normalize_key(candidate);
    candidate == "*" || candidate == "all" || candidate == normalize_key(value)
}

fn contains_with_wildcard(candidates: &[String], value: &str) -> bool {
    if candidates.is_empty() {
        return true;
    }

    candidates.iter().any(|candidate| contains_key(candidate, value))
}

#[cfg(test)]
mod tests {
    use rust_decimal::Decimal;

    use super::{
        ApprovalRoutingInput, ApprovalValidationFailure, ApprovalValidationInput,
        ApprovalValidator, ApproverAuthority, InMemoryCalendarAvailabilityClient, RoutingApprover,
        RoutingEngine, RoutingError, RoutingRule,
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

    fn routing_approvers() -> Vec<RoutingApprover> {
        vec![
            RoutingApprover {
                user_id: "u-rep".to_string(),
                role: "sales_rep".to_string(),
                role_rank: 1,
                manager_id: Some("u-mgr-1".to_string()),
                max_discount_pct: Decimal::new(500, 2),
                max_deal_value: Decimal::new(50_000, 2),
                allowed_account_tiers: vec!["smb".to_string()],
                allowed_product_categories: vec!["saas".to_string()],
            },
            RoutingApprover {
                user_id: "u-mgr-1".to_string(),
                role: "sales_manager".to_string(),
                role_rank: 2,
                manager_id: Some("u-vp".to_string()),
                max_discount_pct: Decimal::new(2000, 2),
                max_deal_value: Decimal::new(200_000, 2),
                allowed_account_tiers: vec!["enterprise".to_string(), "mid_market".to_string()],
                allowed_product_categories: vec!["saas".to_string(), "security".to_string()],
            },
            RoutingApprover {
                user_id: "u-mgr-2".to_string(),
                role: "sales_manager".to_string(),
                role_rank: 2,
                manager_id: Some("u-vp".to_string()),
                max_discount_pct: Decimal::new(2000, 2),
                max_deal_value: Decimal::new(220_000, 2),
                allowed_account_tiers: vec!["enterprise".to_string(), "mid_market".to_string()],
                allowed_product_categories: vec!["saas".to_string(), "security".to_string()],
            },
            RoutingApprover {
                user_id: "u-vp".to_string(),
                role: "vp_sales".to_string(),
                role_rank: 3,
                manager_id: None,
                max_discount_pct: Decimal::new(3000, 2),
                max_deal_value: Decimal::new(1_000_000, 2),
                allowed_account_tiers: vec!["*".to_string()],
                allowed_product_categories: vec!["*".to_string()],
            },
        ]
    }

    fn routing_rules() -> Vec<RoutingRule> {
        vec![
            RoutingRule {
                id: "rule-enterprise-security".to_string(),
                required_role: "sales_manager".to_string(),
                account_tier: Some("enterprise".to_string()),
                product_category: Some("security".to_string()),
                min_deal_value: Some(Decimal::new(100_000, 2)),
                min_discount_pct: Some(Decimal::new(1_000, 2)),
                priority: 10,
            },
            RoutingRule {
                id: "rule-default-manager".to_string(),
                required_role: "sales_manager".to_string(),
                account_tier: None,
                product_category: None,
                min_deal_value: None,
                min_discount_pct: None,
                priority: 100,
            },
        ]
    }

    fn routing_input() -> ApprovalRoutingInput {
        ApprovalRoutingInput {
            requester_user_id: "u-rep".to_string(),
            required_role: "sales_manager".to_string(),
            requested_discount_pct: Decimal::new(1_500, 2),
            deal_value: Decimal::new(150_000, 2),
            account_tier: "enterprise".to_string(),
            product_category: "security".to_string(),
        }
    }

    #[test]
    fn routing_prefers_requester_manager_as_primary_when_available() {
        let engine = RoutingEngine::new(
            routing_approvers(),
            routing_rules(),
            InMemoryCalendarAvailabilityClient::default(),
        );

        let decision = engine.route(&routing_input()).expect("routing should succeed");
        assert_eq!(decision.primary_approver_user_id, "u-mgr-1");
        assert_eq!(decision.selected_approver_user_id, "u-mgr-1");
        assert_eq!(decision.matched_rule_id.as_deref(), Some("rule-enterprise-security"));
        assert!(decision.escalation_reason.is_none());
    }

    #[test]
    fn routing_falls_back_to_same_rank_when_primary_is_unavailable() {
        let engine = RoutingEngine::new(
            routing_approvers(),
            routing_rules(),
            InMemoryCalendarAvailabilityClient::with_unavailable_users(vec!["u-mgr-1".to_string()]),
        );

        let decision = engine.route(&routing_input()).expect("routing should fallback");
        assert_eq!(decision.primary_approver_user_id, "u-mgr-1");
        assert_eq!(decision.selected_approver_user_id, "u-mgr-2");
        assert_eq!(
            decision.escalation_reason.as_deref(),
            Some("primary_unavailable_same_rank_fallback")
        );
    }

    #[test]
    fn routing_escalates_when_same_rank_approvers_are_unavailable() {
        let engine = RoutingEngine::new(
            routing_approvers(),
            routing_rules(),
            InMemoryCalendarAvailabilityClient::with_unavailable_users(vec![
                "u-mgr-1".to_string(),
                "u-mgr-2".to_string(),
            ]),
        );

        let decision = engine.route(&routing_input()).expect("routing should escalate");
        assert_eq!(decision.primary_approver_user_id, "u-mgr-1");
        assert_eq!(decision.selected_approver_user_id, "u-vp");
        assert_eq!(decision.escalation_reason.as_deref(), Some("primary_unavailable_escalated"));
    }

    #[test]
    fn routing_rejects_unknown_required_role() {
        let engine = RoutingEngine::new(
            routing_approvers(),
            routing_rules(),
            InMemoryCalendarAvailabilityClient::default(),
        );
        let mut input = routing_input();
        input.required_role = "chief_revenue_officer".to_string();

        let error = engine.route(&input).expect_err("unknown role should fail");
        assert_eq!(
            error,
            RoutingError::UnknownRequiredRole {
                required_role: "chief_revenue_officer".to_string(),
            }
        );
    }

    #[test]
    fn routing_rejects_when_no_approver_meets_constraints() {
        let engine = RoutingEngine::new(
            routing_approvers(),
            routing_rules(),
            InMemoryCalendarAvailabilityClient::default(),
        );
        let mut input = routing_input();
        input.requested_discount_pct = Decimal::new(3_500, 2);

        let error = engine.route(&input).expect_err("no approver should satisfy high discount");
        assert!(matches!(error, RoutingError::NoEligibleApprover { .. }));
    }
}
