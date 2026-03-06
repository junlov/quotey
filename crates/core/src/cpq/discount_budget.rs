//! Discount budget engine — 80%/100% threshold enforcement per rep.
//!
//! Each rep has a `discount_budget_monthly_cents` and `spent_discount_cents`.
//! - Under 80%: Ok
//! - 80%-99%: SoftWarn (advisory, still allowed)
//! - 100%+: HardLimit (requires manager approval)

use crate::domain::sales_rep::SalesRep;

/// Budget check result.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BudgetStatus {
    /// Under 80% utilization — discount is within budget.
    Ok { used_pct: u8, remaining_cents: i64 },
    /// 80%-99% utilization — soft warning.
    SoftWarn { used_pct: u8, remaining_cents: i64, message: String },
    /// 100%+ utilization — requires manager approval.
    HardLimit { used_pct: u8, overage_cents: i64, message: String },
}

const SOFT_WARN_THRESHOLD_PCT: u8 = 80;

/// Check whether a proposed discount fits within the rep's monthly budget.
///
/// If the rep has no budget configured (budget = 0), returns `Ok` with 0% used.
pub fn check_discount_budget(rep: &SalesRep, proposed_discount_cents: i64) -> BudgetStatus {
    let budget = rep.discount_budget_monthly_cents;

    // No budget configured — everything is allowed
    if budget <= 0 {
        return BudgetStatus::Ok { used_pct: 0, remaining_cents: 0 };
    }

    let total_after = rep.spent_discount_cents.saturating_add(proposed_discount_cents);
    let used_pct_raw = ((total_after as f64 / budget as f64) * 100.0).round() as u8;
    let used_pct = used_pct_raw.min(255);
    let remaining = (budget - total_after).max(0);

    if used_pct >= 100 {
        let overage = (total_after - budget).max(0);
        BudgetStatus::HardLimit {
            used_pct,
            overage_cents: overage,
            message: format!(
                "Discount budget exhausted ({}% used). Requires manager approval for further discounts.",
                used_pct
            ),
        }
    } else if used_pct >= SOFT_WARN_THRESHOLD_PCT {
        BudgetStatus::SoftWarn {
            used_pct,
            remaining_cents: remaining,
            message: format!(
                "You have used {}% of your monthly discount authority. {} cents remaining.",
                used_pct, remaining
            ),
        }
    } else {
        BudgetStatus::Ok { used_pct, remaining_cents: remaining }
    }
}

/// Budget summary for a rep (used by MCP tools and dashboards).
#[derive(Debug, Clone)]
pub struct BudgetSummary {
    pub rep_id: String,
    pub rep_name: String,
    pub budget_monthly_cents: i64,
    pub spent_cents: i64,
    pub remaining_cents: i64,
    pub used_pct: u8,
    pub status: String,
}

/// Build a budget summary from a SalesRep.
pub fn budget_summary(rep: &SalesRep) -> BudgetSummary {
    let budget = rep.discount_budget_monthly_cents;
    let spent = rep.spent_discount_cents;
    let remaining = (budget - spent).max(0);
    let used_pct =
        if budget > 0 { ((spent as f64 / budget as f64) * 100.0).round() as u8 } else { 0 };

    let status = if budget <= 0 {
        "no_budget"
    } else if used_pct >= 100 {
        "hard_limit"
    } else if used_pct >= SOFT_WARN_THRESHOLD_PCT {
        "soft_warn"
    } else {
        "ok"
    };

    BudgetSummary {
        rep_id: rep.id.0.clone(),
        rep_name: rep.name.clone(),
        budget_monthly_cents: budget,
        spent_cents: spent,
        remaining_cents: remaining,
        used_pct,
        status: status.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use chrono::Utc;

    use super::*;
    use crate::domain::sales_rep::{SalesRep, SalesRepId, SalesRepRole, SalesRepStatus};

    fn test_rep(budget: i64, spent: i64) -> SalesRep {
        SalesRep {
            id: SalesRepId("REP-001".to_string()),
            external_user_ref: None,
            name: "Test Rep".to_string(),
            email: None,
            role: SalesRepRole::Ae,
            title: None,
            team_id: None,
            reports_to: None,
            status: SalesRepStatus::Active,
            max_discount_pct: Some(15.0),
            auto_approve_threshold_cents: None,
            capabilities_json: "{}".to_string(),
            config_json: "{}".to_string(),
            discount_budget_monthly_cents: budget,
            spent_discount_cents: spent,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }

    #[test]
    fn ok_when_under_80_pct() {
        let rep = test_rep(10000, 5000); // 50% used
        let result = check_discount_budget(&rep, 1000); // would be 60%
        match result {
            BudgetStatus::Ok { used_pct, remaining_cents } => {
                assert_eq!(used_pct, 60);
                assert_eq!(remaining_cents, 4000);
            }
            other => panic!("expected Ok, got {:?}", other),
        }
    }

    #[test]
    fn soft_warn_at_80_pct() {
        let rep = test_rep(10000, 7000); // 70% used
        let result = check_discount_budget(&rep, 1500); // would be 85%
        match result {
            BudgetStatus::SoftWarn { used_pct, remaining_cents, .. } => {
                assert_eq!(used_pct, 85);
                assert_eq!(remaining_cents, 1500);
            }
            other => panic!("expected SoftWarn, got {:?}", other),
        }
    }

    #[test]
    fn hard_limit_at_100_pct() {
        let rep = test_rep(10000, 9000); // 90% used
        let result = check_discount_budget(&rep, 2000); // would be 110%
        match result {
            BudgetStatus::HardLimit { used_pct, overage_cents, .. } => {
                assert_eq!(used_pct, 110);
                assert_eq!(overage_cents, 1000);
            }
            other => panic!("expected HardLimit, got {:?}", other),
        }
    }

    #[test]
    fn no_budget_returns_ok() {
        let rep = test_rep(0, 0);
        let result = check_discount_budget(&rep, 5000);
        match result {
            BudgetStatus::Ok { used_pct, .. } => assert_eq!(used_pct, 0),
            other => panic!("expected Ok (no budget), got {:?}", other),
        }
    }

    #[test]
    fn exact_80_pct_is_soft_warn() {
        let rep = test_rep(10000, 0);
        let result = check_discount_budget(&rep, 8000); // exactly 80%
        assert!(matches!(result, BudgetStatus::SoftWarn { .. }));
    }

    #[test]
    fn exact_100_pct_is_hard_limit() {
        let rep = test_rep(10000, 0);
        let result = check_discount_budget(&rep, 10000); // exactly 100%
        assert!(matches!(result, BudgetStatus::HardLimit { .. }));
    }

    #[test]
    fn budget_summary_formats_correctly() {
        let rep = test_rep(20000, 15000);
        let summary = budget_summary(&rep);
        assert_eq!(summary.budget_monthly_cents, 20000);
        assert_eq!(summary.spent_cents, 15000);
        assert_eq!(summary.remaining_cents, 5000);
        assert_eq!(summary.used_pct, 75);
        assert_eq!(summary.status, "ok");
    }

    #[test]
    fn budget_summary_hard_limit_status() {
        let rep = test_rep(10000, 12000);
        let summary = budget_summary(&rep);
        assert_eq!(summary.status, "hard_limit");
        assert_eq!(summary.remaining_cents, 0);
    }
}
