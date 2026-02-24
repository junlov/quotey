use serde::Deserialize;
use serde_json::Value;
use std::collections::HashSet;

type SeedContractTestResult<T = ()> = Result<T, String>;

macro_rules! require {
    ($cond:expr) => {
        if !$cond {
            return Err(format!("assertion failed: `{}`", stringify!($cond)));
        }
    };
    ($cond:expr, $($arg:tt)*) => {
        if !$cond {
            return Err(format!($($arg)*));
        }
    };
}

macro_rules! require_eq {
    ($left:expr, $right:expr) => {
        if $left != $right {
            return Err(format!(
                "assertion failed: `left == right` (`{:?}` != `{:?}`)",
                $left,
                $right
            ));
        }
    };
    ($left:expr, $right:expr, $($arg:tt)*) => {
        if $left != $right {
            return Err(format!($($arg)*));
        }
    };
}

fn require_array<'a>(value: &'a Value, field_name: &str) -> Result<&'a [Value], String> {
    value
        .as_array()
        .map(|values| values.as_slice())
        .ok_or_else(|| format!("{field_name} should be an array"))
}

fn require_field<'a>(value: &'a Value, field_name: &str) -> SeedContractTestResult<&'a Value> {
    value.get(field_name).ok_or_else(|| format!("{field_name} should be present"))
}

fn require_str<'a>(value: &'a Value, field_name: &str) -> Result<&'a str, String> {
    value.as_str().ok_or_else(|| format!("{field_name} should be a string"))
}

fn require_u64(value: &Value, field_name: &str) -> Result<u64, String> {
    value.as_u64().ok_or_else(|| format!("{field_name} should be an unsigned integer"))
}

fn require_bool(value: &Value, field_name: &str) -> Result<bool, String> {
    value.as_bool().ok_or_else(|| format!("{field_name} should be a boolean"))
}

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

