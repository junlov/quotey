use thiserror::Error;

use crate::audit::{AuditCategory, AuditContext, AuditEvent, AuditOutcome, AuditSink};
use crate::cpq::policy::PolicyInput;
use crate::cpq::{CpqEvaluation, CpqEvaluationInput, CpqRuntime};
use crate::domain::quote::Quote;
use crate::flows::states::{
    FlowAction, FlowContext, FlowEvent, FlowState, FlowType, TransitionOutcome,
};

pub trait FlowDefinition {
    fn flow_type(&self) -> FlowType;
    fn initial_state(&self) -> FlowState;
    fn transition(
        &self,
        current: &FlowState,
        event: &FlowEvent,
        context: &FlowContext,
    ) -> Result<TransitionOutcome, FlowTransitionError>;
}

#[derive(Clone, Debug, Default)]
pub struct NetNewFlow;

impl FlowDefinition for NetNewFlow {
    fn flow_type(&self) -> FlowType {
        FlowType::NetNew
    }

    fn initial_state(&self) -> FlowState {
        FlowState::Draft
    }

    fn transition(
        &self,
        current: &FlowState,
        event: &FlowEvent,
        context: &FlowContext,
    ) -> Result<TransitionOutcome, FlowTransitionError> {
        transition_net_new(current, event, context)
    }
}

pub struct FlowEngine<F> {
    flow: F,
}

impl<F> FlowEngine<F>
where
    F: FlowDefinition,
{
    pub fn new(flow: F) -> Self {
        Self { flow }
    }

    pub fn flow_type(&self) -> FlowType {
        self.flow.flow_type()
    }

    pub fn initial_state(&self) -> FlowState {
        self.flow.initial_state()
    }

    pub fn apply(
        &self,
        current: &FlowState,
        event: &FlowEvent,
        context: &FlowContext,
    ) -> Result<TransitionOutcome, FlowTransitionError> {
        self.flow.transition(current, event, context)
    }

    pub fn apply_with_audit<S>(
        &self,
        current: &FlowState,
        event: &FlowEvent,
        context: &FlowContext,
        sink: &S,
        audit: &AuditContext,
    ) -> Result<TransitionOutcome, FlowTransitionError>
    where
        S: AuditSink,
    {
        let result = self.apply(current, event, context);
        match &result {
            Ok(outcome) => {
                sink.emit(
                    AuditEvent::new(
                        audit.quote_id.clone(),
                        audit.thread_id.clone(),
                        audit.correlation_id.clone(),
                        "flow.transition_applied",
                        AuditCategory::Flow,
                        audit.actor.clone(),
                        AuditOutcome::Success,
                    )
                    .with_metadata("from", format!("{:?}", outcome.from))
                    .with_metadata("to", format!("{:?}", outcome.to))
                    .with_metadata("event", format!("{:?}", outcome.event)),
                );
            }
            Err(error) => {
                sink.emit(
                    AuditEvent::new(
                        audit.quote_id.clone(),
                        audit.thread_id.clone(),
                        audit.correlation_id.clone(),
                        "flow.transition_rejected",
                        AuditCategory::Flow,
                        audit.actor.clone(),
                        AuditOutcome::Rejected,
                    )
                    .with_metadata("error", error.to_string()),
                );
            }
        }
        result
    }

    pub fn evaluate_cpq<R>(
        &self,
        runtime: &R,
        quote: &Quote,
        currency: &str,
        policy_input: PolicyInput,
    ) -> CpqEvaluation
    where
        R: CpqRuntime,
    {
        runtime.evaluate_quote(CpqEvaluationInput { quote, currency, policy_input })
    }

    pub fn evaluate_cpq_with_audit<R, S>(
        &self,
        runtime: &R,
        quote: &Quote,
        currency: &str,
        policy_input: PolicyInput,
        sink: &S,
        audit: &AuditContext,
    ) -> CpqEvaluation
    where
        R: CpqRuntime,
        S: AuditSink,
    {
        let evaluation = self.evaluate_cpq(runtime, quote, currency, policy_input);
        sink.emit(
            AuditEvent::new(
                Some(quote.id.clone()),
                audit.thread_id.clone(),
                audit.correlation_id.clone(),
                "cpq.runtime_evaluated",
                AuditCategory::Pricing,
                audit.actor.clone(),
                AuditOutcome::Success,
            )
            .with_metadata("total", evaluation.pricing.total.to_string())
            .with_metadata("approval_required", evaluation.policy.approval_required.to_string()),
        );
        evaluation
    }
}

