use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum FlowType {
    NetNew,
    RenewalExpansion,
    DiscountException,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum FlowState {
    Draft,
    Validated,
    Priced,
    Approval,
    Approved,
    Finalized,
    Sent,
    Rejected,
    Expired,
    Cancelled,
    Revised,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum FlowEvent {
    RequiredFieldsCollected,
    PricingCalculated,
    PolicyClear,
    PolicyViolationDetected,
    ApprovalGranted,
    ApprovalDenied,
    QuoteDelivered,
    ReviseRequested,
    CancelRequested,
    QuoteExpired,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct FlowContext {
    pub missing_required_fields: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum FlowAction {
    PromptForMissingFields,
    EvaluatePricing,
    EvaluatePolicy,
    RouteApproval,
    FinalizeQuote,
    GenerateConfigurationFingerprint,
    GenerateDeliveryArtifacts,
    MarkQuoteSent,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TransitionOutcome {
    pub from: FlowState,
    pub to: FlowState,
    pub event: FlowEvent,
    pub actions: Vec<FlowAction>,
}
