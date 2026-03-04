use quotey_core::{AnalyticsQuerySpec, DimensionKind, MetricKind};
use thiserror::Error;

#[derive(Debug, Clone, Default)]
pub struct SqlAnalyticsQueryBuilder;

impl SqlAnalyticsQueryBuilder {
    pub fn build_query(&self, spec: &AnalyticsQuerySpec) -> Result<String, AnalyticsQueryError> {
        spec.validate().map_err(|error| AnalyticsQueryError::InvalidSpec(error.to_string()))?;

        let dimension_selects: Vec<String> = spec
            .dimensions
            .iter()
            .map(|dimension| {
                format!("{} AS {}", dimension_sql(dimension), dimension_alias(dimension))
            })
            .collect();
        let metric_selects: Vec<String> =
            spec.metrics.iter().map(|metric| metric_sql(metric).to_string()).collect();

        let mut select_parts = Vec::new();
        select_parts.extend(dimension_selects.clone());
        select_parts.extend(metric_selects);

        let mut query = format!(
            "SELECT\n  {}\nFROM quote q\nLEFT JOIN quote_line ql ON ql.quote_id = q.id\nLEFT JOIN product p ON p.id = ql.product_id\nLEFT JOIN product_family pf ON pf.id = p.family_id\nLEFT JOIN approval_request ar ON ar.quote_id = q.id\nLEFT JOIN audit_event ae ON ae.quote_id = q.id\nWHERE q.created_at >= datetime('now', '-{} days')",
            select_parts.join(",\n  "),
            spec.lookback_days
        );

        if spec.include_only_finalized {
            query.push_str("\n  AND q.status IN ('approved', 'finalized', 'sent')");
        }

        if !spec.dimensions.is_empty() {
            let group_by =
                spec.dimensions.iter().map(dimension_alias).collect::<Vec<_>>().join(", ");
            query.push_str(&format!("\nGROUP BY {group_by}"));
            query.push_str(&format!("\nORDER BY {group_by}"));
        }

        Ok(query)
    }
}

fn dimension_sql(dimension: &DimensionKind) -> &'static str {
    match dimension {
        DimensionKind::Day => "strftime('%Y-%m-%d', q.created_at)",
        DimensionKind::Week => "strftime('%Y-W%W', q.created_at)",
        DimensionKind::Month => "strftime('%Y-%m', q.created_at)",
        DimensionKind::Quarter => {
            "printf('%s-Q%d', strftime('%Y', q.created_at), ((cast(strftime('%m', q.created_at) as integer)-1)/3)+1)"
        }
        DimensionKind::CustomerSegment => "COALESCE(q.account_id, 'unknown')",
        DimensionKind::Industry => "COALESCE(json_extract(q.notes, '$.industry'), 'unknown')",
        DimensionKind::Region => "COALESCE(json_extract(q.notes, '$.region'), 'unknown')",
        DimensionKind::SalesRep => "COALESCE(q.created_by, 'unknown')",
        DimensionKind::ProductFamily => "COALESCE(pf.name, 'unknown')",
        DimensionKind::ApprovalRole => "COALESCE(ar.approver_role, 'none')",
    }
}

fn dimension_alias(dimension: &DimensionKind) -> &'static str {
    match dimension {
        DimensionKind::Day => "dim_day",
        DimensionKind::Week => "dim_week",
        DimensionKind::Month => "dim_month",
        DimensionKind::Quarter => "dim_quarter",
        DimensionKind::CustomerSegment => "dim_customer_segment",
        DimensionKind::Industry => "dim_industry",
        DimensionKind::Region => "dim_region",
        DimensionKind::SalesRep => "dim_sales_rep",
        DimensionKind::ProductFamily => "dim_product_family",
        DimensionKind::ApprovalRole => "dim_approval_role",
    }
}

fn metric_sql(metric: &MetricKind) -> &'static str {
    match metric {
        MetricKind::QuoteCount => "COUNT(DISTINCT q.id) AS metric_quote_count",
        MetricKind::WinRatePct => {
            "ROUND(AVG(CASE WHEN q.status IN ('approved','finalized','sent') THEN 100.0 ELSE 0.0 END), 2) AS metric_win_rate_pct"
        }
        MetricKind::AvgDiscountPct => "ROUND(AVG(COALESCE(ql.discount_pct, 0)), 2) AS metric_avg_discount_pct",
        MetricKind::AvgDealValue => {
            "ROUND(AVG(COALESCE(ql.unit_price, 0) * COALESCE(ql.quantity, 0)), 2) AS metric_avg_deal_value"
        }
        MetricKind::ApprovalCycleHours => {
            "ROUND(AVG((julianday(ar.updated_at) - julianday(ar.created_at)) * 24.0), 2) AS metric_approval_cycle_hours"
        }
        MetricKind::TimeToFinalizeHours => {
            "ROUND(AVG((julianday(q.updated_at) - julianday(q.created_at)) * 24.0), 2) AS metric_time_to_finalize_hours"
        }
        MetricKind::AnomalyRatePct => {
            "ROUND(AVG(CASE WHEN ae.event_type LIKE 'anomaly.%' THEN 100.0 ELSE 0.0 END), 2) AS metric_anomaly_rate_pct"
        }
    }
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum AnalyticsQueryError {
    #[error("invalid analytics query spec: {0}")]
    InvalidSpec(String),
}

#[cfg(test)]
mod tests {
    use quotey_core::{AnalyticsQuerySpec, DimensionKind, MetricKind, ANALYTICS_SCHEMA_VERSION};

    use super::SqlAnalyticsQueryBuilder;

    #[test]
    fn build_query_includes_metrics_dimensions_and_filters() {
        let builder = SqlAnalyticsQueryBuilder;
        let spec = AnalyticsQuerySpec {
            schema_version: ANALYTICS_SCHEMA_VERSION.to_string(),
            metrics: vec![MetricKind::QuoteCount, MetricKind::WinRatePct],
            dimensions: vec![DimensionKind::Month, DimensionKind::Region],
            lookback_days: 90,
            include_only_finalized: true,
        };

        let sql = builder.build_query(&spec).expect("query should build");
        assert!(sql.contains("metric_quote_count"));
        assert!(sql.contains("metric_win_rate_pct"));
        assert!(sql.contains("dim_month"));
        assert!(sql.contains("dim_region"));
        assert!(sql.contains("GROUP BY dim_month, dim_region"));
        assert!(sql.contains("q.status IN ('approved', 'finalized', 'sent')"));
        assert!(sql.contains("datetime('now', '-90 days')"));
    }

    #[test]
    fn build_query_without_dimensions_skips_group_by() {
        let builder = SqlAnalyticsQueryBuilder;
        let spec = AnalyticsQuerySpec {
            schema_version: ANALYTICS_SCHEMA_VERSION.to_string(),
            metrics: vec![MetricKind::AvgDiscountPct],
            dimensions: vec![],
            lookback_days: 30,
            include_only_finalized: false,
        };

        let sql = builder.build_query(&spec).expect("query should build");
        assert!(sql.contains("metric_avg_discount_pct"));
        assert!(!sql.contains("GROUP BY"));
    }
}