#[test]
fn seed_contract_matches_e2e_seed_sql_fixture() -> SeedContractTestResult {
    let fixture_sql = include_str!("../../../config/fixtures/e2e_seed_data.sql");
    let contract: SeedContract =
        serde_json::from_str(include_str!("../../../config/fixtures/e2e_seed_contract.json"))
            .map_err(|_| "seed contract JSON must parse".to_string())?;
    let mut flow_types_seen = HashSet::new();

    require_eq!(contract.dataset_version, "bd-3vp2.7.2");
    require_eq!(contract.seed_dataset, "deterministic_e2e_core_flows");
    require_eq!(contract.flows.len(), 3);

    for flow in &contract.flows {
        require!(
            flow_types_seen.insert(flow.flow_type.clone()),
            "duplicate flow type: {}",
            flow.flow_type
        );
        require!(!flow.quote_id.is_empty());
        require!(!flow.status.is_empty());
        require!(!flow.current_step.is_empty());
        require!(flow.expected_line_count > 0);
        require!(flow.step_number >= 1);
        require!(!flow.account_id.is_empty());
        require!(!flow.deal_id.is_empty());
        require!(!flow.policy_profile.is_empty());
        require!(!flow.product_ids.is_empty());
        if let Some(account_name) = &flow.account_name {
            require!(!account_name.is_empty());
        }
        if let Some(deal_name) = &flow.deal_name {
            require!(!deal_name.is_empty());
        }
        if let Some(product_names) = &flow.product_names {
            require!(!product_names.is_empty());
        }
        require!(!flow.expected_transition_checkpoints.is_empty());
        require!(!flow.expected_audit_events.is_empty());
        require!(!flow.missing_fields.is_empty() || flow.flow_type != "discount_exception");

        if let (Some(requested), Some(threshold)) =
            (flow.requested_discount_pct, flow.threshold_pct)
        {
            require!(
                requested >= 1,
                "requested discount should be positive for {}, got {}",
                flow.flow_type,
                requested
            );
            require!(
                threshold >= 1,
                "threshold should be positive for {}, got {}",
                flow.flow_type,
                threshold
            );
        }

        require!(
            fixture_sql.contains(&format!("'{}'", flow.quote_id)),
            "seed SQL fixture should include quote id {}",
            flow.quote_id
        );

        for required_field in &flow.required_fields {
            require!(
                fixture_sql.contains(required_field),
                "seed SQL fixture should include required field {} for {}",
                required_field,
                flow.flow_type
            );
        }

        if let Some(prior_quote_id) = &flow.prior_quote_id {
            require!(
                fixture_sql.contains(&format!("\"prior_quote_id\":\"{}\"", prior_quote_id)),
                "seed SQL fixture should include metadata prior quote {} for {}",
                prior_quote_id,
                flow.flow_type
            );
        }

        require!(
            fixture_sql.contains(&format!("\"account_id\":\"{}\"", flow.account_id)),
            "seed SQL fixture should include account id {} for {}",
            flow.account_id,
            flow.flow_type
        );
        if let Some(account_name) = &flow.account_name {
            require!(
                fixture_sql.contains(&format!("\"account_name\":\"{}\"", account_name)),
                "seed SQL fixture should include account name {} for {}",
                account_name,
                flow.flow_type
            );
        }
        if let Some(deal_name) = &flow.deal_name {
            require!(
                fixture_sql.contains(&format!("\"deal_name\":\"{}\"", deal_name)),
                "seed SQL fixture should include deal name {} for {}",
                deal_name,
                flow.flow_type
            );
        }
        if let Some(product_names) = &flow.product_names {
            for product_name in product_names {
                require!(
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
            require!(
                fixture_sql.contains(&format!("'{}', 'sent'", prior_quote_id)),
                "renewal prior quote should be seeded with sent status"
            );
            require!(
                fixture_sql.contains("'ql-renewal-prior-001-1'"),
                "renewal prior quote should include deterministic line 1"
            );
            require!(
                fixture_sql.contains("'ql-renewal-prior-001-2'"),
                "renewal prior quote should include deterministic line 2"
            );
        }

        if flow.flow_type == "discount_exception" {
            require!(
                fixture_sql.contains("approval.requested"),
                "discount flow should include approval request audit event"
            );
        }
    }

    for expected_flow in ["net_new", "renewal", "discount_exception"] {
        require!(
            flow_types_seen.contains(expected_flow),
            "missing canonical flow: {expected_flow}"
        );
    }
    Ok(())
}

#[test]
fn discount_threshold_matrix_is_consistent() -> SeedContractTestResult {
    let contract: SeedContract =
        serde_json::from_str(include_str!("../../../config/fixtures/e2e_seed_contract.json"))
            .map_err(|_| "seed contract JSON must parse".to_string())?;
    let mut seen_policy_points: HashSet<(String, u8)> = HashSet::new();
    let mut account_tiers_seen: HashSet<String> = HashSet::new();
    let mut auto_approve_count = 0usize;
    let mut managed_approval_count = 0usize;
    let mut total_rows = 0usize;

    for row in &contract.discount_threshold_matrix {
        total_rows += 1;
        require!(
            seen_policy_points.insert((row.account_tier.clone(), row.requested_discount_pct)),
            "duplicate discount-policy row detected for tier '{}' at requested discount {}",
            row.account_tier,
            row.requested_discount_pct
        );

        require_eq!(row.flow_type, "discount_exception");
        require!(!row.account_tier.is_empty());
        account_tiers_seen.insert(row.account_tier.clone());
        require!(!row.expected_routing.is_empty());
        require!(row.requested_discount_pct > 0);
        require!(row.threshold_pct > 0);
        let requires_approval = row.requested_discount_pct as i16 >= row.threshold_pct as i16;
        if row.approval_required {
            require!(
                row.expected_routing.contains("approval"),
                "approval-required matrix rows should encode approval routing explicitly for {} (got '{}')",
                row.account_tier,
                row.expected_routing
            );
            managed_approval_count += 1;
        } else {
            require_eq!(row.expected_routing, "auto_approve");
            auto_approve_count += 1;
        }
        require_eq!(
            row.approval_required,
            requires_approval,
            "routing must align with requested vs threshold boundary for {}: requested={} threshold={}",
            row.account_tier,
            row.requested_discount_pct,
            row.threshold_pct
        );
    }

    require!(
        contract.discount_threshold_matrix.len() >= 3,
        "discount threshold matrix should include multiple policy points"
    );
    require_eq!(total_rows, contract.discount_threshold_matrix.len());
    require!(
        account_tiers_seen.len() >= 2,
        "discount threshold matrix should cover at least two account tiers"
    );
    require!(
        auto_approve_count >= 1,
        "discount threshold matrix should include at least one auto-approve policy point"
    );
    require!(
        managed_approval_count >= 1,
        "discount threshold matrix should include at least one approval-required policy point"
    );
    Ok(())
}

#[test]
fn per_flow_contracts_derive_from_seed_contract() -> SeedContractTestResult {
    let seed_contract: SeedContract =
        serde_json::from_str(include_str!("../../../config/fixtures/e2e_seed_contract.json"))
            .map_err(|_| "seed contract JSON must parse".to_string())?;

    let netnew: Value = serde_json::from_str(include_str!(
        "../../../config/fixtures/e2e_netnew_flow_contract.json"
    ))
    .map_err(|_| "net-new flow contract JSON must parse".to_string())?;

    let renewal: Value = serde_json::from_str(include_str!(
        "../../../config/fixtures/e2e_renewal_flow_contract.json"
    ))
    .map_err(|_| "renewal flow contract JSON must parse".to_string())?;

    let discount: Value = serde_json::from_str(include_str!(
        "../../../config/fixtures/e2e_discount_exception_flow_contract.json"
    ))
    .map_err(|_| "discount exception flow contract JSON must parse".to_string())?;

    require_eq!(
        netnew["dataset_version"].as_str().unwrap_or_default(),
        seed_contract.dataset_version
    );
    require_eq!(
        renewal["dataset_version"].as_str().unwrap_or_default(),
        seed_contract.dataset_version
    );
    require_eq!(
        discount["dataset_version"].as_str().unwrap_or_default(),
        seed_contract.dataset_version
    );

    let netnew_seed = seed_contract
        .flows
        .iter()
        .find(|flow| flow.flow_type == "net_new")
        .ok_or_else(|| "missing canonical net_new flow".to_string())?;
    let renewal_seed = seed_contract
        .flows
        .iter()
        .find(|flow| flow.flow_type == "renewal")
        .ok_or_else(|| "missing canonical renewal flow".to_string())?;
    let discount_seed = seed_contract
        .flows
        .iter()
        .find(|flow| flow.flow_type == "discount_exception")
        .ok_or_else(|| "missing canonical discount_exception flow".to_string())?;

    let assert_seed_fields_match = |contract: &Value,
                                    flow: &SeedFlowContract|
     -> SeedContractTestResult {
        require_eq!(contract["quote_id"].as_str(), Some(flow.quote_id.as_str()));
        require_eq!(contract["account_id"].as_str(), Some(flow.account_id.as_str()));
        require_eq!(contract["deal_id"].as_str(), Some(flow.deal_id.as_str()));
        require_eq!(contract["policy_profile"].as_str(), Some(flow.policy_profile.as_str()));
        require_eq!(contract["status"].as_str(), Some(flow.status.as_str()));
        require_eq!(contract["current_step"].as_str(), Some(flow.current_step.as_str()));
        require_eq!(
            contract["step_number"].as_u64().unwrap_or_default(),
            u64::from(flow.step_number)
        );
        require_eq!(
            contract["expected_line_count"].as_u64().unwrap_or_default(),
            u64::from(flow.expected_line_count)
        );
        require_eq!(
            contract["expected_transition_checkpoints"]
                .as_array()
                .ok_or_else(|| "expected_transition_checkpoints should be an array".to_string())?
                .iter()
                .map(|v| v.as_str().unwrap_or_default().to_owned())
                .collect::<Vec<_>>(),
            flow.expected_transition_checkpoints
                .iter()
                .map(std::string::ToString::to_string)
                .collect::<Vec<_>>()
        );
        require_eq!(
            contract["expected_audit_events"]
                .as_array()
                .ok_or_else(|| "expected_audit_events should be an array".to_string())?
                .iter()
                .map(|v| v.as_str().unwrap_or_default().to_owned())
                .collect::<Vec<_>>(),
            flow.expected_audit_events
                .iter()
                .map(std::string::ToString::to_string)
                .collect::<Vec<_>>()
        );
        require_eq!(
            contract["required_fields"]
                .as_array()
                .ok_or_else(|| "required_fields should be an array".to_string())?
                .iter()
                .map(|v| v.as_str().unwrap_or_default().to_owned())
                .collect::<Vec<_>>(),
            flow.required_fields.iter().map(std::string::ToString::to_string).collect::<Vec<_>>()
        );
        require_eq!(
            contract["missing_fields"]
                .as_array()
                .ok_or_else(|| "missing_fields should be an array".to_string())?
                .iter()
                .map(|v| v.as_str().unwrap_or_default().to_owned())
                .collect::<Vec<_>>(),
            flow.missing_fields.iter().map(std::string::ToString::to_string).collect::<Vec<_>>()
        );
        if let Some(account_name) = &flow.account_name {
            require_eq!(contract["account_name"].as_str(), Some(account_name.as_str()));
        }
        if let Some(deal_name) = &flow.deal_name {
            require_eq!(contract["deal_name"].as_str(), Some(deal_name.as_str()));
        }
        require_eq!(
            contract["product_ids"]
                .as_array()
                .ok_or_else(|| "product_ids should be an array".to_string())?
                .iter()
                .map(|v| v.as_str().unwrap_or_default().to_owned())
                .collect::<Vec<_>>(),
            flow.product_ids.iter().map(std::string::ToString::to_string).collect::<Vec<_>>()
        );
        match (&flow.product_names, contract["product_names"].as_array()) {
            (Some(names), Some(contract_names)) => {
                require_eq!(
                    contract_names
                        .iter()
                        .map(|v| v.as_str().unwrap_or_default().to_owned())
                        .collect::<Vec<_>>(),
                    names.iter().map(std::string::ToString::to_string).collect::<Vec<_>>()
                );
            }
            (Some(_), None) | (None, Some(_)) => {
                return Err(
                    "product_names presence must match canonical contract definition".to_string()
                );
            }
            _ => {}
        }
        Ok(())
    };

    require_eq!(netnew["flow_type"].as_str(), Some("net_new"));
    require_eq!(netnew["seed_quote_id"].as_str(), Some(netnew_seed.quote_id.as_str()));
    require_eq!(netnew["child_bead"].as_str(), Some("bd-3vp2.8.1.1"));
    assert_seed_fields_match(&netnew["seed_contract"], netnew_seed)?;

    require_eq!(renewal["flow_type"].as_str(), Some("renewal"));
    require_eq!(renewal["seed_quote_id"].as_str(), Some(renewal_seed.quote_id.as_str()));
    require_eq!(renewal["child_bead"].as_str(), Some("bd-3vp2.8.2.1"));
    assert_seed_fields_match(&renewal["seed_contract"], renewal_seed)?;
    require_eq!(
        renewal["seed_contract"]["prior_quote_id"].as_str(),
        Some("quote-renewal-prior-001")
    );
    require_eq!(renewal["flow_type"].as_str(), Some(renewal_seed.flow_type.as_str()));

    require_eq!(discount["flow_type"].as_str(), Some("discount_exception"));
    require_eq!(discount["seed_quote_id"].as_str(), Some(discount_seed.quote_id.as_str()));
    require_eq!(discount["child_bead"].as_str(), Some("bd-3vp2.8.3.1"));
    assert_seed_fields_match(&discount["seed_contract"], discount_seed)?;
    require_eq!(
        discount["seed_contract"]["requested_discount_pct"].as_u64().unwrap_or_default(),
        u64::from(discount_seed.requested_discount_pct.unwrap_or(0))
    );
    require_eq!(
        discount["seed_contract"]["threshold_pct"].as_u64().unwrap_or_default(),
        u64::from(discount_seed.threshold_pct.unwrap_or(0))
    );
    if let Some(seed_prior) = &discount_seed.prior_quote_id {
        require_eq!(
            discount["seed_contract"]["prior_quote_id"].as_str(),
            Some(seed_prior.as_str())
        );
    } else {
        require!(discount["seed_contract"]["prior_quote_id"].is_null());
    }
    Ok(())
}

#[test]
fn resilience_fault_contract_shape_is_deterministic() -> SeedContractTestResult {
    let resilience_contract: Value = serde_json::from_str(include_str!(
        "../../../config/fixtures/e2e_resilience_fault_contract.json"
    ))
    .map_err(|_| "resilience fault contract JSON must parse".to_string())?;

    require_eq!(resilience_contract["dataset_version"].as_str(), Some("bd-3vp2.7.2"));
    require_eq!(resilience_contract["child_bead"].as_str(), Some("bd-3vp2.8.4.1"));
    require_eq!(resilience_contract["scenario_group"].as_str(), Some("resilience_fault_matrix"));

    let phase_fault_matrix = require_array(
        require_field(&resilience_contract, "phase_fault_matrix")?,
        "phase_fault_matrix",
    )?;
    require_eq!(phase_fault_matrix.len(), 4);

    let phases = phase_fault_matrix
        .iter()
        .map(|entry| require_str(require_field(entry, "phase")?, "phase"))
        .collect::<SeedContractTestResult<Vec<_>>>()?;
    require_eq!(
        phases,
        vec!["seed_load", "policy_evaluation", "pricing_evaluation", "approval_routing"]
    );

    for entry in phase_fault_matrix {
        let phase = require_str(require_field(entry, "phase")?, "phase")?;
        let fault_domain = require_str(require_field(entry, "fault_domain")?, "fault_domain")?;
        let injected_error =
            require_str(require_field(entry, "injected_error")?, "injected_error")?;
        let expected_outcome =
            require_str(require_field(entry, "expected_outcome")?, "expected_outcome")?;
        let expected_retry_window = entry.get("expected_retry_window_sec");
        let expected_retryable = require_bool(require_field(entry, "retryable")?, "retryable")?;
        let max_retries = require_u64(require_field(entry, "max_retries")?, "max_retries")?;

        require!(!fault_domain.is_empty(), "fault_domain should not be empty for phase {}", phase);
        require!(
            !injected_error.is_empty(),
            "injected_error should not be empty for phase {}",
            phase
        );
        require!(
            !expected_outcome.is_empty(),
            "expected_outcome should not be empty for phase {}",
            phase
        );
        require!(
            matches!(
                expected_outcome,
                "recover_and_continue" | "recover_and_retry" | "fail_fast" | "retry_with_backoff"
            ),
            "unexpected expected_outcome {} for phase {}",
            expected_outcome,
            phase
        );

        if expected_retryable {
            let expected_retry_window = require_field(entry, "expected_retry_window_sec")?;
            let window = require_array(expected_retry_window, "expected_retry_window_sec")?;
            require!(!window.is_empty());
            require!(max_retries > 0);
            for window_value in window {
                let seconds = require_u64(window_value, "expected_retry_window_sec entry")?;
                require!(seconds > 0, "retry window should be strictly positive");
            }
        } else {
            require_eq!(expected_outcome, "fail_fast");
            require_eq!(max_retries, 0);
            require!(expected_retry_window.is_none());
        }
        require!(!phase.is_empty());
    }
    Ok(())
}

#[test]
fn resilience_fault_contract_phase_rules_are_deterministic() -> SeedContractTestResult {
    let resilience_contract: Value = serde_json::from_str(include_str!(
        "../../../config/fixtures/e2e_resilience_fault_contract.json"
    ))
    .map_err(|_| "resilience fault contract JSON must parse".to_string())?;

    let phase_fault_matrix = resilience_contract["phase_fault_matrix"]
        .as_array()
        .ok_or_else(|| "phase_fault_matrix should be an array".to_string())?;
    require_eq!(phase_fault_matrix.len(), 4);

    for entry in phase_fault_matrix {
        let phase = require_str(require_field(entry, "phase")?, "phase")?;
        let expected_outcome =
            require_str(require_field(entry, "expected_outcome")?, "expected_outcome")?;
        let expected_retryable = require_bool(require_field(entry, "retryable")?, "retryable")?;
        let expected_retry_window = entry.get("expected_retry_window_sec");
        let max_retries = require_u64(require_field(entry, "max_retries")?, "max_retries")?;

        match phase {
            "seed_load" => {
                require_eq!(expected_outcome, "recover_and_continue");
                require!(expected_retryable);
                require_eq!(max_retries, 3);
                require!(expected_retry_window.is_some());
            }
            "policy_evaluation" => {
                require_eq!(expected_outcome, "recover_and_retry");
                require!(expected_retryable);
                require_eq!(max_retries, 2);
                require!(expected_retry_window.is_some());
            }
            "pricing_evaluation" => {
                require_eq!(expected_outcome, "fail_fast");
                require!(!expected_retryable);
                require_eq!(max_retries, 0);
                require!(expected_retry_window.is_none());
            }
            "approval_routing" => {
                require_eq!(expected_outcome, "retry_with_backoff");
                require!(expected_retryable);
                require_eq!(max_retries, 2);
                require!(expected_retry_window.is_some());
            }
            _ => {}
        }
    }
    Ok(())
}

#[test]
fn netnew_flow_contract_is_self_consistent() -> SeedContractTestResult {
    let netnew: Value = serde_json::from_str(include_str!(
        "../../../config/fixtures/e2e_netnew_flow_contract.json"
    ))
    .map_err(|_| "net-new flow contract JSON must parse".to_string())?;

    require_eq!(netnew["dataset_version"].as_str(), Some("bd-3vp2.7.2"));
    require_eq!(netnew["flow_type"].as_str(), Some("net_new"));
    require_eq!(netnew["child_bead"].as_str(), Some("bd-3vp2.8.1.1"));
    let contract = &netnew["seed_contract"];

    require_eq!(contract["quote_id"].as_str(), Some("quote-netnew-001"));
    require_eq!(contract["account_id"].as_str(), Some("acct-netnew-001"));
    require_eq!(contract["deal_id"].as_str(), Some("deal-netnew-001"));
    require_eq!(contract["account_name"].as_str(), Some("Acme Corp"));
    require_eq!(contract["deal_name"].as_str(), Some("Acme Corp - New License"));
    require_eq!(contract["policy_profile"].as_str(), Some("standard"));
    require_eq!(contract["status"].as_str(), Some("draft"));
    require_eq!(contract["current_step"].as_str(), Some("gather_requirements"));
    require_eq!(contract["step_number"].as_u64().unwrap_or_default(), 1);
    require_eq!(contract["expected_line_count"].as_u64().unwrap_or_default(), 3);

    require_eq!(
        contract["required_fields"]
            .as_array()
            .ok_or_else(|| "required_fields should be present".to_string())?
            .iter()
            .map(|v| v.as_str())
            .collect::<Vec<_>>(),
        vec![Some("billing_country"), Some("payment_terms")]
    );
    require_eq!(
        contract["missing_fields"]
            .as_array()
            .ok_or_else(|| "missing_fields should be present".to_string())?
            .iter()
            .map(|v| v.as_str())
            .collect::<Vec<_>>(),
        vec![Some("billing_country"), Some("payment_terms")]
    );

    require_eq!(
        contract["product_ids"]
            .as_array()
            .ok_or_else(|| "product_ids should be present".to_string())?
            .iter()
            .map(|v| v.as_str())
            .collect::<Vec<_>>(),
        vec![Some("prod-plan-ent"), Some("prod-sso"), Some("prod-support-premium")]
    );
    require_eq!(
        contract["product_names"]
            .as_array()
            .ok_or_else(|| "product_names should be present".to_string())?
            .iter()
            .map(|v| v.as_str())
            .collect::<Vec<_>>(),
        vec![Some("Enterprise Plan"), Some("SSO Add-on"), Some("Premium Support")]
    );
    require!(contract["requested_discount_pct"].is_null());
    require!(contract["threshold_pct"].is_null());
    require!(contract["prior_quote_id"].is_null());
    Ok(())
}

#[test]
fn renewal_flow_contract_is_self_consistent() -> SeedContractTestResult {
    let renewal: Value = serde_json::from_str(include_str!(
        "../../../config/fixtures/e2e_renewal_flow_contract.json"
    ))
    .map_err(|_| "renewal flow contract JSON must parse".to_string())?;

    require_eq!(renewal["dataset_version"].as_str(), Some("bd-3vp2.7.2"));
    require_eq!(renewal["flow_type"].as_str(), Some("renewal"));
    require_eq!(renewal["child_bead"].as_str(), Some("bd-3vp2.8.2.1"));
    let contract = &renewal["seed_contract"];

    require_eq!(contract["quote_id"].as_str(), Some("quote-renewal-001"));
    require_eq!(contract["account_id"].as_str(), Some("acct-renewal-001"));
    require_eq!(contract["deal_id"].as_str(), Some("deal-renewal-001"));
    require_eq!(contract["account_name"].as_str(), Some("Globex Industries"));
    require_eq!(contract["deal_name"].as_str(), Some("Globex - Annual Renewal"));
    require_eq!(contract["policy_profile"].as_str(), Some("renewal"));
    require_eq!(contract["status"].as_str(), Some("priced"));
    require_eq!(contract["current_step"].as_str(), Some("validate_expansion"));
    require_eq!(contract["step_number"].as_u64().unwrap_or_default(), 3);
    require_eq!(contract["expected_line_count"].as_u64().unwrap_or_default(), 3);

    require_eq!(
        contract["required_fields"]
            .as_array()
            .ok_or_else(|| "required_fields should be present".to_string())?
            .iter()
            .map(|v| v.as_str())
            .collect::<Vec<_>>(),
        vec![Some("prior_quote_id")]
    );
    require_eq!(
        contract["missing_fields"]
            .as_array()
            .ok_or_else(|| "missing_fields should be present".to_string())?
            .iter()
            .map(|v| v.as_str())
            .collect::<Vec<_>>(),
        Vec::<Option<&str>>::new()
    );
    require_eq!(
        contract["product_ids"]
            .as_array()
            .ok_or_else(|| "product_ids should be present".to_string())?
            .iter()
            .map(|v| v.as_str())
            .collect::<Vec<_>>(),
        vec![Some("prod-plan-ent"), Some("prod-support-premium"), Some("prod-onboarding")]
    );
    require_eq!(
        contract["product_names"]
            .as_array()
            .ok_or_else(|| "product_names should be present".to_string())?
            .iter()
            .map(|v| v.as_str())
            .collect::<Vec<_>>(),
        vec![Some("Enterprise Plan"), Some("Premium Support"), Some("Onboarding")]
    );
    require_eq!(contract["prior_quote_id"].as_str(), Some("quote-renewal-prior-001"));
    require!(contract["requested_discount_pct"].is_null());
    require!(contract["threshold_pct"].is_null());
    Ok(())
}

#[test]
fn discount_flow_contract_is_self_consistent() -> SeedContractTestResult {
    let discount: Value = serde_json::from_str(include_str!(
        "../../../config/fixtures/e2e_discount_exception_flow_contract.json"
    ))
    .map_err(|_| "discount exception flow contract JSON must parse".to_string())?;

    require_eq!(discount["dataset_version"].as_str(), Some("bd-3vp2.7.2"));
    require_eq!(discount["flow_type"].as_str(), Some("discount_exception"));
    require_eq!(discount["child_bead"].as_str(), Some("bd-3vp2.8.3.1"));
    let contract = &discount["seed_contract"];

    require_eq!(contract["quote_id"].as_str(), Some("quote-discount-001"));
    require_eq!(contract["account_id"].as_str(), Some("acct-discount-001"));
    require_eq!(contract["deal_id"].as_str(), Some("deal-discount-001"));
    require_eq!(contract["account_name"].as_str(), Some("Initech LLC"));
    require_eq!(contract["deal_name"].as_str(), Some("Initech - Expansion Deal"));
    require_eq!(contract["policy_profile"].as_str(), Some("discount_exception"));
    require_eq!(contract["status"].as_str(), Some("approval"));
    require_eq!(contract["current_step"].as_str(), Some("awaiting_approval"));
    require_eq!(contract["step_number"].as_u64().unwrap_or_default(), 4);
    require_eq!(contract["expected_line_count"].as_u64().unwrap_or_default(), 2);
    require_eq!(
        contract["required_fields"]
            .as_array()
            .ok_or_else(|| "required_fields should be present".to_string())?
            .iter()
            .map(|v| v.as_str())
            .collect::<Vec<_>>(),
        vec![Some("approval_decision")]
    );
    require_eq!(
        contract["missing_fields"]
            .as_array()
            .ok_or_else(|| "missing_fields should be present".to_string())?
            .iter()
            .map(|v| v.as_str())
            .collect::<Vec<_>>(),
        vec![Some("approval_decision")]
    );
    require_eq!(
        contract["product_ids"]
            .as_array()
            .ok_or_else(|| "product_ids should be present".to_string())?
            .iter()
            .map(|v| v.as_str())
            .collect::<Vec<_>>(),
        vec![Some("prod-plan-pro"), Some("prod-sso")]
    );
    require_eq!(
        contract["product_names"]
            .as_array()
            .ok_or_else(|| "product_names should be present".to_string())?
            .iter()
            .map(|v| v.as_str())
            .collect::<Vec<_>>(),
        vec![Some("Pro Plan"), Some("SSO Add-on")]
    );
    require_eq!(contract["requested_discount_pct"].as_u64().unwrap_or_default(), 25);
    require_eq!(contract["threshold_pct"].as_u64().unwrap_or_default(), 20);
    require!(contract["prior_quote_id"].is_null());
    Ok(())
}
