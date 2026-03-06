//! Sales org hierarchy traversal for approval routing.
//!
//! Walks the `reports_to` chain on `SalesRep` to find the approval chain
//! and the first person with sufficient discount authority.

use crate::domain::sales_rep::{SalesRep, SalesRepId};

/// Maximum chain depth to prevent infinite loops from circular references.
pub const MAX_CHAIN_DEPTH: usize = 20;

/// Result of an approval chain lookup.
#[derive(Clone, Debug)]
pub struct ApprovalChain {
    /// The originating rep (first element) through the chain of managers.
    pub chain: Vec<SalesRep>,
}

/// Result of a discount authority lookup.
#[derive(Clone, Debug)]
pub struct DiscountAuthority {
    /// The rep who has sufficient authority, if found.
    pub authorizer: Option<SalesRep>,
    /// The full chain traversed to find them.
    pub chain: Vec<SalesRep>,
}

/// Build the approval chain by walking `reports_to` links.
///
/// `lookup` is a function that resolves a `SalesRepId` to a `SalesRep`.
/// Returns the chain from `start` upward (start is first element).
/// Stops at MAX_CHAIN_DEPTH or when `reports_to` is None / already visited.
pub fn find_approval_chain<F>(start: &SalesRep, lookup: F) -> ApprovalChain
where
    F: Fn(&SalesRepId) -> Option<SalesRep>,
{
    let mut chain = vec![start.clone()];
    let mut visited = std::collections::HashSet::new();
    visited.insert(start.id.0.clone());

    let mut current = start.clone();
    for _ in 0..MAX_CHAIN_DEPTH {
        match &current.reports_to {
            Some(manager_id) if !visited.contains(&manager_id.0) => {
                visited.insert(manager_id.0.clone());
                match lookup(manager_id) {
                    Some(manager) => {
                        chain.push(manager.clone());
                        current = manager;
                    }
                    None => break, // Manager ID references a non-existent rep
                }
            }
            _ => break, // No manager or circular reference detected
        }
    }

    ApprovalChain { chain }
}

/// Find the first person in the approval chain with `max_discount_pct >= discount_pct`.
///
/// Walks up from `start` using `lookup` and returns the first rep whose
/// `max_discount_pct` is at least `required_discount_pct`.
pub fn find_authority_for_discount<F>(
    start: &SalesRep,
    required_discount_pct: f64,
    lookup: F,
) -> DiscountAuthority
where
    F: Fn(&SalesRepId) -> Option<SalesRep>,
{
    let approval_chain = find_approval_chain(start, lookup);

    let authorizer = approval_chain
        .chain
        .iter()
        .find(|rep| rep.max_discount_pct.map(|pct| pct >= required_discount_pct).unwrap_or(false))
        .cloned();

    DiscountAuthority { authorizer, chain: approval_chain.chain }
}

#[cfg(test)]
mod tests {
    use chrono::Utc;

    use super::*;
    use crate::domain::sales_rep::{SalesRepRole, SalesRepStatus};

    fn make_rep(
        id: &str,
        role: SalesRepRole,
        reports_to: Option<&str>,
        max_discount_pct: Option<f64>,
    ) -> SalesRep {
        let now = Utc::now();
        SalesRep {
            id: SalesRepId(id.to_string()),
            external_user_ref: None,
            name: format!("Rep {id}"),
            email: None,
            role,
            title: None,
            team_id: None,
            reports_to: reports_to.map(|s| SalesRepId(s.to_string())),
            status: SalesRepStatus::Active,
            max_discount_pct,
            auto_approve_threshold_cents: None,
            capabilities_json: "[]".to_string(),
            config_json: "{}".to_string(),
            discount_budget_monthly_cents: 0,
            spent_discount_cents: 0,
            created_at: now,
            updated_at: now,
        }
    }

