// Re-export chrono for downstream crates that need DateTime types
pub use chrono;

pub mod approvals;
pub mod archaeology;
pub mod audit;
pub mod collab;
pub mod config;
pub mod cpq;
pub mod dna;
pub mod domain;
pub mod errors;
pub mod execution_engine;
pub mod explanation;
pub mod flows;
pub mod ghost;
pub mod ledger;
pub mod policy;

pub use approvals::{
    ApprovalValidationFailure, ApprovalValidationInput, ApprovalValidationResult,
    ApprovalValidator, ApproverAuthority,
};
pub use archaeology::{
    CatalogConstraint, ConstraintEdgeType, DependencyEdge, DependencyGraph, DependencyGraphEngine,
    DependencyNode, GraphAnalysis, GraphBlockage, ResolutionPath,
};
pub use collab::{
    OperationAuthority, OperationHistoryEntry, OperationStatus, OperationType,
    OperationalTransform, QuoteOperation, TransformResult,
};
pub use dna::{
    ConfigurationFingerprint, DealOutcomeMetadata, DealOutcomeStatus, FingerprintGenerator,
    SimilarDeal, SimilarityCandidate, SimilarityEngine,
};
pub use domain::approval::{ApprovalId, ApprovalRequest, ApprovalStatus};
pub use domain::execution::{
    ExecutionTask, ExecutionTaskId, ExecutionTaskState, ExecutionTransitionEvent,
    ExecutionTransitionId, IdempotencyRecord, IdempotencyRecordState, OperationKey,
};
pub use domain::product::{Product, ProductId};
pub use domain::quote::{Quote, QuoteId, QuoteLine, QuoteStatus};
pub use errors::{ApplicationError, DomainError, InterfaceError};
pub use execution_engine::{
    ClaimResult, DeterministicExecutionEngine, ExecutionEngineConfig, ExecutionError,
    InMemoryExecutionEngine, RetryPolicy, TransitionResult,
};
pub use ghost::{
    GhostQuote, GhostQuoteGenerator, InMemoryCustomerHistoryProvider, InMemoryGhostQuoteStore,
    Signal, SignalDetector, SignalDetectorConfig,
};
pub use domain::explanation::*;
pub use explanation::{
    AppliedRule, CalculationStep, ExplanationEngine, ExplanationError, InMemoryPolicyProvider,
    InMemoryPricingProvider, PolicyEvaluation, PolicyEvaluationProvider, PolicyViolation,
    PricingLineSnapshot, PricingSnapshot, PricingSnapshotProvider,
};
pub use ledger::{LedgerAction, LedgerEntry, LedgerService, VerificationResult};
pub use policy::{ExplanationGenerator, ExplanationTemplate, GeneratedExplanation};
