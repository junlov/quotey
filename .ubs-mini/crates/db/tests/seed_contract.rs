use serde::Deserialize;
use serde_json::Value;
use std::collections::HashSet;

#[derive(Debug, Deserialize)]
struct SeedFlowContract {
    flow_type: String,
    quote_id: String,
    status: String,
    current_step: String,
    step_number: u8,
    account_id: String,
    deal_id: String,
    policy_profile: String,
    product_ids: Vec<String>,
    required_fields: Vec<String>,
    missing_fields: Vec<String>,
    expected_line_count: u32,
    #[serde(default)]
    account_name: Option<String>,
    #[serde(default)]
    deal_name: Option<String>,
    #[serde(default)]
    product_names: Option<Vec<String>>,
    #[serde(default)]
    requested_discount_pct: Option<u8>,
    #[serde(default)]
    threshold_pct: Option<u8>,
    expected_transition_checkpoints: Vec<String>,
    expected_audit_events: Vec<String>,
    prior_quote_id: Option<String>,
}

#[derive(Debug, Deserialize)]
struct DiscountMatrixRow {
    flow_type: String,
    account_tier: String,
    requested_discount_pct: u8,
    threshold_pct: u8,
    expected_routing: String,
    approval_required: bool,
}

#[derive(Debug, Deserialize)]
struct SeedContract {
    dataset_version: String,
    seed_dataset: String,
    flows: Vec<SeedFlowContract>,
    discount_threshold_matrix: Vec<DiscountMatrixRow>,
}

type SeedContractTestResult = Result<(), String>;

fn require_array<'a>(value: &'a Value, field: &'static str) -> SeedContractTestResult<&'a [Value]> {
    value
        .get(field)
        .and_then(Value::as_array)
        .map(|values| values.as_slice())
        .ok_or_else(|| format!("field '{field}' should be an array"))
}

fn require_flow<'a>(
    flows: &'a [SeedFlowContract],
    flow_type: &'static str,
) -> SeedContractTestResult<&'a SeedFlowContract> {
    flows
        .iter()
        .find(|flow| flow.flow_type == flow_type)
        .ok_or_else(|| format!("missing canonical flow '{flow_type}'"))
}

