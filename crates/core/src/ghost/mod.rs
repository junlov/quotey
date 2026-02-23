use std::collections::{BTreeSet, HashMap};

use serde::{Deserialize, Serialize};

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
    use super::{SignalDetector, SignalDetectorConfig};

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
}
