// Re-export chrono for downstream crates that need DateTime types
pub use chrono;

pub mod ambiguity;
pub use ambiguity::{
    ambiguity_set_to_card_set, render_ambiguity_slack_blocks,
    render_assumption_card_set_slack_blocks, render_assumption_card_slack_blocks, Ambiguity,
    AmbiguityDetectionEngine, AmbiguityDetectionInput, AmbiguityOption, AmbiguitySet,
    AmbiguitySeverity, AmbiguityType, AssumptionCard, AssumptionCardSet, AssumptionCategory,
    AssumptionState, DateMention, ProductMention, QuantityMention,
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
pub use cpq::constraint_rule_builder::{
    build_constraint_rule, ConstraintRuleAction, ConstraintRuleBuilderError,
    ConstraintRuleCondition, ConstraintRuleDraft, ConstraintRuleOperator,
};
pub use cpq::discount_policy_builder::{
    build_discount_policy, DiscountPolicyBuilderError, DiscountPolicyDraft,
};
pub use cpq::draft_quote_builder::{
    DraftQuoteBuildError, DraftQuoteBuildRequest, DraftQuoteBuildResult, DraftQuoteBuilder,
};
pub use cpq::product_matcher::{MatchAmbiguity, ProductMatch, ProductMatchResult, ProductMatcher};
pub use cpq::rule_builder::{
    build_pricing_rule, preview_pricing_rule, pricing_rule_sql_preview, PricingRuleAction,
    PricingRuleBuilderError, PricingRuleCondition, PricingRuleDraft, PricingRuleOperator,
    PricingRulePreviewCase, PricingRulePreviewInput, PricingRulePreviewResult,
};
pub use dna::{
    ClosedDealOutcome, ConfigurationFingerprint, DealDnaLifecycleService, DealOutcomeMetadata,
    DealOutcomeStatus, DnaLifecycleError, DnaLifecycleStore, FingerprintGenerator,
    FingerprintSnapshot, SimilarDeal, SimilarityCandidate, SimilarityEngine,
};
pub use domain::analytics::{
    AnalyticsContractError, AnalyticsQuerySpec, DimensionKind, MetricKind, ANALYTICS_SCHEMA_VERSION,
};
pub use domain::approval::{ApprovalId, ApprovalRequest, ApprovalStatus};
pub use domain::auth::{
    AuthChannel, AuthContext, AuthError, AuthErrorCode, AuthMethod, AuthPrincipal, AuthStrength,
};
pub use domain::autopsy::*;
pub use domain::execution::{
    ExecutionTask, ExecutionTaskId, ExecutionTaskState, ExecutionTransitionEvent,
    ExecutionTransitionId, IdempotencyRecord, IdempotencyRecordState, OperationKey,
};
pub use domain::explanation::*;
pub use domain::negotiation::{
    BoundaryEvaluation, ConcessionEnvelope, ConcessionRange, CounterofferAlternative,
    CounterofferPlan, NegotiationSession, NegotiationSessionId, NegotiationState, NegotiationTurn,
    NegotiationTurnId, TurnOutcome, TurnRequestType,
};
pub use domain::optimizer::*;
pub use domain::precedent::*;
pub use domain::product::{Product, ProductId};
pub use domain::quote::{Quote, QuoteId, QuoteLine, QuoteStatus};
pub use domain::quote_lock::{LockConflict, LockInfo};
pub use domain::requirement_extraction::{
    ExtractedRequirement, ExtractedRequirements, RequirementAmbiguity,
    RequirementExtractionValidationError, RequirementSourceType,
    REQUIREMENT_EXTRACTION_SCHEMA_VERSION,
};
pub use domain::sales_rep::{SalesRep, SalesRepId, SalesRepRole, SalesRepStatus};
pub use domain::simulation::*;
pub use domain::visual_rule::{
    LogicalConnector, VisualActionType, VisualOperator, VisualRuleAction, VisualRuleCondition,
    VisualRuleDefinition, VisualRuleMetadata, VisualRuleType, VisualRuleValidationError,
    VISUAL_RULE_SCHEMA_VERSION,
};
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
    CustomerSimilarity, FeedbackExperimentVariant, ProductInfo, ProductRelationship,
    ProductSuggestion, QuoteContext, RelationshipType, ScoreCalculator, ScoringWeights,
    SeasonalPattern, SuggestionCategory, SuggestionEngine, SuggestionFeedback, SuggestionRequest,
};