#[test]
fn seed_contract_matches_e2e_seed_sql_fixture() -> SeedContractTestResult {
    let fixture_sql = include_str!("../../../config/fixtures/e2e_seed_data.sql");
    let contract: SeedContract =
        serde_json::from_str(include_str!("../../../config/fixtures/e2e_seed_contract.json"))
            .map_err(|error| format!("seed contract JSON must parse: {error}"))?;
    let mut flow_types_seen = HashSet::new();

    assert_eq!(contract.dataset_version, "bd-3vp2.7.2");
    assert_eq!(contract.seed_dataset, "deterministic_e2e_core_flows");
    assert_eq!(contract.flows.len(), 3);

    for flow in &contract.flows {
        assert!(
            flow_types_seen.insert(flow.flow_type.clone()),
            "duplicate flow type: {}",
            flow.flow_type
        );
        assert!(!flow.quote_id.is_empty());
        assert!(!flow.status.is_empty());
        assert!(!flow.current_step.is_empty());
        assert!(flow.expected_line_count > 0);
        assert!(flow.step_number >= 1);
        assert!(!flow.account_id.is_empty());
        assert!(!flow.deal_id.is_empty());
        assert!(!flow.policy_profile.is_empty());
        assert!(!flow.product_ids.is_empty());
        if let Some(account_name) = &flow.account_name {
            assert!(!account_name.is_empty());
        }
        if let Some(deal_name) = &flow.deal_name {
            assert!(!deal_name.is_empty());
        }
        if let Some(product_names) = &flow.product_names {
            assert!(!product_names.is_empty());
        }
        assert!(!flow.expected_transition_checkpoints.is_empty());
        assert!(!flow.expected_audit_events.is_empty());
        assert!(!flow.missing_fields.is_empty() || flow.flow_type != "discount_exception");

        if let (Some(requested), Some(threshold)) =
            (flow.requested_discount_pct, flow.threshold_pct)
        {
            assert!(
                requested >= 1,
                "requested discount should be positive for {}, got {}",
                flow.flow_type,
                requested
            );
            assert!(
                threshold >= 1,
                "threshold should be positive for {}, got {}",
                flow.flow_type,
                threshold
            );
        }

        assert!(
            fixture_sql.contains(&format!("'{}'", flow.quote_id)),
            "seed SQL fixture should include quote id {}",
            flow.quote_id
        );

        for required_field in &flow.required_fields {
            assert!(
                fixture_sql.contains(required_field),
                "seed SQL fixture should include required field {} for {}",
                required_field,
                flow.flow_type
            );
        }

        if let Some(prior_quote_id) = &flow.prior_quote_id {
            assert!(
                fixture_sql.contains(&format!("\"prior_quote_id\":\"{}\"", prior_quote_id)),
                "seed SQL fixture should include metadata prior quote {} for {}",
                prior_quote_id,
                flow.flow_type
            );
        }

        assert!(
            fixture_sql.contains(&format!("\"account_id\":\"{}\"", flow.account_id)),
            "seed SQL fixture should include account id {} for {}",
            flow.account_id,
            flow.flow_type
        );
        if let Some(account_name) = &flow.account_name {
            assert!(
                fixture_sql.contains(&format!("\"account_name\":\"{}\"", account_name)),
                "seed SQL fixture should include account name {} for {}",
                account_name,
                flow.flow_type
            );
        }
        if let Some(deal_name) = &flow.deal_name {
            assert!(
                fixture_sql.contains(&format!("\"deal_name\":\"{}\"", deal_name)),
                "seed SQL fixture should include deal name {} for {}",
                deal_name,
                flow.flow_type
            );
        }
        if let Some(product_names) = &flow.product_names {
            for product_name in product_names {
                assert!(
                    fixture_sql.contains(&format!("\"{}\"", product_name)),
                    "seed SQL fixture should include product name {} for {}",
                    product_name,
                    flow.flow_type
                );
            }
        }

        if flow.flow_type == "renewal" {
            let prior_quote_id = flow
                .prior_quote_id
                .as_ref()
                .ok_or_else(|| "renewal flow should include prior quote id".to_string())?;
            assert!(
                fixture_sql.contains(&format!("'{}', 'sent'", prior_quote_id)),
                "renewal prior quote should be seeded with sent status"
            );
            assert!(
                fixture_sql.contains("'ql-renewal-prior-001-1'"),
                "renewal prior quote should include deterministic line 1"
            );
            assert!(
                fixture_sql.contains("'ql-renewal-prior-001-2'"),
                "renewal prior quote should include deterministic line 2"
            );
        }

        if flow.flow_type == "discount_exception" {
            assert!(
                fixture_sql.contains("approval.requested"),
                "discount flow should include approval request audit event"
            );
        }
    }

    for expected_flow in ["net_new", "renewal", "discount_exception"] {
        assert!(flow_types_seen.contains(expected_flow), "missing canonical flow: {expected_flow}");
    }
    Ok(())
}