    fn org_lookup(reps: &[SalesRep]) -> impl Fn(&SalesRepId) -> Option<SalesRep> + '_ {
        move |id: &SalesRepId| reps.iter().find(|r| r.id == *id).cloned()
    }

    #[test]
    fn chain_with_no_manager_returns_just_self() {
        let ae = make_rep("ae1", SalesRepRole::Ae, None, Some(10.0));
        let result = find_approval_chain(&ae, |_| None);
        assert_eq!(result.chain.len(), 1);
        assert_eq!(result.chain[0].id.0, "ae1");
    }

    #[test]
    fn chain_follows_reports_to() {
        let cro = make_rep("cro1", SalesRepRole::Cro, None, Some(100.0));
        let vp = make_rep("vp1", SalesRepRole::Vp, Some("cro1"), Some(30.0));
        let mgr = make_rep("mgr1", SalesRepRole::Manager, Some("vp1"), Some(20.0));
        let ae = make_rep("ae1", SalesRepRole::Ae, Some("mgr1"), Some(10.0));

        let all = vec![ae.clone(), mgr, vp, cro];
        let result = find_approval_chain(&ae, org_lookup(&all));

        assert_eq!(result.chain.len(), 4);
        assert_eq!(result.chain[0].id.0, "ae1");
        assert_eq!(result.chain[1].id.0, "mgr1");
        assert_eq!(result.chain[2].id.0, "vp1");
        assert_eq!(result.chain[3].id.0, "cro1");
    }

    #[test]
    fn chain_detects_circular_reference() {
        let rep_a = make_rep("a", SalesRepRole::Ae, Some("b"), Some(10.0));
        let rep_b = make_rep("b", SalesRepRole::Manager, Some("a"), Some(20.0));

        let all = vec![rep_a.clone(), rep_b];
        let result = find_approval_chain(&rep_a, org_lookup(&all));

        // Should stop at 2 (a -> b, then b->a is circular, stops)
        assert_eq!(result.chain.len(), 2);
    }

    #[test]
    fn chain_handles_missing_manager() {
        let ae = make_rep("ae1", SalesRepRole::Ae, Some("ghost"), Some(10.0));
        let result = find_approval_chain(&ae, |_| None);

        assert_eq!(result.chain.len(), 1);
    }

    #[test]
    fn authority_found_in_chain() {
        let cro = make_rep("cro1", SalesRepRole::Cro, None, Some(100.0));
        let vp = make_rep("vp1", SalesRepRole::Vp, Some("cro1"), Some(30.0));
        let mgr = make_rep("mgr1", SalesRepRole::Manager, Some("vp1"), Some(20.0));
        let ae = make_rep("ae1", SalesRepRole::Ae, Some("mgr1"), Some(10.0));

        let all = vec![ae.clone(), mgr, vp, cro];

        // 15% discount — AE can't, Manager (20%) can
        let result = find_authority_for_discount(&ae, 15.0, org_lookup(&all));
        assert!(result.authorizer.is_some());
        assert_eq!(result.authorizer.unwrap().id.0, "mgr1");

        // 25% discount — Manager can't, VP (30%) can
        let result = find_authority_for_discount(&ae, 25.0, org_lookup(&all));
        assert!(result.authorizer.is_some());
        assert_eq!(result.authorizer.unwrap().id.0, "vp1");

        // 8% discount — AE (10%) can approve themselves
        let result = find_authority_for_discount(&ae, 8.0, org_lookup(&all));
        assert!(result.authorizer.is_some());
        assert_eq!(result.authorizer.unwrap().id.0, "ae1");
    }

    #[test]
    fn authority_not_found_when_nobody_has_enough() {
        let mgr = make_rep("mgr1", SalesRepRole::Manager, None, Some(20.0));
        let ae = make_rep("ae1", SalesRepRole::Ae, Some("mgr1"), Some(10.0));

        let all = vec![ae.clone(), mgr];

        // 50% discount — nobody in chain has authority
        let result = find_authority_for_discount(&ae, 50.0, org_lookup(&all));
        assert!(result.authorizer.is_none());
        assert_eq!(result.chain.len(), 2);
    }

    #[test]
    fn authority_skips_reps_with_no_discount_pct() {
        let cro = make_rep("cro1", SalesRepRole::Cro, None, Some(100.0));
        let mgr = make_rep("mgr1", SalesRepRole::Manager, Some("cro1"), None); // No max_discount_pct
        let ae = make_rep("ae1", SalesRepRole::Ae, Some("mgr1"), Some(10.0));

        let all = vec![ae.clone(), mgr, cro];

        // 15% discount — AE can't, Manager has None, CRO (100%) can
        let result = find_authority_for_discount(&ae, 15.0, org_lookup(&all));
        assert!(result.authorizer.is_some());
        assert_eq!(result.authorizer.unwrap().id.0, "cro1");
    }

    #[test]
    fn chain_caps_at_max_depth_for_long_hierarchies() {
        let mut reps = Vec::new();
        for idx in 0..=25 {
            let id = format!("rep-{idx}");
            let reports_to = if idx == 25 { None } else { Some(format!("rep-{}", idx + 1)) };
            reps.push(make_rep(
                &id,
                SalesRepRole::Ae,
                reports_to.as_deref(),
                Some(10.0 + idx as f64),
            ));
        }

        let start = reps.iter().find(|rep| rep.id.0 == "rep-0").cloned().expect("start rep exists");
        let result = find_approval_chain(&start, org_lookup(&reps));

        // Start node + 20 hops max from MAX_CHAIN_DEPTH.
        assert_eq!(result.chain.len(), 21);
        assert_eq!(result.chain.first().map(|rep| rep.id.0.as_str()), Some("rep-0"));
        assert_eq!(result.chain.last().map(|rep| rep.id.0.as_str()), Some("rep-20"));
    }
}