impl Default for FlowEngine<NetNewFlow> {
    fn default() -> Self {
        Self::new(NetNewFlow)
    }
}

#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum FlowTransitionError {
    #[error("missing required fields before transition from {state:?}: {missing_fields:?}")]
    MissingRequiredFields { state: FlowState, missing_fields: Vec<String> },
    #[error("invalid transition from {state:?} using event {event:?}")]
    InvalidTransition { state: FlowState, event: FlowEvent },
}

fn transition_net_new(
    current: &FlowState,
    event: &FlowEvent,
    context: &FlowContext,
) -> Result<TransitionOutcome, FlowTransitionError> {
    use FlowAction::{
        EvaluatePolicy, EvaluatePricing, FinalizeQuote, GenerateDeliveryArtifacts, MarkQuoteSent,
        RouteApproval,
    };
    use FlowEvent::{
        ApprovalDenied, ApprovalGranted, CancelRequested, PolicyClear, PolicyViolationDetected,
        PricingCalculated, QuoteDelivered, QuoteExpired, RequiredFieldsCollected, ReviseRequested,
    };
    use FlowState::{
        Approval, Approved, Cancelled, Draft, Expired, Finalized, Priced, Rejected, Revised, Sent,
        Validated,
    };

    let (to, actions) = match (current, event) {
        (Draft, RequiredFieldsCollected) | (Revised, RequiredFieldsCollected) => {
            if !context.missing_required_fields.is_empty() {
                return Err(FlowTransitionError::MissingRequiredFields {
                    state: current.clone(),
                    missing_fields: context.missing_required_fields.clone(),
                });
            }
            (Validated, vec![EvaluatePricing])
        }
        (Validated, PricingCalculated) => (Priced, vec![EvaluatePolicy]),
        (Priced, PolicyClear) => (Finalized, vec![FinalizeQuote, GenerateDeliveryArtifacts]),
        (Priced, PolicyViolationDetected) => (Approval, vec![RouteApproval]),
        (Approval, ApprovalGranted) => (Approved, vec![FinalizeQuote]),
        (Approval, ApprovalDenied) => (Rejected, Vec::new()),
        (Finalized, QuoteDelivered) | (Approved, QuoteDelivered) => (Sent, vec![MarkQuoteSent]),
        (Rejected, ReviseRequested) => (Revised, vec![EvaluatePricing]),
        (_, CancelRequested) => (Cancelled, Vec::new()),
        (Sent, QuoteExpired) | (Cancelled, QuoteExpired) => {
            return Err(FlowTransitionError::InvalidTransition {
                state: current.clone(),
                event: event.clone(),
            });
        }
        (_, QuoteExpired) => (Expired, Vec::new()),
        _ => {
            return Err(FlowTransitionError::InvalidTransition {
                state: current.clone(),
                event: event.clone(),
            });
        }
    };

    Ok(TransitionOutcome { from: current.clone(), to, event: event.clone(), actions })
}

#[cfg(test)]
mod tests {
    use chrono::Utc;
    use rust_decimal::Decimal;

    use crate::audit::InMemoryAuditSink;
    use crate::cpq::{policy::PolicyInput, DeterministicCpqRuntime};
    use crate::domain::{
        product::ProductId,
        quote::{Quote, QuoteId, QuoteLine, QuoteStatus},
    };
    use crate::flows::engine::{FlowDefinition, FlowEngine, FlowTransitionError, NetNewFlow};
    use crate::flows::states::{FlowAction, FlowContext, FlowEvent, FlowState, FlowType};

    #[test]
    fn net_new_flow_happy_path_without_approval() {
        let engine = FlowEngine::new(NetNewFlow);
        let mut state = engine.initial_state();
        let context = FlowContext::default();

        state = engine
            .apply(&state, &FlowEvent::RequiredFieldsCollected, &context)
            .expect("draft -> validated")
            .to;
        state = engine
            .apply(&state, &FlowEvent::PricingCalculated, &context)
            .expect("validated -> priced")
            .to;
        let finalized =
            engine.apply(&state, &FlowEvent::PolicyClear, &context).expect("priced -> finalized");

        assert_eq!(finalized.to, FlowState::Finalized);
        assert!(finalized.actions.contains(&FlowAction::GenerateDeliveryArtifacts));

        state = finalized.to;
        state = engine
            .apply(&state, &FlowEvent::QuoteDelivered, &context)
            .expect("finalized -> sent")
            .to;
        assert_eq!(state, FlowState::Sent);
    }

