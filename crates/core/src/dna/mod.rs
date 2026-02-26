use std::cmp::Ordering;

use rust_decimal::Decimal;
use serde_json::{json, Value};
use thiserror::Error;

use crate::domain::quote::{Quote, QuoteLine, QuoteStatus};
use crate::flows::states::{FlowAction, FlowState, TransitionOutcome};

pub const FINGERPRINT_BITS: usize = 128;
pub const FINGERPRINT_BYTES: usize = FINGERPRINT_BITS / 8;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ConfigurationFingerprint {
    pub hash_hex: String,
    pub hash_bytes: [u8; FINGERPRINT_BYTES],
    pub vector: [u8; FINGERPRINT_BITS],
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DealOutcomeStatus {
    Won,
    Lost,
    Pending,
}

#[derive(Clone, Debug, PartialEq)]
pub struct DealOutcomeMetadata {
    pub quote_id: String,
    pub outcome_status: DealOutcomeStatus,
    pub final_price: Decimal,
    pub close_date: Option<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct SimilarityCandidate {
    pub fingerprint: ConfigurationFingerprint,
    pub outcome: DealOutcomeMetadata,
}

#[derive(Clone, Debug, PartialEq)]
pub struct SimilarDeal {
    pub outcome: DealOutcomeMetadata,
    pub similarity_score: f32,
    pub hamming_distance: usize,
}

#[derive(Clone, Debug, Default)]
pub struct FingerprintGenerator;

impl FingerprintGenerator {
    pub fn new() -> Self {
        Self
    }

    pub fn generate_from_json(&self, configuration: &Value) -> ConfigurationFingerprint {
        let mut features = Vec::new();
        collect_features(configuration, "$", &mut features);
        if features.is_empty() {
            features.push("$=empty".to_owned());
        }

        let mut accumulator = [0i32; FINGERPRINT_BITS];
        for feature in features {
            let (left, right) = feature_hashes(&feature);
            for bit in 0..64 {
                if (left >> bit) & 1 == 1 {
                    accumulator[bit] += 1;
                } else {
                    accumulator[bit] -= 1;
                }

                if (right >> bit) & 1 == 1 {
                    accumulator[bit + 64] += 1;
                } else {
                    accumulator[bit + 64] -= 1;
                }
            }
        }

        let mut hash_bytes = [0u8; FINGERPRINT_BYTES];
        let mut vector = [0u8; FINGERPRINT_BITS];
        for bit in 0..FINGERPRINT_BITS {
            let is_one = accumulator[bit] >= 0;
            vector[bit] = u8::from(is_one);
            if is_one {
                hash_bytes[bit / 8] |= 1 << (7 - (bit % 8));
            }
        }

        ConfigurationFingerprint { hash_hex: bytes_to_hex(&hash_bytes), hash_bytes, vector }
    }

    pub fn generate_from_lines(&self, lines: &[QuoteLine]) -> ConfigurationFingerprint {
        self.generate_from_json(&configuration_from_lines(lines))
    }

    pub fn generate_from_quote(&self, quote: &Quote) -> ConfigurationFingerprint {
        self.generate_from_lines(&quote.lines)
    }

    pub fn hamming_distance(
        &self,
        left: &ConfigurationFingerprint,
        right: &ConfigurationFingerprint,
    ) -> usize {
        left.vector.iter().zip(right.vector.iter()).filter(|(a, b)| a != b).count()
    }

    pub fn similarity_score(
        &self,
        left: &ConfigurationFingerprint,
        right: &ConfigurationFingerprint,
    ) -> f32 {
        let distance = self.hamming_distance(left, right) as f32;
        1.0 - distance / FINGERPRINT_BITS as f32
    }
}

#[derive(Clone, Debug)]
pub struct SimilarityEngine {
    min_similarity: f32,
    candidates: Vec<SimilarityCandidate>,
}

impl SimilarityEngine {
    const DEFAULT_MIN_SIMILARITY: f32 = 0.8;

    pub fn new(candidates: Vec<SimilarityCandidate>) -> Self {
        Self { min_similarity: Self::DEFAULT_MIN_SIMILARITY, candidates }
    }

    pub fn with_min_similarity(mut self, min_similarity: f32) -> Self {
        self.min_similarity = min_similarity.clamp(0.0, 1.0);
        self
    }

    pub fn min_similarity(&self) -> f32 {
        self.min_similarity
    }

    pub fn find_similar(
        &self,
        fingerprint: &ConfigurationFingerprint,
        limit: usize,
    ) -> Vec<SimilarDeal> {
        if limit == 0 {
            return Vec::new();
        }

        let generator = FingerprintGenerator::new();
        let mut matches: Vec<SimilarDeal> = self
            .candidates
            .iter()
            .filter_map(|candidate| {
                let hamming_distance =
                    generator.hamming_distance(fingerprint, &candidate.fingerprint);
                let similarity_score = 1.0 - hamming_distance as f32 / FINGERPRINT_BITS as f32;
                if similarity_score >= self.min_similarity {
                    Some(SimilarDeal {
                        outcome: candidate.outcome.clone(),
                        similarity_score,
                        hamming_distance,
                    })
                } else {
                    None
                }
            })
            .collect();

        matches.sort_by(|left, right| {
            right
                .similarity_score
                .partial_cmp(&left.similarity_score)
                .unwrap_or(Ordering::Equal)
                .then_with(|| left.hamming_distance.cmp(&right.hamming_distance))
                .then_with(|| left.outcome.quote_id.cmp(&right.outcome.quote_id))
        });

        if matches.len() > limit {
            matches.truncate(limit);
        }

        matches
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct FingerprintSnapshot {
    pub quote_id: String,
    pub fingerprint_hash: String,
    pub configuration_vector: Vec<u8>,
    pub final_price: Decimal,
    pub close_date: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ClosedDealOutcome {
    Won,
    Lost,
}

impl From<ClosedDealOutcome> for DealOutcomeStatus {
    fn from(value: ClosedDealOutcome) -> Self {
        match value {
            ClosedDealOutcome::Won => DealOutcomeStatus::Won,
            ClosedDealOutcome::Lost => DealOutcomeStatus::Lost,
        }
    }
}

pub trait DnaLifecycleStore {
    fn upsert_fingerprint_snapshot(&mut self, snapshot: FingerprintSnapshot) -> Result<(), String>;
    fn upsert_deal_outcome(&mut self, outcome: DealOutcomeMetadata) -> Result<(), String>;
}

#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum DnaLifecycleError {
    #[error("failed to persist fingerprint snapshot for quote {quote_id}: {details}")]
    PersistFingerprintSnapshot { quote_id: String, details: String },
    #[error("failed to persist deal outcome for quote {quote_id}: {details}")]
    PersistDealOutcome { quote_id: String, details: String },
}

#[derive(Clone, Debug, Default)]
pub struct DealDnaLifecycleService {
    generator: FingerprintGenerator,
}

impl DealDnaLifecycleService {
    pub fn new(generator: FingerprintGenerator) -> Self {
        Self { generator }
    }

    pub fn on_quote_closed<S: DnaLifecycleStore>(
        &self,
        quote: &Quote,
        close_date: Option<String>,
        store: &mut S,
    ) -> Result<FingerprintSnapshot, DnaLifecycleError> {
        let snapshot = self.snapshot_for(quote, close_date);
        store.upsert_fingerprint_snapshot(snapshot.clone()).map_err(|details| {
            DnaLifecycleError::PersistFingerprintSnapshot {
                quote_id: snapshot.quote_id.clone(),
                details,
            }
        })?;

        Ok(snapshot)
    }

    pub fn on_quote_reopened_or_modified<S: DnaLifecycleStore>(
        &self,
        quote: &Quote,
        store: &mut S,
    ) -> Result<FingerprintSnapshot, DnaLifecycleError> {
        self.on_quote_closed(quote, None, store)
    }

    pub fn record_closed_deal_outcome<S: DnaLifecycleStore>(
        &self,
        quote: &Quote,
        outcome: ClosedDealOutcome,
        close_date: String,
        store: &mut S,
    ) -> Result<DealOutcomeMetadata, DnaLifecycleError> {
        let metadata = DealOutcomeMetadata {
            quote_id: quote.id.0.clone(),
            outcome_status: outcome.into(),
            final_price: quote_total(quote),
            close_date: Some(close_date),
        };

        store.upsert_deal_outcome(metadata.clone()).map_err(|details| {
            DnaLifecycleError::PersistDealOutcome { quote_id: metadata.quote_id.clone(), details }
        })?;

        Ok(metadata)
    }

    pub fn backfill_historical_quotes<S: DnaLifecycleStore>(
        &self,
        quotes: &[Quote],
        store: &mut S,
    ) -> Result<usize, DnaLifecycleError> {
        let mut processed = 0;
        for quote in quotes {
            if is_closed_quote_status(&quote.status) {
                self.on_quote_closed(quote, None, store)?;
                processed += 1;
            }
        }
        Ok(processed)
    }

    pub fn on_flow_transition<S: DnaLifecycleStore>(
        &self,
        transition: &TransitionOutcome,
        quote: &Quote,
        store: &mut S,
    ) -> Result<Option<FingerprintSnapshot>, DnaLifecycleError> {
        if !transition.actions.contains(&FlowAction::GenerateConfigurationFingerprint) {
            return Ok(None);
        }

        match transition.to {
            FlowState::Finalized | FlowState::Sent => {
                self.on_quote_closed(quote, None, store).map(Some)
            }
            FlowState::Revised => self.on_quote_reopened_or_modified(quote, store).map(Some),
            _ => Ok(None),
        }
    }

    fn snapshot_for(&self, quote: &Quote, close_date: Option<String>) -> FingerprintSnapshot {
        let fingerprint = self.generator.generate_from_quote(quote);
        FingerprintSnapshot {
            quote_id: quote.id.0.clone(),
            fingerprint_hash: fingerprint.hash_hex,
            configuration_vector: fingerprint.hash_bytes.to_vec(),
            final_price: quote_total(quote),
            close_date,
        }
    }
}

pub fn configuration_from_lines(lines: &[QuoteLine]) -> Value {
    let mut line_entries: Vec<(String, u32, String)> = lines
        .iter()
        .map(|line| {
            (line.product_id.0.clone(), line.quantity, line.unit_price.normalize().to_string())
        })
        .collect();
    line_entries.sort();

    json!({
        "lines": line_entries
            .into_iter()
            .map(|(product_id, quantity, unit_price)| {
                json!({
                    "product_id": product_id,
                    "quantity": quantity,
                    "unit_price": unit_price,
                })
            })
            .collect::<Vec<_>>()
    })
}

fn collect_features(value: &Value, path: &str, output: &mut Vec<String>) {
    match value {
        Value::Null => output.push(format!("{path}=null")),
        Value::Bool(v) => output.push(format!("{path}=bool:{v}")),
        Value::Number(v) => output.push(format!("{path}=number:{v}")),
        Value::String(v) => output.push(format!("{path}=string:{v}")),
        Value::Array(values) => {
            for (index, item) in values.iter().enumerate() {
                collect_features(item, &format!("{path}[{index}]"), output);
            }
        }
        Value::Object(values) => {
            let mut keys: Vec<&str> = values.keys().map(String::as_str).collect();
            keys.sort_unstable();
            for key in keys {
                if let Some(item) = values.get(key) {
                    collect_features(item, &format!("{path}.{key}"), output);
                }
            }
        }
    }
}

const FNV_OFFSET_BASIS: u64 = 0xcbf2_9ce4_8422_2325;
const FNV_PRIME: u64 = 0x1000_0000_01b3;
const LEFT_SEED: u64 = 0x9e37_79b1_85eb_ca87;
const RIGHT_SEED: u64 = 0xc2b2_ae3d_27d4_eb4f;

fn feature_hashes(feature: &str) -> (u64, u64) {
    (
        fnv1a_64_with_seed(feature.as_bytes(), LEFT_SEED),
        fnv1a_64_with_seed(feature.as_bytes(), RIGHT_SEED),
    )
}

fn fnv1a_64_with_seed(bytes: &[u8], seed: u64) -> u64 {
    let mut hash = FNV_OFFSET_BASIS ^ seed;
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(FNV_PRIME);
    }
    hash
}

fn bytes_to_hex(bytes: &[u8; FINGERPRINT_BYTES]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn is_closed_quote_status(status: &QuoteStatus) -> bool {
    matches!(status, QuoteStatus::Finalized | QuoteStatus::Sent)
}

fn quote_total(quote: &Quote) -> Decimal {
    quote.lines.iter().fold(Decimal::ZERO, |acc, line| {
        acc + line.unit_price * Decimal::from(u64::from(line.quantity))
    })
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, Instant};

    use chrono::Utc;
    use rust_decimal::Decimal;
    use serde_json::json;

    use crate::domain::{
        product::ProductId,
        quote::{Quote, QuoteId, QuoteLine, QuoteStatus},
    };
    use crate::flows::{
        engine::FlowEngine,
        states::{FlowContext, FlowEvent, FlowState},
    };

    use super::{
        configuration_from_lines, ClosedDealOutcome, DealDnaLifecycleService, DealOutcomeMetadata,
        DealOutcomeStatus, DnaLifecycleStore, FingerprintGenerator, SimilarityCandidate,
        SimilarityEngine, FINGERPRINT_BITS, FINGERPRINT_BYTES,
    };

    #[test]
    fn generates_128_bit_fingerprint_and_vector() {
        let generator = FingerprintGenerator::new();
        let fingerprint = generator.generate_from_json(&json!({
            "account_tier": "enterprise",
            "products": [
                { "id": "plan-enterprise", "qty": 50 },
                { "id": "support-premium", "qty": 1 }
            ],
            "discount_pct": 15
        }));

        assert_eq!(fingerprint.hash_bytes.len(), FINGERPRINT_BYTES);
        assert_eq!(fingerprint.hash_hex.len(), FINGERPRINT_BYTES * 2);
        assert_eq!(fingerprint.vector.len(), FINGERPRINT_BITS);
    }

    #[test]
    fn same_configuration_is_deterministic_even_with_key_reordering() {
        let generator = FingerprintGenerator::new();

        let left = generator.generate_from_json(&json!({
            "discount_pct": 20,
            "products": [{"id": "plan-pro", "qty": 25}],
            "account_tier": "mid-market"
        }));
        let right = generator.generate_from_json(&json!({
            "account_tier": "mid-market",
            "products": [{"qty": 25, "id": "plan-pro"}],
            "discount_pct": 20
        }));

        assert_eq!(left, right);
        assert_eq!(generator.hamming_distance(&left, &right), 0);
    }

    #[test]
    fn quote_line_order_does_not_change_fingerprint() {
        let generator = FingerprintGenerator::new();
        let quote_a = quote_fixture(vec![
            QuoteLine {
                product_id: ProductId("plan-enterprise".to_owned()),
                quantity: 100,
                unit_price: Decimal::new(15_000, 2),
                discount_pct: 0.0,
                notes: None,
            },
            QuoteLine {
                product_id: ProductId("support-premium".to_owned()),
                quantity: 1,
                unit_price: Decimal::new(2_500, 2),
                discount_pct: 0.0,
                notes: None,
            },
        ]);
        let quote_b = quote_fixture(vec![
            QuoteLine {
                product_id: ProductId("support-premium".to_owned()),
                quantity: 1,
                unit_price: Decimal::new(2_500, 2),
                discount_pct: 0.0,
                notes: None,
            },
            QuoteLine {
                product_id: ProductId("plan-enterprise".to_owned()),
                quantity: 100,
                unit_price: Decimal::new(15_000, 2),
                discount_pct: 0.0,
                notes: None,
            },
        ]);

        let left = generator.generate_from_quote(&quote_a);
        let right = generator.generate_from_quote(&quote_b);
        assert_eq!(left, right);
    }

    #[test]
    fn similar_configurations_have_closer_signatures_than_distant_ones() {
        let generator = FingerprintGenerator::new();
        let base = generator.generate_from_json(&json!({
            "account_tier": "enterprise",
            "products": [
                { "id": "plan-enterprise", "qty": 100 },
                { "id": "support-premium", "qty": 1 }
            ],
            "term_months": 24,
            "discount_pct": 18
        }));
        let similar = generator.generate_from_json(&json!({
            "account_tier": "enterprise",
            "products": [
                { "id": "plan-enterprise", "qty": 105 },
                { "id": "support-premium", "qty": 1 }
            ],
            "term_months": 24,
            "discount_pct": 19
        }));
        let distant = generator.generate_from_json(&json!({
            "account_tier": "smb",
            "products": [
                { "id": "starter", "qty": 5 },
                { "id": "support-basic", "qty": 1 }
            ],
            "term_months": 12,
            "discount_pct": 0
        }));

        let similar_distance = generator.hamming_distance(&base, &similar);
        let distant_distance = generator.hamming_distance(&base, &distant);

        assert!(similar_distance < distant_distance);
        assert!(
            generator.similarity_score(&base, &similar)
                > generator.similarity_score(&base, &distant)
        );
    }

    #[test]
    fn canonical_line_configuration_is_stable() {
        let lines = vec![
            QuoteLine {
                product_id: ProductId("support-premium".to_owned()),
                quantity: 1,
                unit_price: Decimal::new(2_500, 2),
                discount_pct: 0.0,
                notes: None,
            },
            QuoteLine {
                product_id: ProductId("plan-enterprise".to_owned()),
                quantity: 100,
                unit_price: Decimal::new(15_000, 2),
                discount_pct: 0.0,
                notes: None,
            },
        ];
        let configuration = configuration_from_lines(&lines);
        assert_eq!(
            configuration,
            json!({
                "lines": [
                    {
                        "product_id": "plan-enterprise",
                        "quantity": 100,
                        "unit_price": "150"
                    },
                    {
                        "product_id": "support-premium",
                        "quantity": 1,
                        "unit_price": "25"
                    }
                ]
            })
        );
    }

    #[test]
    fn similarity_engine_default_threshold_is_point_eight() {
        let engine = SimilarityEngine::new(Vec::new());
        assert!((engine.min_similarity() - 0.8).abs() < f32::EPSILON);
    }

    #[test]
    fn find_similar_returns_ranked_matches_by_similarity() {
        let generator = FingerprintGenerator::new();
        let target = generator.generate_from_json(&json!({
            "account_tier": "enterprise",
            "products": [{"id": "plan-enterprise", "qty": 100}],
            "term_months": 24
        }));
        let near = generator.generate_from_json(&json!({
            "account_tier": "enterprise",
            "products": [{"id": "plan-enterprise", "qty": 102}],
            "term_months": 24
        }));
        let far = generator.generate_from_json(&json!({
            "account_tier": "smb",
            "products": [{"id": "starter", "qty": 5}],
            "term_months": 12
        }));

        let engine = SimilarityEngine::new(vec![
            SimilarityCandidate {
                fingerprint: target.clone(),
                outcome: outcome("Q-EXACT", DealOutcomeStatus::Won, Decimal::new(120_000, 2)),
            },
            SimilarityCandidate {
                fingerprint: near,
                outcome: outcome("Q-NEAR", DealOutcomeStatus::Won, Decimal::new(118_000, 2)),
            },
            SimilarityCandidate {
                fingerprint: far,
                outcome: outcome("Q-FAR", DealOutcomeStatus::Lost, Decimal::new(9_500, 2)),
            },
        ])
        .with_min_similarity(0.0);

        let results = engine.find_similar(&target, 2);
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].outcome.quote_id, "Q-EXACT");
        assert_eq!(results[1].outcome.quote_id, "Q-NEAR");
        assert!(results[0].similarity_score >= results[1].similarity_score);
    }

    #[test]
    fn find_similar_respects_threshold_filtering() {
        let generator = FingerprintGenerator::new();
        let target = generator.generate_from_json(&json!({
            "account_tier": "enterprise",
            "products": [{"id": "plan-enterprise", "qty": 100}],
        }));
        let far = generator.generate_from_json(&json!({
            "account_tier": "smb",
            "products": [{"id": "starter", "qty": 2}],
        }));

        let engine = SimilarityEngine::new(vec![
            SimilarityCandidate {
                fingerprint: target.clone(),
                outcome: outcome("Q-EXACT", DealOutcomeStatus::Won, Decimal::new(150_000, 2)),
            },
            SimilarityCandidate {
                fingerprint: far,
                outcome: outcome("Q-FAR", DealOutcomeStatus::Lost, Decimal::new(5_000, 2)),
            },
        ])
        .with_min_similarity(0.9);

        let results = engine.find_similar(&target, 5);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].outcome.quote_id, "Q-EXACT");
    }

    #[test]
    fn similarity_query_scales_to_ten_thousand_candidates_under_budget() {
        let generator = FingerprintGenerator::new();
        let target = generator.generate_from_json(&json!({
            "account_tier": "enterprise",
            "products": [{"id": "plan-enterprise", "qty": 100}],
            "term_months": 24
        }));

        let candidates = (0..10_000)
            .map(|index| SimilarityCandidate {
                fingerprint: generator.generate_from_json(&json!({
                    "account_tier": "enterprise",
                    "products": [{"id": "plan-enterprise", "qty": 100 + (index % 3)}],
                    "term_months": 24
                })),
                outcome: outcome(
                    &format!("Q-{index}"),
                    DealOutcomeStatus::Pending,
                    Decimal::new(100_000 + index as i64, 2),
                ),
            })
            .collect();

        let engine = SimilarityEngine::new(candidates).with_min_similarity(0.0);

        let start = Instant::now();
        let results = engine.find_similar(&target, 5);
        let elapsed = start.elapsed();

        assert_eq!(results.len(), 5);
        assert!(
            elapsed < Duration::from_millis(100),
            "expected similarity lookup under 100ms for 10k candidates, got {:?}",
            elapsed
        );
    }

    /// qa-tag: fake-in-memory-critical-path (bd-3vp2.3.1)
    #[test]
    fn lifecycle_finalization_generates_and_stores_fingerprint_snapshot() {
        let service = DealDnaLifecycleService::default();
        let mut store = InMemoryLifecycleStore::default();
        let flow_engine = FlowEngine::default();
        let quote = quote_with_status(
            "Q-2026-CLOSE-1",
            QuoteStatus::Priced,
            vec![QuoteLine {
                product_id: ProductId("plan-enterprise".to_owned()),
                quantity: 10,
                unit_price: Decimal::new(15_000, 2),
                discount_pct: 0.0,
                notes: None,
            }],
        );
        let transition = flow_engine
            .apply(&FlowState::Priced, &FlowEvent::PolicyClear, &FlowContext::default())
            .expect("priced->finalized should succeed");

        let snapshot = service
            .on_flow_transition(&transition, &quote, &mut store)
            .expect("finalized quote should generate fingerprint snapshot");
        let snapshot = snapshot.expect("finalized transition should trigger fingerprint storage");

        assert_eq!(snapshot.quote_id, "Q-2026-CLOSE-1");
        assert!(snapshot.close_date.is_none());
        assert_eq!(store.snapshots.len(), 1);
        assert_eq!(store.snapshots[0], snapshot);
    }

    /// qa-tag: fake-in-memory-critical-path (bd-3vp2.3.1)
    #[test]
    fn lifecycle_reopened_quote_updates_existing_fingerprint_snapshot() {
        let service = DealDnaLifecycleService::default();
        let mut store = InMemoryLifecycleStore::default();

        let original = quote_with_status(
            "Q-2026-REOPEN-1",
            QuoteStatus::Finalized,
            vec![QuoteLine {
                product_id: ProductId("plan-pro".to_owned()),
                quantity: 5,
                unit_price: Decimal::new(12_000, 2),
                discount_pct: 0.0,
                notes: None,
            }],
        );
        let updated = quote_with_status(
            "Q-2026-REOPEN-1",
            QuoteStatus::Revised,
            vec![QuoteLine {
                product_id: ProductId("plan-pro".to_owned()),
                quantity: 7,
                unit_price: Decimal::new(12_000, 2),
                discount_pct: 0.0,
                notes: None,
            }],
        );

        let original_snapshot = service
            .on_quote_closed(&original, Some("2026-02-20".to_owned()), &mut store)
            .expect("closed quote should generate snapshot");
        let refreshed_snapshot = service
            .on_quote_reopened_or_modified(&updated, &mut store)
            .expect("reopened quote should refresh snapshot");

        assert_eq!(store.snapshots.len(), 1);
        assert_ne!(original_snapshot.fingerprint_hash, refreshed_snapshot.fingerprint_hash);
        assert_eq!(store.snapshots[0].final_price, Decimal::new(84_000, 2));
    }

    /// qa-tag: fake-in-memory-critical-path (bd-3vp2.3.1)
    #[test]
    fn lifecycle_records_won_or_lost_deal_outcomes() {
        let service = DealDnaLifecycleService::default();
        let mut store = InMemoryLifecycleStore::default();
        let quote = quote_with_status(
            "Q-2026-OUTCOME-1",
            QuoteStatus::Sent,
            vec![QuoteLine {
                product_id: ProductId("plan-growth".to_owned()),
                quantity: 3,
                unit_price: Decimal::new(8_500, 2),
                discount_pct: 0.0,
                notes: None,
            }],
        );

        let won = service
            .record_closed_deal_outcome(
                &quote,
                ClosedDealOutcome::Won,
                "2026-02-21".to_owned(),
                &mut store,
            )
            .expect("won outcome should persist");
        let lost = service
            .record_closed_deal_outcome(
                &quote,
                ClosedDealOutcome::Lost,
                "2026-02-22".to_owned(),
                &mut store,
            )
            .expect("lost outcome should upsert for quote");

        assert_eq!(won.outcome_status, DealOutcomeStatus::Won);
        assert_eq!(lost.outcome_status, DealOutcomeStatus::Lost);
        assert_eq!(store.outcomes.len(), 1);
        assert_eq!(store.outcomes[0].close_date.as_deref(), Some("2026-02-22"));
    }

    /// qa-tag: fake-in-memory-critical-path (bd-3vp2.3.1)
    #[test]
    fn lifecycle_backfill_processes_only_closed_quotes() {
        let service = DealDnaLifecycleService::default();
        let mut store = InMemoryLifecycleStore::default();
        let quotes = vec![
            quote_with_status(
                "Q-2026-BACKFILL-1",
                QuoteStatus::Finalized,
                vec![QuoteLine {
                    product_id: ProductId("plan-pro".to_owned()),
                    quantity: 2,
                    unit_price: Decimal::new(5_000, 2),
                    discount_pct: 0.0,
                    notes: None,
                }],
            ),
            quote_with_status(
                "Q-2026-BACKFILL-2",
                QuoteStatus::Draft,
                vec![QuoteLine {
                    product_id: ProductId("plan-pro".to_owned()),
                    quantity: 2,
                    unit_price: Decimal::new(5_000, 2),
                    discount_pct: 0.0,
                    notes: None,
                }],
            ),
            quote_with_status(
                "Q-2026-BACKFILL-3",
                QuoteStatus::Sent,
                vec![QuoteLine {
                    product_id: ProductId("plan-enterprise".to_owned()),
                    quantity: 1,
                    unit_price: Decimal::new(25_000, 2),
                    discount_pct: 0.0,
                    notes: None,
                }],
            ),
        ];

        let count = service
            .backfill_historical_quotes(&quotes, &mut store)
            .expect("backfill should succeed");
        assert_eq!(count, 2);
        assert_eq!(store.snapshots.len(), 2);
        assert!(store.snapshots.iter().all(|snapshot| snapshot.quote_id == "Q-2026-BACKFILL-1"
            || snapshot.quote_id == "Q-2026-BACKFILL-3"));
    }

    fn quote_fixture(lines: Vec<QuoteLine>) -> Quote {
        quote_with_status("Q-2026-2001", QuoteStatus::Draft, lines)
    }

    fn quote_with_status(id: &str, status: QuoteStatus, lines: Vec<QuoteLine>) -> Quote {
        let now = Utc::now();
        Quote {
            id: QuoteId(id.to_owned()),
            version: 1,
            status,
            account_id: None,
            deal_id: None,
            currency: "USD".to_string(),
            term_months: None,
            start_date: None,
            end_date: None,
            valid_until: None,
            notes: None,
            created_by: "system".to_string(),
            lines,
            created_at: now,
            updated_at: now,
        }
    }

    fn outcome(
        quote_id: &str,
        status: DealOutcomeStatus,
        final_price: Decimal,
    ) -> DealOutcomeMetadata {
        DealOutcomeMetadata {
            quote_id: quote_id.to_owned(),
            outcome_status: status,
            final_price,
            close_date: None,
        }
    }

    #[derive(Default)]
    struct InMemoryLifecycleStore {
        snapshots: Vec<super::FingerprintSnapshot>,
        outcomes: Vec<DealOutcomeMetadata>,
    }

    impl DnaLifecycleStore for InMemoryLifecycleStore {
        fn upsert_fingerprint_snapshot(
            &mut self,
            snapshot: super::FingerprintSnapshot,
        ) -> Result<(), String> {
            if let Some(index) =
                self.snapshots.iter().position(|existing| existing.quote_id == snapshot.quote_id)
            {
                self.snapshots[index] = snapshot;
            } else {
                self.snapshots.push(snapshot);
            }
            Ok(())
        }

        fn upsert_deal_outcome(&mut self, outcome: DealOutcomeMetadata) -> Result<(), String> {
            if let Some(index) =
                self.outcomes.iter().position(|existing| existing.quote_id == outcome.quote_id)
            {
                self.outcomes[index] = outcome;
            } else {
                self.outcomes.push(outcome);
            }
            Ok(())
        }
    }
}
