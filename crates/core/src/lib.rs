// Re-export chrono for downstream crates that need DateTime types
pub use chrono;

pub mod ambiguity;
pub use ambiguity::{
    Ambiguity, AmbiguityDetectionEngine, AmbiguityDetectionInput, AmbiguityOption, AmbiguitySet,
    AmbiguitySeverity, AmbiguityType, DateMention, ProductMention, QuantityMention,
    render_ambiguity_slack_blocks,
};
pub mod approvals;
pub mod archaeology;
pub mod audit;
pub mod autopsy;
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
pub mod suggestions;

pub use approvals::{
    ApprovalValidationFailure, ApprovalValidationInput, ApprovalValidationResult,
    ApprovalValidator, ApproverAuthority,
};
pub use archaeology::{
    CatalogConstraint, ConstraintEdgeType, DependencyEdge, DependencyGraph, DependencyGraphEngine,
    DependencyNode, GraphAnalysis, GraphBlockage, ResolutionPath,
};
pub use autopsy::{
    AlternativeDecision, AttributionGraphBuilder, AttributionGraphSnapshot, AuditTrailEntry,
    AutopsyError, AutopsyInput, AutopsyReport, CounterfactualEngine, CounterfactualError,
    CounterfactualRequest, CounterfactualResponse, DealAutopsyEngine, ForkComparison,
    GenomeFinding, GenomeQueryError, GenomeQueryRequest, GenomeQueryResponse,
    RevenueGenomeQueryEngine,
};
pub use collab::{
    OperationAuthority, OperationHistoryEntry, OperationStatus, OperationType,
    OperationalTransform, QuoteOperation, TransformResult,
};
pub use dna::{
    ClosedDealOutcome, ConfigurationFingerprint, DealDnaLifecycleService, DealOutcomeMetadata,
    DealOutcomeStatus, DnaLifecycleError, DnaLifecycleStore, FingerprintGenerator,
    FingerprintSnapshot, SimilarDeal, SimilarityCandidate, SimilarityEngine,
};
pub use domain::approval::{ApprovalId, ApprovalRequest, ApprovalStatus};
pub use domain::autopsy::*;
pub use domain::execution::{
    ExecutionTask, ExecutionTaskId, ExecutionTaskState, ExecutionTransitionEvent,
    ExecutionTransitionId, IdempotencyRecord, IdempotencyRecordState, OperationKey,
};
pub use domain::explanation::*;
pub use domain::optimizer::*;
pub use domain::precedent::*;
pub use domain::product::{Product, ProductId};
pub use domain::quote::{Quote, QuoteId, QuoteLine, QuoteStatus};
pub use domain::simulation::*;
pub use errors::{ApplicationError, DomainError, InterfaceError};
pub use execution_engine::{
    ClaimResult, DeterministicExecutionEngine, ExecutionEngineConfig, ExecutionError,
    InMemoryExecutionEngine, RetryPolicy, TransitionResult,
};
pub use explanation::{
    AppliedRule, CalculationStep, ExplanationEngine, ExplanationError, InMemoryPolicyProvider,
    InMemoryPricingProvider, PolicyEvaluation, PolicyEvaluationProvider, PolicyViolation,
    PricingLineSnapshot, PricingSnapshot, PricingSnapshotProvider,
};
pub use ghost::{
    GhostQuote, GhostQuoteGenerator, InMemoryCustomerHistoryProvider, InMemoryGhostQuoteStore,
    Signal, SignalDetector, SignalDetectorConfig,
};
pub use ledger::{LedgerAction, LedgerEntry, LedgerService, VerificationResult};
pub use policy::optimizer::{
    BlastRadiusSummary, CandidateCohortScope, CandidateConfidenceBounds,
    CandidateDiffValidationError, CandidateGenerationError, CandidateGenerationRequest,
    CandidateProjectedImpact, CandidateProvenance, CandidateRuleDiff, CandidateRuleOperation,
    CandidateRuleSignal, GeneratedCandidatePackage, PolicyCandidateDiffV1,
    PolicyCandidateGenerator, PolicyReplayEngine, ReplayGuardrailBlock, ReplayGuardrailCode,
    ReplayGuardrailEvaluation, ReplayGuardrailThresholds, ReplayImpactError, ReplayImpactReport,
    ReplayImpactRequest, ReplayQuoteSnapshot,
};
pub use policy::{ExplanationGenerator, ExplanationTemplate, GeneratedExplanation};
pub use suggestions::{
    BusinessRule, BusinessRuleType, ComponentScores, ConfidenceLevel, CustomerProfile,
    CustomerSimilarity, ProductInfo, ProductRelationship, ProductSuggestion, QuoteContext,
    RelationshipType, ScoreCalculator, ScoringWeights, SeasonalPattern, SuggestionCategory,
    SuggestionEngine, SuggestionFeedback, SuggestionRequest,
};
