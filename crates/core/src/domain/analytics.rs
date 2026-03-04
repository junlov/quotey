use serde::{Deserialize, Serialize};
use thiserror::Error;

pub const ANALYTICS_SCHEMA_VERSION: &str = "analytics_contract.v1";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MetricKind {
    QuoteCount,
    WinRatePct,
    AvgDiscountPct,
    AvgDealValue,
    ApprovalCycleHours,
    TimeToFinalizeHours,
    AnomalyRatePct,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DimensionKind {
    Day,
    Week,
    Month,
    Quarter,
    CustomerSegment,
    Industry,
    Region,
    SalesRep,
    ProductFamily,
    ApprovalRole,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AnalyticsQuerySpec {
    pub schema_version: String,
    pub metrics: Vec<MetricKind>,
    pub dimensions: Vec<DimensionKind>,
    pub lookback_days: u32,
    pub include_only_finalized: bool,
}

impl AnalyticsQuerySpec {
    pub fn validate(&self) -> Result<(), AnalyticsContractError> {
        if self.schema_version != ANALYTICS_SCHEMA_VERSION {
            return Err(AnalyticsContractError::UnsupportedSchemaVersion {
                expected: ANALYTICS_SCHEMA_VERSION.to_string(),
                actual: self.schema_version.clone(),
            });
        }

        if self.metrics.is_empty() {
            return Err(AnalyticsContractError::MissingMetrics);
        }

        if self.lookback_days == 0 || self.lookback_days > 3650 {
            return Err(AnalyticsContractError::InvalidLookbackDays { value: self.lookback_days });
        }

        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Error)]
pub enum AnalyticsContractError {
    #[error("unsupported schema version: expected `{expected}`, got `{actual}`")]
    UnsupportedSchemaVersion { expected: String, actual: String },
    #[error("analytics query must include at least one metric")]
    MissingMetrics,
    #[error("lookback_days must be in range 1..=3650, got {value}")]
    InvalidLookbackDays { value: u32 },
}

#[cfg(test)]
mod tests {
    use super::{
        AnalyticsContractError, AnalyticsQuerySpec, DimensionKind, MetricKind,
        ANALYTICS_SCHEMA_VERSION,
    };

    fn valid_spec() -> AnalyticsQuerySpec {
        AnalyticsQuerySpec {
            schema_version: ANALYTICS_SCHEMA_VERSION.to_string(),
            metrics: vec![MetricKind::QuoteCount, MetricKind::WinRatePct],
            dimensions: vec![DimensionKind::Month, DimensionKind::Region],
            lookback_days: 90,
            include_only_finalized: true,
        }
    }

    #[test]
    fn validate_accepts_well_formed_spec() {
        let spec = valid_spec();
        assert_eq!(spec.validate(), Ok(()));
    }

    #[test]
    fn validate_rejects_empty_metrics() {
        let mut spec = valid_spec();
        spec.metrics.clear();
        assert_eq!(spec.validate(), Err(AnalyticsContractError::MissingMetrics));
    }

    #[test]
    fn validate_rejects_out_of_range_lookback() {
        let mut spec = valid_spec();
        spec.lookback_days = 0;
        assert_eq!(spec.validate(), Err(AnalyticsContractError::InvalidLookbackDays { value: 0 }));
    }
}