#[test]
fn discount_threshold_matrix_is_consistent() -> SeedContractTestResult {
    let contract: SeedContract =
        serde_json::from_str(include_str!("../../../config/fixtures/e2e_seed_contract.json"))
            .map_err(|error| format!("seed contract JSON must parse: {error}"))?;
    let mut seen_policy_points: HashSet<(String, u8)> = HashSet::new();
    let mut account_tiers_seen: HashSet<String> = HashSet::new();
    let mut auto_approve_count = 0usize;
    let mut managed_approval_count = 0usize;
    let mut total_rows = 0usize;

    for row in &contract.discount_threshold_matrix {
        total_rows += 1;
        assert!(
            seen_policy_points.insert((row.account_tier.clone(), row.requested_discount_pct)),
            "duplicate discount-policy row detected for tier '{}' at requested discount {}",
            row.account_tier,
            row.requested_discount_pct
        );

        assert_eq!(row.flow_type, "discount_exception");
        assert!(!row.account_tier.is_empty());
        account_tiers_seen.insert(row.account_tier.clone());
        assert!(!row.expected_routing.is_empty());
        assert!(row.requested_discount_pct > 0);
        assert!(row.threshold_pct > 0);
        let requires_approval = row.requested_discount_pct as i16 >= row.threshold_pct as i16;
        if row.approval_required {
            assert!(
                row.expected_routing.contains("approval"),
                "approval-required matrix rows should encode approval routing explicitly for {} (got '{}')",
                row.account_tier,
                row.expected_routing
            );
            managed_approval_count += 1;
        } else {
            assert_eq!(row.expected_routing, "auto_approve");
            auto_approve_count += 1;
        }
        assert_eq!(
            row.approval_required,
            requires_approval,
            "routing must align with requested vs threshold boundary for {}: requested={} threshold={}",
            row.account_tier,
            row.requested_discount_pct,
            row.threshold_pct
        );
    }

    assert!(
        contract.discount_threshold_matrix.len() >= 3,
        "discount threshold matrix should include multiple policy points"
    );
    assert_eq!(total_rows, contract.discount_threshold_matrix.len());
    assert!(
        account_tiers_seen.len() >= 2,
        "discount threshold matrix should cover at least two account tiers"
    );
    assert!(
        auto_approve_count >= 1,
        "discount threshold matrix should include at least one auto-approve policy point"
    );
    assert!(
        managed_approval_count >= 1,
        "discount threshold matrix should include at least one approval-required policy point"
    );
    Ok(())
}

