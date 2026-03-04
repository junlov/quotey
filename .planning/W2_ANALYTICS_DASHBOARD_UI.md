# W2 Analytics Dashboard UI (quotey-009-3)

## Objective
Build dashboard UI affordances that align with the analytics contract metrics/dimensions and
make filter context explicit to users.

## Implementation
Updated:
- `templates/dashboard/slack_widget.html`

Added UI sections:
1. **Analytics Controls**
   - metric chips reflecting contract metrics (quote count, win rate, avg discount, anomaly rate, approval/finalization cycle metrics)
   - dimension chips reflecting contract dimensions (month, region, customer segment, sales rep, product family, approval role)
2. **Secondary KPI Row**
   - anomaly rate
   - approval cycle hours
   - time-to-finalize hours
3. **Dimension Breakout Section**
   - grouped rows for `Month × Region` style outputs
   - fallback empty state when no grouped data exists

## Contract Alignment
The displayed labels map directly to:
- metrics from `MetricKind`
- dimensions from `DimensionKind`

This keeps UI naming synchronized with query-layer capabilities introduced in `quotey-009-2`.
