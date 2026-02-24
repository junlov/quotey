# W1 OBS Observability Infrastructure Spec

## Purpose
Define scope, KPI contract, deterministic guardrails, and interface boundaries for `W1_OBS` (Observability Infrastructure) so all Quotey operations are traceable, measurable, and debuggable in production.

## Scope
### In Scope
- Structured logging with configurable formats (compact for dev, JSON for production)
- Distributed tracing with OpenTelemetry integration
- Business and system metrics collection (counters, histograms, gauges)
- Request correlation across Slack threads, quotes, and operations
- Automatic span creation for core service operations
- Health check and readiness probe instrumentation

### Out of Scope (for Wave 1)
- Real-time alerting thresholds (use external systems like PagerDuty)
- Log aggregation infrastructure (assume external ELK/Datadog)
- Distributed tracing across multiple services (single-process only)
- Custom metrics dashboards (assume external Grafana/Datadog)
- Log sampling or rate limiting

## Rollout Slices
- `Slice A` (contracts): Define event schema, span naming conventions, and metric types
- `Slice B` (core): Implement tracing layer, structured logging, and correlation propagation
- `Slice C` (instrumentation): Add #[instrument] macros and metrics collection to core services
- `Slice D` (integration): Configure OTel export, health check instrumentation, and runbook

`Slice A/B` must be complete before `Slice C`; `Slice C` must be complete before `Slice D` production use.

## KPI Contract
| KPI | Baseline | Wave-1 Target | Owner | Measurement |
|---|---:|---:|---|---|
| Log queryability | 0% structured | 100% JSON in prod | Platform | `% of prod logs with parseable JSON` |
| Request traceability | No correlation IDs | 100% correlated | Runtime | `% of requests with correlation_id in logs` |
| Operation latency visibility | No duration tracking | 100% instrumented | Platform | `% of core operations with duration_ms` |
| Error debuggability | Text logs only | Stack traces + context | Runtime | `mean time to identify error root cause` |
| Health observability | Basic health check | Full readiness/liveness | Platform | `health check coverage of dependencies` |

## Deterministic Safety Constraints
- Logging must never panic or fail the operation being logged
- Trace IDs must be propagated deterministically (same input = same correlation)
- Metrics collection must not block business operations (async/fire-and-forget)
- Log fields must not contain PII (emails, phone numbers, API keys)
- Health checks must not modify system state (read-only probes)

## Interface Boundaries (Draft)
### Domain Contracts
- `ObservabilityContext`: correlation_id, quote_id, thread_id, actor_id, span_id
- `LogEvent`: timestamp, level, event_name, fields (BTreeMap), source_location
- `MetricEvent`: name, value, type (counter/histogram/gauge), labels
- `SpanContext`: trace_id, span_id, parent_span_id, operation_name, start_time, duration_ms

### Service Contracts
- `ObservabilityService::initialize(config) -> Result<ObservabilityHandle, Error>`
- `ObservabilityService::create_span(name, parent, context) -> Span`
- `ObservabilityService::record_metric(metric) -> Result<(), Error>`
- `ObservabilityService::emit_log(event) -> Result<(), Error>`
- `ObservabilityHandle::shutdown() -> Result<(), Error>`

### Persistence Contracts
- No persistence layer (logs/metrics exported externally)
- In-memory buffer for batch export (configurable size)

### Slack Contract
- All Slack events include correlation_id in logs
- Slash command processing has dedicated span
- Error responses include trace reference for support

### Crate Boundaries
- `quotey-core`: Domain types, context propagation, instrumented traits
- `quotey-db`: SQL query instrumentation with span per query
- `quotey-slack`: Event processing spans, correlation extraction
- `quotey-agent`: Runtime instrumentation, guardrail decision logging
- `quotey-server`: Initialization, health check instrumentation, graceful shutdown

## Risk Register
| Risk | Impact | Likelihood | Mitigation | Owner |
|---|---|---|---|---|
| OTel exporter performance degradation | High | Medium | Batch export, configurable buffer size, fallback to stdout | Platform |
| Log volume overwhelming ingestion | High | Low | Structured JSON enables filtering at destination, not source | Platform |
| Correlation ID context loss across async | Medium | High | Use tracing's built-in context propagation, audit all spawn points | Runtime |
| PII leakage in logs | High | Low | Audit all log fields, redaction middleware, code review checklist | Security |
| Health check false positives | Medium | Medium | Multi-layer health checks (liveness vs readiness), dependency probing | Platform |

## Guardrail Checklist (Pre-implementation Exit)
- [ ] Scope and non-goals agreed.
- [ ] KPI owner and metric formula explicitly documented.
- [ ] Deterministic constraints copied into implementation task templates.
- [ ] Interface contracts reviewed against existing crate boundaries.
- [ ] Risks and mitigations acknowledged by feature owner.

## Migration Contract
### Schema Additions
No database schema changes required.

### Configuration Changes
Add to config:
```toml
[observability]
log_format = "json"  # or "compact", "pretty"
log_level = "info"   # error, warn, info, debug, trace
otel_enabled = true
otel_endpoint = "http://localhost:4317"
metrics_export_interval_secs = 60
```

### Environment Variables
- `QUOTEY_LOGGING_FORMAT` - json, compact, or pretty
- `QUOTEY_LOGGING_LEVEL` - error, warn, info, debug, trace
- `QUOTEY_OTEL_ENABLED` - true/false
- `QUOTEY_OTEL_ENDPOINT` - OTLP collector endpoint

### Rollback Behavior
- Configuration can be changed at runtime (log level via env reload)
- OTel export can be disabled without restart
- Fallback to stdout logging always available

## Dependencies
- `tracing` (already in workspace)
- `tracing-subscriber` (already in workspace)
- `opentelemetry` (new)
- `opentelemetry_sdk` (new)
- `opentelemetry-otlp` (new)
- `tracing-opentelemetry` (new)
- `metrics` (new, optional)
- `metrics-exporter-prometheus` (new, optional)

## Reference Research
- `bd-397l`: Current observability patterns in codebase
- `bd-mlse`: Rust tracing best practices
- `bd-1shi`: Structured logging formats
- `bd-1bpy`: OpenTelemetry integration
- `bd-3i26`: Metrics collection strategy