#[test]
fn per_flow_contracts_derive_from_seed_contract() -> SeedContractTestResult {
    let seed_contract: SeedContract =
        serde_json::from_str(include_str!("../../../config/fixtures/e2e_seed_contract.json"))
            .map_err(|error| format!("seed contract JSON must parse: {error}"))?;

    let netnew: serde_json::Value = serde_json::from_str(include_str!(
        "../../../config/fixtures/e2e_netnew_flow_contract.json"
    ))
    .map_err(|error| format!("net-new flow contract JSON must parse: {error}"))?;

    let renewal: serde_json::Value = serde_json::from_str(include_str!(
        "../../../config/fixtures/e2e_renewal_flow_contract.json"
    ))
    .map_err(|error| format!("renewal flow contract JSON must parse: {error}"))?;

    let discount: serde_json::Value = serde_json::from_str(include_str!(
        "../../../config/fixtures/e2e_discount_exception_flow_contract.json"
    ))
    .map_err(|error| format!("discount exception flow contract JSON must parse: {error}"))?;

    assert_eq!(
        netnew["dataset_version"].as_str().unwrap_or_default(),
        seed_contract.dataset_version
    );
    assert_eq!(
        renewal["dataset_version"].as_str().unwrap_or_default(),
        seed_contract.dataset_version
    );
    assert_eq!(
        discount["dataset_version"].as_str().unwrap_or_default(),
        seed_contract.dataset_version
    );

    let netnew_seed = require_flow(&seed_contract.flows, "net_new")?;
    let renewal_seed = require_flow(&seed_contract.flows, "renewal")?;
    let discount_seed = require_flow(&seed_contract.flows, "discount_exception")?;

    let assert_seed_fields_match = |contract: &serde_json::Value, flow: &SeedFlowContract| -> SeedContractTestResult {
        assert_eq!(contract["quote_id"].as_str(), Some(flow.quote_id.as_str()));
        assert_eq!(contract["account_id"].as_str(), Some(flow.account_id.as_str()));
        assert_eq!(contract["deal_id"].as_str(), Some(flow.deal_id.as_str()));
        assert_eq!(contract["policy_profile"].as_str(), Some(flow.policy_profile.as_str()));
        assert_eq!(contract["status"].as_str(), Some(flow.status.as_str()));
        assert_eq!(contract["current_step"].as_str(), Some(flow.current_step.as_str()));
        assert_eq!(
            contract["step_number"].as_u64().unwrap_or_default(),
            u64::from(flow.step_number)
        );
        assert_eq!(
            contract["expected_line_count"].as_u64().unwrap_or_default(),
            u64::from(flow.expected_line_count)
        );
        assert_eq!(
            require_array(contract, "expected_transition_checkpoints")?
                .iter()
                .map(|v| v.as_str().unwrap_or_default().to_owned())
                .collect::<Vec<_>>(),
            flow.expected_transition_checkpoints
                .iter()
                .map(std::string::ToString::to_string)
                .collect::<Vec<_>>()
        );
        assert_eq!(
            require_array(contract, "expected_audit_events")?
                .iter()
                .map(|v| v.as_str().unwrap_or_default().to_owned())
                .collect::<Vec<_>>(),
            flow.expected_audit_events
                .iter()
                .map(std::string::ToString::to_string)
                .collect::<Vec<_>>()
        );
        assert_eq!(
            require_array(contract, "required_fields")?
                .iter()
                .map(|v| v.as_str().unwrap_or_default().to_owned())
                .collect::<Vec<_>>(),
            flow.required_fields.iter().map(std::string::ToString::to_string).collect::<Vec<_>>()
        );
        assert_eq!(
            require_array(contract, "missing_fields")?
                .iter()
                .map(|v| v.as_str().unwrap_or_default().to_owned())
                .collect::<Vec<_>>(),
            flow.missing_fields.iter().map(std::string::ToString::to_string).collect::<Vec<_>>()
        );
        if let Some(account_name) = &flow.account_name {
            assert_eq!(contract["account_name"].as_str(), Some(account_name.as_str()));
        }
        if let Some(deal_name) = &flow.deal_name {
            assert_eq!(contract["deal_name"].as_str(), Some(deal_name.as_str()));
        }
        assert_eq!(
            require_array(contract, "product_ids")?
                .iter()
                .map(|v| v.as_str().unwrap_or_default().to_owned())
                .collect::<Vec<_>>(),
            flow.product_ids.iter().map(std::string::ToString::to_string).collect::<Vec<_>>()
        );
        match (&flow.product_names, contract.get("product_names").and_then(Value::as_array)) {
            (Some(names), Some(contract_names)) => {
                assert_eq!(
                    contract_names
                        .iter()
                        .map(|v| v.as_str().unwrap_or_default().to_owned())
                        .collect::<Vec<_>>(),
                    names.iter().map(std::string::ToString::to_string).collect::<Vec<_>>()
                );
            }
            (Some(_), None) => {
                return Err(
                    "product_names presence must match canonical contract definition".to_string()
                );
            }
            (None, Some(contract_names))
                if !contract_names.is_empty() && !contract_names.iter().all(Value::is_null) =>
            {
                return Err(
                    "product_names presence must match canonical contract definition".to_string()
                );
            }
            _ => {}
        }
        Ok(())
    };

    assert_eq!(netnew["flow_type"].as_str(), Some("net_new"));
    assert_eq!(netnew["seed_quote_id"].as_str(), Some(netnew_seed.quote_id.as_str()));
    assert_eq!(netnew["child_bead"].as_str(), Some("bd-3vp2.8.1.1"));
    assert_seed_fields_match(&netnew["seed_contract"], netnew_seed)?;

    assert_eq!(renewal["flow_type"].as_str(), Some("renewal"));
    assert_eq!(renewal["seed_quote_id"].as_str(), Some(renewal_seed.quote_id.as_str()));
    assert_eq!(renewal["child_bead"].as_str(), Some("bd-3vp2.8.2.1"));
    assert_seed_fields_match(&renewal["seed_contract"], renewal_seed)?;
    assert_eq!(
        renewal["seed_contract"]["prior_quote_id"].as_str(),
        Some("quote-renewal-prior-001")
    );
    assert_eq!(renewal["flow_type"].as_str(), Some(renewal_seed.flow_type.as_str()));

    assert_eq!(discount["flow_type"].as_str(), Some("discount_exception"));
    assert_eq!(discount["seed_quote_id"].as_str(), Some(discount_seed.quote_id.as_str()));
    assert_eq!(discount["child_bead"].as_str(), Some("bd-3vp2.8.3.1"));
    assert_seed_fields_match(&discount["seed_contract"], discount_seed)?;
    assert_eq!(
        discount["seed_contract"]["requested_discount_pct"].as_u64().unwrap_or_default(),
        u64::from(discount_seed.requested_discount_pct.unwrap_or(0))
    );
    assert_eq!(
        discount["seed_contract"]["threshold_pct"].as_u64().unwrap_or_default(),
        u64::from(discount_seed.threshold_pct.unwrap_or(0))
    );
    if let Some(seed_prior) = &discount_seed.prior_quote_id {
        assert_eq!(discount["seed_contract"]["prior_quote_id"].as_str(), Some(seed_prior.as_str()));
    } else {
        assert!(discount["seed_contract"]["prior_quote_id"].is_null());
    }
    Ok(())
}

