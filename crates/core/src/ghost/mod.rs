use std::collections::{BTreeSet, HashMap};

use chrono::Utc;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

use crate::dna::{
    DealOutcomeMetadata, DealOutcomeStatus, FingerprintGenerator, SimilarDeal, SimilarityCandidate,
    SimilarityEngine,
};
use crate::domain::quote::{Quote, QuoteId, QuoteStatus};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SignalDetectorConfig {
    pub confidence_threshold: u8,
    pub buying_intent_keywords: Vec<String>,
    pub competitor_keywords: Vec<String>,
}

impl Default for SignalDetectorConfig {
    fn default() -> Self {
        Self {
            confidence_threshold: 70,
            buying_intent_keywords: vec![
                "budget".to_string(),
                "expand".to_string(),
                "looking for".to_string(),
                "evaluating".to_string(),
                "pricing".to_string(),
                "quote".to_string(),
                "renewal".to_string(),
                "upgrade".to_string(),
                "rollout".to_string(),
            ],
            competitor_keywords: vec![
                "salesforce".to_string(),
                "hubspot".to_string(),
                "oracle".to_string(),
                "sap".to_string(),
                "dealhub".to_string(),
                "conga".to_string(),
                "pros".to_string(),
                "tacton".to_string(),
            ],
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Signal {
    pub confidence: u8,
    pub keyword_matches: Vec<String>,
    pub companies: Vec<String>,
    pub departments: Vec<String>,
    pub timelines: Vec<String>,
    pub competitors: Vec<String>,
    pub above_threshold: bool,
}

#[derive(Clone, Debug, Default)]
pub struct SignalDetector {
    config: SignalDetectorConfig,
}

impl SignalDetector {
    pub fn new(config: SignalDetectorConfig) -> Self {
        Self { config }
    }

    pub fn config(&self) -> &SignalDetectorConfig {
        &self.config
    }

    pub fn analyze(&self, message: &str) -> Signal {
        let normalized = message.to_ascii_lowercase();
        let keyword_matches = self.detect_keywords(&normalized);
        let companies = extract_companies(message);
        let departments = extract_departments(&normalized);
        let timelines = extract_timelines(&normalized);
        let competitors = self.detect_competitors(&normalized);

        let confidence = score_confidence(
            keyword_matches.len(),
            companies.len(),
            departments.len(),
            timelines.len(),
            competitors.len(),
        );

        Signal {
            confidence,
            keyword_matches,
            companies,
            departments,
            timelines,
            competitors,
            above_threshold: confidence >= self.config.confidence_threshold,
        }
    }

    pub fn detect(&self, message: &str) -> Option<Signal> {
        let signal = self.analyze(message);
        signal.above_threshold.then_some(signal)
    }

    fn detect_keywords(&self, normalized_message: &str) -> Vec<String> {
        self.config
            .buying_intent_keywords
            .iter()
            .filter(|keyword| normalized_message.contains(keyword.as_str()))
            .cloned()
            .collect()
    }

    fn detect_competitors(&self, normalized_message: &str) -> Vec<String> {
        self.config
            .competitor_keywords
            .iter()
            .filter(|keyword| normalized_message.contains(keyword.as_str()))
            .cloned()
            .collect()
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct GhostQuote {
    pub company: String,
    pub draft_quote: Quote,
    pub confidence: u8,
    pub suggested_discount_pct: u8,
    pub similar_quote_id: Option<String>,
}

pub trait CustomerHistoryProvider {
    fn history_for_company(&self, company: &str) -> Option<Vec<Quote>>;
}

pub trait GhostQuoteStore {
    fn save_draft(&mut self, quote: Quote) -> Result<(), String>;
}

#[derive(Clone, Debug, Default)]
pub struct InMemoryCustomerHistoryProvider {
    histories: HashMap<String, Vec<Quote>>,
}

impl InMemoryCustomerHistoryProvider {
    pub fn insert_history(&mut self, company: &str, history: Vec<Quote>) {
        self.histories.insert(company.to_string(), history);
    }
}

impl CustomerHistoryProvider for InMemoryCustomerHistoryProvider {
    fn history_for_company(&self, company: &str) -> Option<Vec<Quote>> {
        self.histories.get(company).cloned()
    }
}

#[derive(Clone, Debug, Default)]
pub struct InMemoryGhostQuoteStore {
    drafts: Vec<Quote>,
}

impl InMemoryGhostQuoteStore {
    pub fn drafts(&self) -> &[Quote] {
        &self.drafts
    }
}

impl GhostQuoteStore for InMemoryGhostQuoteStore {
    fn save_draft(&mut self, quote: Quote) -> Result<(), String> {
        self.drafts.push(quote);
        Ok(())
    }
}

#[derive(Clone, Debug)]
pub struct GhostQuoteGenerator {
    min_signal_confidence: u8,
    similarity_floor: f32,
}

impl Default for GhostQuoteGenerator {
    fn default() -> Self {
        Self { min_signal_confidence: 70, similarity_floor: 0.6 }
    }
}

impl GhostQuoteGenerator {
    pub fn new(min_signal_confidence: u8) -> Self {
        Self { min_signal_confidence, ..Self::default() }
    }

    pub fn generate<P, S>(
        &self,
        quote_id: &str,
        signal: &Signal,
        history_provider: &P,
        store: &mut S,
    ) -> Result<Option<GhostQuote>, String>
    where
        P: CustomerHistoryProvider,
        S: GhostQuoteStore,
    {
        if !signal.above_threshold || signal.confidence < self.min_signal_confidence {
            return Ok(None);
        }

        let Some(company) = signal.companies.first() else {
            return Ok(None);
        };
        let Some(history) = history_provider.history_for_company(company) else {
            return Ok(None);
        };
        let Some(template_quote) = history.last() else {
            return Ok(None);
        };

        let similar_deal = self.best_similar_deal(&history, template_quote);
        let suggested_discount_pct = suggested_discount_pct(signal, similar_deal.as_ref());
        let draft_quote = build_draft_quote(template_quote, quote_id, suggested_discount_pct);
        store.save_draft(draft_quote.clone())?;

        Ok(Some(GhostQuote {
            company: company.clone(),
            confidence: combined_confidence(signal.confidence, similar_deal.as_ref()),
            suggested_discount_pct,
            similar_quote_id: similar_deal.map(|deal| deal.outcome.quote_id),
            draft_quote,
        }))
    }

    fn best_similar_deal(&self, history: &[Quote], reference_quote: &Quote) -> Option<SimilarDeal> {
        let fingerprint_generator = FingerprintGenerator::new();
        let reference_fingerprint = fingerprint_generator.generate_from_quote(reference_quote);
        let candidates = history
            .iter()
            .filter(|quote| quote.id != reference_quote.id)
            .map(|quote| SimilarityCandidate {
                fingerprint: fingerprint_generator.generate_from_quote(quote),
                outcome: DealOutcomeMetadata {
                    quote_id: quote.id.0.clone(),
                    outcome_status: DealOutcomeStatus::Won,
                    final_price: quote_total(quote),
                    close_date: None,
                },
            })
            .collect::<Vec<_>>();

        let similarity_engine =
            SimilarityEngine::new(candidates).with_min_similarity(self.similarity_floor);
        similarity_engine.find_similar(&reference_fingerprint, 1).into_iter().next()
    }
}

fn build_draft_quote(template_quote: &Quote, quote_id: &str, discount_pct: u8) -> Quote {
    let discount_multiplier = Decimal::ONE - (Decimal::from(discount_pct) / Decimal::from(100u8));
    let discounted_lines = template_quote
        .lines
        .iter()
        .map(|line| {
            let mut updated = line.clone();
            updated.unit_price = (updated.unit_price * discount_multiplier).round_dp(2);
            updated
        })
        .collect::<Vec<_>>();

    Quote {
        id: QuoteId(quote_id.to_string()),
        status: QuoteStatus::Draft,
        lines: discounted_lines,
        created_at: Utc::now(),
    }
}

fn suggested_discount_pct(signal: &Signal, similar_deal: Option<&SimilarDeal>) -> u8 {
    let mut discount = 0u8;
    if signal.keyword_matches.iter().any(|keyword| keyword == "expand" || keyword == "upgrade") {
        discount = discount.saturating_add(10);
    }
    if !signal.competitors.is_empty() {
        discount = discount.saturating_add(5);
    }
    if !signal.timelines.is_empty() {
        discount = discount.saturating_add(3);
    }
    if similar_deal.is_some_and(|deal| deal.similarity_score >= 0.9) {
        discount = discount.saturating_add(5);
    }
    discount.min(25)
}

fn combined_confidence(base_confidence: u8, similar_deal: Option<&SimilarDeal>) -> u8 {
    let boost = similar_deal.map(|deal| (deal.similarity_score * 20.0).round() as u8).unwrap_or(0);
    base_confidence.saturating_add(boost).min(100)
}

fn quote_total(quote: &Quote) -> Decimal {
    quote.lines.iter().map(|line| line.unit_price * Decimal::from(line.quantity)).sum()
}

fn score_confidence(
    keyword_count: usize,
    company_count: usize,
    department_count: usize,
    timeline_count: usize,
    competitor_count: usize,
) -> u8 {
    let keyword_score = u32::try_from(keyword_count).unwrap_or(u32::MAX).min(3) * 15;
    let company_score = u32::try_from(company_count).unwrap_or(u32::MAX).min(2) * 20;
    let department_score = u32::try_from(department_count).unwrap_or(u32::MAX).min(2) * 10;
    let timeline_score = u32::try_from(timeline_count).unwrap_or(u32::MAX).min(2) * 10;
    let competitor_score = u32::try_from(competitor_count).unwrap_or(u32::MAX).min(2) * 15;
    let total =
        5 + keyword_score + company_score + department_score + timeline_score + competitor_score;

    u8::try_from(total.min(100)).unwrap_or(100)
}

fn extract_companies(message: &str) -> Vec<String> {
    let company_suffixes = ["inc", "corp", "llc", "ltd", "gmbh"];
    let mut companies = BTreeSet::new();
    let tokens = message.split_whitespace().collect::<Vec<_>>();

    for window in tokens.windows(2) {
        if let [name, suffix] = window {
            let suffix_normalized = suffix
                .trim_matches(|character: char| !character.is_ascii_alphabetic())
                .to_ascii_lowercase();
            if company_suffixes.contains(&suffix_normalized.as_str()) {
                let company = format!(
                    "{} {}",
                    name.trim_matches(|character: char| !character.is_ascii_alphanumeric()),
                    suffix.trim_matches(|character: char| !character.is_ascii_alphabetic())
                );
                companies.insert(company.trim().to_string());
            }
        }
    }

    companies.into_iter().collect()
}

fn extract_departments(normalized_message: &str) -> Vec<String> {
    let known_departments = [
        "finance",
        "hr",
        "operations",
        "sales",
        "marketing",
        "engineering",
        "support",
        "it",
        "procurement",
    ];
    let mut departments = BTreeSet::new();

    for department in known_departments {
        if normalized_message.contains(department) {
            departments.insert(department.to_string());
        }
    }

    if normalized_message.contains("department") || normalized_message.contains("departments") {
        departments.insert("departments".to_string());
    }
    if normalized_message.contains("team") || normalized_message.contains("teams") {
        departments.insert("teams".to_string());
    }

    departments.into_iter().collect()
}

fn extract_timelines(normalized_message: &str) -> Vec<String> {
    let timeline_patterns = [
        "this quarter",
        "next quarter",
        "q1",
        "q2",
        "q3",
        "q4",
        "this month",
        "next month",
        "this year",
        "next year",
        "by friday",
        "by monday",
        "eom",
        "eoq",
    ];

    timeline_patterns
        .iter()
        .filter(|pattern| normalized_message.contains(**pattern))
        .map(|pattern| (*pattern).to_string())
        .collect()
}

pub fn detect_signals(detector: &SignalDetector, messages: &[String]) -> HashMap<String, Signal> {
    messages
        .iter()
        .filter_map(|message| detector.detect(message).map(|signal| (message.clone(), signal)))
        .collect()
}

#[cfg(test)]
mod tests {
    use chrono::Utc;
    use rust_decimal::Decimal;

    use crate::domain::{
        product::ProductId,
        quote::{Quote, QuoteId, QuoteLine, QuoteStatus},
    };

    use super::{
        GhostQuoteGenerator, InMemoryCustomerHistoryProvider, InMemoryGhostQuoteStore,
        SignalDetector, SignalDetectorConfig,
    };

    #[test]
    fn detects_buying_signal_with_entities_and_competitor_mentions() {
        let detector = SignalDetector::default();
        let signal = detector
            .detect(
                "Acme Corp is evaluating options and needs budget approval to expand marketing and sales teams next quarter vs Salesforce.",
            )
            .expect("high-confidence signal");

        assert!(signal.confidence >= 70);
        assert!(signal.keyword_matches.contains(&"evaluating".to_string()));
        assert!(signal.keyword_matches.contains(&"budget".to_string()));
        assert!(signal.companies.contains(&"Acme Corp".to_string()));
        assert!(signal.departments.contains(&"marketing".to_string()));
        assert!(signal.timelines.contains(&"next quarter".to_string()));
        assert!(signal.competitors.contains(&"salesforce".to_string()));
    }

    #[test]
    fn returns_low_confidence_signal_below_threshold_for_weak_text() {
        let detector = SignalDetector::default();
        let signal = detector.analyze("Team standup in 10 minutes.");

        assert!(signal.confidence < detector.config().confidence_threshold);
        assert!(!signal.above_threshold);
        assert!(detector.detect("Team standup in 10 minutes.").is_none());
    }

    #[test]
    fn threshold_is_configurable() {
        let detector = SignalDetector::new(SignalDetectorConfig {
            confidence_threshold: 40,
            ..SignalDetectorConfig::default()
        });

        let signal = detector
            .detect("Evaluating upgrade options for operations team.")
            .expect("signal should pass lowered threshold");

        assert!(signal.confidence >= 40);
    }

    #[test]
    fn extracts_multiple_timeline_patterns_and_departments() {
        let detector = SignalDetector::default();
        let signal = detector.analyze(
            "We are looking for pricing in Q2 and next month for finance and engineering departments.",
        );

        assert!(signal.keyword_matches.contains(&"looking for".to_string()));
        assert!(signal.timelines.contains(&"q2".to_string()));
        assert!(signal.timelines.contains(&"next month".to_string()));
        assert!(signal.departments.contains(&"finance".to_string()));
        assert!(signal.departments.contains(&"engineering".to_string()));
    }

    #[test]
    fn ghost_quote_generator_creates_discounted_draft_and_persists_it() {
        let detector = SignalDetector::default();
        let signal = detector
            .detect(
                "Acme Corp plans to expand operations next quarter and is evaluating Salesforce pricing.",
            )
            .expect("should detect high-confidence signal");

        let mut history_provider = InMemoryCustomerHistoryProvider::default();
        history_provider.insert_history(
            "Acme Corp",
            vec![quote("Q-hist-1", 10, 100_000), quote("Q-hist-2", 12, 110_000)],
        );
        let mut store = InMemoryGhostQuoteStore::default();
        let generator = GhostQuoteGenerator::default();

        let ghost_quote = generator
            .generate("Q-ghost-1", &signal, &history_provider, &mut store)
            .expect("generation should not error")
            .expect("ghost quote should be created");

        assert_eq!(ghost_quote.company, "Acme Corp".to_string());
        assert!(ghost_quote.confidence >= signal.confidence);
        assert!(ghost_quote.suggested_discount_pct > 0);
        assert_eq!(store.drafts().len(), 1);
        assert_eq!(store.drafts()[0].id, QuoteId("Q-ghost-1".to_string()));
        assert!(
            store.drafts()[0].lines[0].unit_price < Decimal::new(110_000, 2),
            "expected discounted unit price in draft"
        );
    }

    #[test]
    fn ghost_quote_generator_returns_none_for_low_confidence_signal() {
        let signal = SignalDetector::default().analyze("General sync later today.");
        let history_provider = InMemoryCustomerHistoryProvider::default();
        let mut store = InMemoryGhostQuoteStore::default();
        let generator = GhostQuoteGenerator::default();

        let ghost_quote = generator
            .generate("Q-ghost-2", &signal, &history_provider, &mut store)
            .expect("generation should not error");

        assert!(ghost_quote.is_none());
        assert!(store.drafts().is_empty());
    }

    fn quote(quote_id: &str, quantity: u32, unit_price_cents: i64) -> Quote {
        Quote {
            id: QuoteId(quote_id.to_string()),
            status: QuoteStatus::Draft,
            lines: vec![QuoteLine {
                product_id: ProductId("starter".to_string()),
                quantity,
                unit_price: Decimal::new(unit_price_cents, 2),
            }],
            created_at: Utc::now(),
        }
    }
}