    #[test]
    fn net_new_flow_approval_path() {
        let engine = FlowEngine::default();
        let context = FlowContext::default();

        let validated = engine
            .apply(&FlowState::Draft, &FlowEvent::RequiredFieldsCollected, &context)
            .expect("draft -> validated")
            .to;
        let priced = engine
            .apply(&validated, &FlowEvent::PricingCalculated, &context)
            .expect("validated -> priced")
            .to;
        let approval = engine
            .apply(&priced, &FlowEvent::PolicyViolationDetected, &context)
            .expect("priced -> approval");
        assert_eq!(approval.to, FlowState::Approval);
        assert_eq!(approval.actions, vec![FlowAction::RouteApproval]);

        let approved = engine
            .apply(&approval.to, &FlowEvent::ApprovalGranted, &context)
            .expect("approval -> approved")
            .to;
        let sent = engine
            .apply(&approved, &FlowEvent::QuoteDelivered, &context)
            .expect("approved -> sent")
            .to;
        assert_eq!(sent, FlowState::Sent);
    }

    #[test]
    fn invalid_transition_is_rejected() {
        let engine = FlowEngine::default();
        let error = engine
            .apply(&FlowState::Draft, &FlowEvent::PricingCalculated, &FlowContext::default())
            .expect_err("draft cannot transition directly to pricing");

        assert!(matches!(
            error,
            FlowTransitionError::InvalidTransition {
                state: FlowState::Draft,
                event: FlowEvent::PricingCalculated
            }
        ));
    }

    #[test]
    fn missing_required_fields_are_rejected() {
        let engine = FlowEngine::default();
        let error = engine
            .apply(
                &FlowState::Draft,
                &FlowEvent::RequiredFieldsCollected,
                &FlowContext {
                    missing_required_fields: vec![
                        "billing_country".to_owned(),
                        "currency".to_owned(),
                    ],
                },
            )
            .expect_err("must reject missing fields");

        assert!(matches!(error, FlowTransitionError::MissingRequiredFields { .. }));
    }

    #[test]
    fn replay_is_deterministic_for_same_event_sequence() {
        let engine = FlowEngine::default();
        let events = [
            FlowEvent::RequiredFieldsCollected,
            FlowEvent::PricingCalculated,
            FlowEvent::PolicyClear,
            FlowEvent::QuoteDelivered,
        ];

        let run = |engine: &FlowEngine<NetNewFlow>| {
            let mut state = engine.initial_state();
            let mut actions = Vec::new();
            for event in &events {
                let outcome = engine
                    .apply(&state, event, &FlowContext::default())
                    .expect("deterministic run");
                actions.push(outcome.actions);
                state = outcome.to;
            }
            (state, actions)
        };

        let first = run(&engine);
        let second = run(&engine);

        assert_eq!(first, second);
        assert_eq!(engine.flow_type(), FlowType::NetNew);
        assert_eq!(NetNewFlow.flow_type(), FlowType::NetNew);
    }

    #[test]
    fn flow_runtime_can_call_cpq_stubs() {
        let engine = FlowEngine::default();
        let runtime = DeterministicCpqRuntime::default();
        let quote = Quote {
            id: QuoteId("Q-2026-0044".to_owned()),
            status: QuoteStatus::Draft,
            lines: vec![QuoteLine {
                product_id: ProductId("plan-pro".to_owned()),
                quantity: 2,
                unit_price: Decimal::new(4_999, 2),
            }],
            created_at: Utc::now(),
        };

        let result = engine.evaluate_cpq(
            &runtime,
            &quote,
            "USD",
            PolicyInput {
                requested_discount_pct: Decimal::ZERO,
                deal_value: Decimal::new(9_998, 2),
                minimum_margin_pct: Decimal::new(3_000, 2),
            },
        );

        assert!(result.constraints.valid);
        assert!(result.pricing.total > Decimal::ZERO);
    }

    #[test]
    fn flow_transition_emits_audit_event() {
        let engine = FlowEngine::default();
        let sink = InMemoryAuditSink::default();

        let _ = engine
            .apply_with_audit(
                &FlowState::Draft,
                &FlowEvent::RequiredFieldsCollected,
                &FlowContext::default(),
                &sink,
                &crate::audit::AuditContext::new(
                    Some(QuoteId("Q-2026-0009".to_owned())),
                    Some("1730000000.0200".to_owned()),
                    "req-42",
                    "flow-engine",
                ),
            )
            .expect("transition should succeed");

        let events = sink.events();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].correlation_id, "req-42");
        assert_eq!(events[0].thread_id.as_deref(), Some("1730000000.0200"));
        assert_eq!(events[0].event_type, "flow.transition_applied");
    }
}