#[test]
fn resilience_fault_contract_shape_is_deterministic() -> SeedContractTestResult {
    let resilience_contract: serde_json::Value = serde_json::from_str(include_str!(
        "../../../config/fixtures/e2e_resilience_fault_contract.json"
    ))
    .map_err(|error| format!("resilience fault contract JSON must parse: {error}"))?;

    assert_eq!(resilience_contract["dataset_version"].as_str(), Some("bd-3vp2.7.2"));
    assert_eq!(resilience_contract["child_bead"].as_str(), Some("bd-3vp2.8.4.1"));
    assert_eq!(resilience_contract["scenario_group"].as_str(), Some("resilience_fault_matrix"));

    let phase_fault_matrix = resilience_contract["phase_fault_matrix"]
        .as_array()
        .ok_or_else(|| "phase_fault_matrix should be an array".to_string())?;
    assert_eq!(phase_fault_matrix.len(), 4);

    let phases = phase_fault_matrix
        .iter()
        .map(|entry| entry["phase"].as_str().unwrap_or_default())
        .collect::<Vec<_>>();
    assert_eq!(
        phases,
        vec!["seed_load", "policy_evaluation", "pricing_evaluation", "approval_routing"]
    );

    for entry in phase_fault_matrix {
        let phase = entry["phase"]
            .as_str()
            .ok_or_else(|| "phase should be present".to_string())?;
        let fault_domain = entry["fault_domain"]
            .as_str()
            .ok_or_else(|| "fault_domain should be present".to_string())?;
        let injected_error =
            entry["injected_error"]
                .as_str()
                .ok_or_else(|| "injected_error should be present".to_string())?;
        let expected_outcome =
            entry["expected_outcome"]
                .as_str()
                .ok_or_else(|| "expected_outcome should be present".to_string())?;
        let expected_retry_window = entry["expected_retry_window_sec"].as_array();
        let expected_retryable = entry["retryable"].as_bool().unwrap_or(false);
        let max_retries = entry["max_retries"].as_u64().unwrap_or(0);

        assert!(!fault_domain.is_empty(), "fault_domain should not be empty for phase {}", phase);
        assert!(
            !injected_error.is_empty(),
            "injected_error should not be empty for phase {}",
            phase
        );
        assert!(
            !expected_outcome.is_empty(),
            "expected_outcome should not be empty for phase {}",
            phase
        );
        assert!(
            matches!(
                expected_outcome,
                "recover_and_continue" | "recover_and_retry" | "fail_fast" | "retry_with_backoff"
            ),
            "unexpected expected_outcome {} for phase {}",
            expected_outcome,
            phase
        );

        if expected_retryable {
            assert!(expected_retry_window.is_some());
            let window = expected_retry_window.ok_or_else(|| {
                "expected_retry_window_sec should be present when retryable is true".to_string()
            })?;
            assert!(!window.is_empty());
            assert!(max_retries > 0);
            for window_value in window {
                let seconds = window_value.as_u64().ok_or_else(|| {
                    "expected_retry_window_sec entries should be positive integers".to_string()
                })?;
                assert!(seconds > 0, "retry window should be strictly positive");
            }
        } else {
            assert_eq!(expected_outcome, "fail_fast");
            assert_eq!(max_retries, 0);
            assert!(expected_retry_window.is_none());
        }
        assert!(!phase.is_empty());
    }
    Ok(())
}

#[test]
fn resilience_fault_contract_phase_rules_are_deterministic() -> SeedContractTestResult {
    let resilience_contract: serde_json::Value = serde_json::from_str(include_str!(
        "../../../config/fixtures/e2e_resilience_fault_contract.json"
    ))
    .map_err(|error| format!("resilience fault contract JSON must parse: {error}"))?;

    let phase_fault_matrix = resilience_contract["phase_fault_matrix"]
        .as_array()
        .ok_or_else(|| "phase_fault_matrix should be an array".to_string())?;
    assert_eq!(phase_fault_matrix.len(), 4);

    for entry in phase_fault_matrix {
        let phase = entry["phase"].as_str().ok_or_else(|| "phase should be present".to_string())?;
        let expected_outcome =
            entry["expected_outcome"]
                .as_str()
                .ok_or_else(|| "expected_outcome should be present".to_string())?;
        let expected_retryable = entry["retryable"].as_bool().unwrap_or(false);
        let expected_retry_window = entry["expected_retry_window_sec"].as_array();
        let max_retries = entry["max_retries"].as_u64().unwrap_or(0);

        match phase {
            "seed_load" => {
                assert_eq!(expected_outcome, "recover_and_continue");
                assert!(expected_retryable);
                assert_eq!(max_retries, 3);
                assert!(expected_retry_window.is_some());
            }
            "policy_evaluation" => {
                assert_eq!(expected_outcome, "recover_and_retry");
                assert!(expected_retryable);
                assert_eq!(max_retries, 2);
                assert!(expected_retry_window.is_some());
            }
            "pricing_evaluation" => {
                assert_eq!(expected_outcome, "fail_fast");
                assert!(!expected_retryable);
                assert_eq!(max_retries, 0);
                assert!(expected_retry_window.is_none());
            }
            "approval_routing" => {
                assert_eq!(expected_outcome, "retry_with_backoff");
                assert!(expected_retryable);
                assert_eq!(max_retries, 2);
                assert!(expected_retry_window.is_some());
            }
            _ => {}
        }
    }
    Ok(())
}

#[test]
fn netnew_flow_contract_is_self_consistent() -> SeedContractTestResult {
    let netnew: serde_json::Value = serde_json::from_str(include_str!(
        "../../../config/fixtures/e2e_netnew_flow_contract.json"
    ))
    .map_err(|error| format!("net-new flow contract JSON must parse: {error}"))?;

    assert_eq!(netnew["dataset_version"].as_str(), Some("bd-3vp2.7.2"));
    assert_eq!(netnew["flow_type"].as_str(), Some("net_new"));
    assert_eq!(netnew["child_bead"].as_str(), Some("bd-3vp2.8.1.1"));
    let contract = &netnew["seed_contract"];

    assert_eq!(contract["quote_id"].as_str(), Some("quote-netnew-001"));
    assert_eq!(contract["account_id"].as_str(), Some("acct-netnew-001"));
    assert_eq!(contract["deal_id"].as_str(), Some("deal-netnew-001"));
    assert_eq!(contract["account_name"].as_str(), Some("Acme Corp"));
    assert_eq!(contract["deal_name"].as_str(), Some("Acme Corp - New License"));
    assert_eq!(contract["policy_profile"].as_str(), Some("standard"));
    assert_eq!(contract["status"].as_str(), Some("draft"));
    assert_eq!(contract["current_step"].as_str(), Some("gather_requirements"));
    assert_eq!(contract["step_number"].as_u64().unwrap_or_default(), 1);
    assert_eq!(contract["expected_line_count"].as_u64().unwrap_or_default(), 3);

    assert_eq!(
        contract["required_fields"]
            .as_array()
            .ok_or_else(|| "required_fields should be present".to_string())?
            .iter()
            .map(|v| v.as_str())
            .collect::<Vec<_>>(),
        vec![Some("billing_country"), Some("payment_terms")]
    );
    assert_eq!(
        contract["missing_fields"]
            .as_array()
            .ok_or_else(|| "missing_fields should be present".to_string())?
            .iter()
            .map(|v| v.as_str())
            .collect::<Vec<_>>(),
        vec![Some("billing_country"), Some("payment_terms")]
    );

    assert_eq!(
        contract["product_ids"]
            .as_array()
            .ok_or_else(|| "product_ids should be present".to_string())?
            .iter()
            .map(|v| v.as_str())
            .collect::<Vec<_>>(),
        vec![Some("prod-plan-ent"), Some("prod-sso"), Some("prod-support-premium")]
    );
    assert_eq!(
        contract["product_names"]
            .as_array()
            .ok_or_else(|| "product_names should be present".to_string())?
            .iter()
            .map(|v| v.as_str())
            .collect::<Vec<_>>(),
        vec![Some("Enterprise Plan"), Some("SSO Add-on"), Some("Premium Support")]
    );
    assert!(contract["requested_discount_pct"].is_null());
    assert!(contract["threshold_pct"].is_null());
    assert!(contract["prior_quote_id"].is_null());
    Ok(())
}

#[test]
fn renewal_flow_contract_is_self_consistent() -> SeedContractTestResult {
    let renewal: serde_json::Value = serde_json::from_str(include_str!(
        "../../../config/fixtures/e2e_renewal_flow_contract.json"
    ))
    .map_err(|error| format!("renewal flow contract JSON must parse: {error}"))?;

    assert_eq!(renewal["dataset_version"].as_str(), Some("bd-3vp2.7.2"));
    assert_eq!(renewal["flow_type"].as_str(), Some("renewal"));
    assert_eq!(renewal["child_bead"].as_str(), Some("bd-3vp2.8.2.1"));
    let contract = &renewal["seed_contract"];

    assert_eq!(contract["quote_id"].as_str(), Some("quote-renewal-001"));
    assert_eq!(contract["account_id"].as_str(), Some("acct-renewal-001"));
    assert_eq!(contract["deal_id"].as_str(), Some("deal-renewal-001"));
    assert_eq!(contract["account_name"].as_str(), Some("Globex Industries"));
    assert_eq!(contract["deal_name"].as_str(), Some("Globex - Annual Renewal"));
    assert_eq!(contract["policy_profile"].as_str(), Some("renewal"));
    assert_eq!(contract["status"].as_str(), Some("priced"));
    assert_eq!(contract["current_step"].as_str(), Some("validate_expansion"));
    assert_eq!(contract["step_number"].as_u64().unwrap_or_default(), 3);
    assert_eq!(contract["expected_line_count"].as_u64().unwrap_or_default(), 3);

    assert_eq!(
        contract["required_fields"]
            .as_array()
            .ok_or_else(|| "required_fields should be present".to_string())?
            .iter()
            .map(|v| v.as_str())
            .collect::<Vec<_>>(),
        vec![Some("prior_quote_id")]
    );
    assert_eq!(
        contract["missing_fields"]
            .as_array()
            .ok_or_else(|| "missing_fields should be present".to_string())?
            .iter()
            .map(|v| v.as_str())
            .collect::<Vec<_>>(),
        Vec::<Option<&str>>::new()
    );
    assert_eq!(
        contract["product_ids"]
            .as_array()
            .ok_or_else(|| "product_ids should be present".to_string())?
            .iter()
            .map(|v| v.as_str())
            .collect::<Vec<_>>(),
        vec![Some("prod-plan-ent"), Some("prod-support-premium"), Some("prod-onboarding")]
    );
    assert_eq!(
        contract["product_names"]
            .as_array()
            .ok_or_else(|| "product_names should be present".to_string())?
            .iter()
            .map(|v| v.as_str())
            .collect::<Vec<_>>(),
        vec![Some("Enterprise Plan"), Some("Premium Support"), Some("Onboarding")]
    );
    assert_eq!(contract["prior_quote_id"].as_str(), Some("quote-renewal-prior-001"));
    assert!(contract["requested_discount_pct"].is_null());
    assert!(contract["threshold_pct"].is_null());
    Ok(())
}

#[test]
fn discount_flow_contract_is_self_consistent() -> SeedContractTestResult {
    let discount: serde_json::Value = serde_json::from_str(include_str!(
        "../../../config/fixtures/e2e_discount_exception_flow_contract.json"
    ))
    .map_err(|error| format!("discount exception flow contract JSON must parse: {error}"))?;

    assert_eq!(discount["dataset_version"].as_str(), Some("bd-3vp2.7.2"));
    assert_eq!(discount["flow_type"].as_str(), Some("discount_exception"));
    assert_eq!(discount["child_bead"].as_str(), Some("bd-3vp2.8.3.1"));
    let contract = &discount["seed_contract"];

    assert_eq!(contract["quote_id"].as_str(), Some("quote-discount-001"));
    assert_eq!(contract["account_id"].as_str(), Some("acct-discount-001"));
    assert_eq!(contract["deal_id"].as_str(), Some("deal-discount-001"));
    assert_eq!(contract["account_name"].as_str(), Some("Initech LLC"));
    assert_eq!(contract["deal_name"].as_str(), Some("Initech - Expansion Deal"));
    assert_eq!(contract["policy_profile"].as_str(), Some("discount_exception"));
    assert_eq!(contract["status"].as_str(), Some("approval"));
    assert_eq!(contract["current_step"].as_str(), Some("awaiting_approval"));
    assert_eq!(contract["step_number"].as_u64().unwrap_or_default(), 4);
    assert_eq!(contract["expected_line_count"].as_u64().unwrap_or_default(), 2);
    assert_eq!(
        contract["required_fields"]
            .as_array()
            .ok_or_else(|| "required_fields should be present".to_string())?
            .iter()
            .map(|v| v.as_str())
            .collect::<Vec<_>>(),
        vec![Some("approval_decision")]
    );
    assert_eq!(
        contract["missing_fields"]
            .as_array()
            .ok_or_else(|| "missing_fields should be present".to_string())?
            .iter()
            .map(|v| v.as_str())
            .collect::<Vec<_>>(),
        vec![Some("approval_decision")]
    );
    assert_eq!(
        contract["product_ids"]
            .as_array()
            .ok_or_else(|| "product_ids should be present".to_string())?
            .iter()
            .map(|v| v.as_str())
            .collect::<Vec<_>>(),
        vec![Some("prod-plan-pro"), Some("prod-sso")]
    );
    assert_eq!(
        contract["product_names"]
            .as_array()
            .ok_or_else(|| "product_names should be present".to_string())?
            .iter()
            .map(|v| v.as_str())
            .collect::<Vec<_>>(),
        vec![Some("Pro Plan"), Some("SSO Add-on")]
    );
    assert_eq!(contract["requested_discount_pct"].as_u64().unwrap_or_default(), 25);
    assert_eq!(contract["threshold_pct"].as_u64().unwrap_or_default(), 20);
    assert!(contract["prior_quote_id"].is_null());
    Ok(())
}
