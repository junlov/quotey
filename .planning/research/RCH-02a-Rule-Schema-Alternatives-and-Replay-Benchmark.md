# RCH-02a: Rule Schema Alternatives and Replay Benchmark

**Research Task:** `bd-3d8.11.3.1`  
**Status:** Complete  
**Date:** 2026-02-23  
**Researcher:** LilacMountain (Codex)  
**Primary Inputs:** `.planning/research/RCH-02-Rules-Architecture-and-Evaluation-Order.md`, `.planning/research/RCH-09-Decision-Freeze-and-Phased-Execution-Plan.md`

---

## Executive Summary

This task compares viable rule schema approaches for deterministic CPQ evaluation and measures replay/explain costs under synthetic workload.

Decision:

1. keep the **normalized stage+priority relational model with structured JSON payloads** as the primary architecture,
2. avoid making snapshot-only JSON storage the primary path due replay and explainability overhead,
3. apply a phased migration that adds benchmark-backed validation and replay conformance checks before broader rule growth.

Headline benchmark result (in-memory synthetic run):

1. normalized model replay p50: `6.903ms`,
2. snapshot-JSON model replay p50: `18.970ms` (~2.75x slower),
3. normalized explain query p50: `0.024ms`,
4. event-JSON explain query p50: `0.056ms` (~2.33x slower).

---

## 1. Alternatives Evaluated

## 1.1 Option A (Recommended): Normalized Relational Rules + Structured JSON Payload

Core shape:

1. typed rule metadata columns (`stage`, `priority`, `specificity`, `ruleset_version_id`, `rule_family`),
2. condition/effect payload in JSON columns,
3. indexed trace table for explainability and replay verification.

Strengths:

1. deterministic ordering can be index-aligned,
2. explainability queries are narrow and cheap,
3. compatible with SQLite CLI/operator tooling and immutable ruleset versioning.

Risks:

1. requires schema discipline and validation for JSON payload shape.

## 1.2 Option B: Snapshot-JSON Primary Rules + JSON Decision Event Log

Core shape:

1. one JSON snapshot blob per ruleset,
2. JSON payload rows for decision events.

Strengths:

1. simple write model for rule authoring prototype paths,
2. flexible for rapid schema experimentation.

Risks:

1. replay cost shifts to decode/sort in application path,
2. explainability requires JSON parse cost and looser query-level guarantees,
3. weaker constraints on deterministic ordering at persistence layer.

---

## 2. Benchmark Method

## 2.1 Workload Profile

Synthetic in-memory SQLite workload:

1. rules: `8,000`,
2. quotes with trace data: `1,200`,
3. trace rows per quote: `36`,
4. total trace rows: `43,200`.

## 2.2 Operations Measured

Replay benchmark:

1. Option A: indexed ordered query over normalized rule rows,
2. Option B: read snapshot JSON + decode + sort in app path.

Explain benchmark:

1. Option A: indexed select over trace table for one quote/version,
2. Option B: select JSON event rows + parse payloads.

## 2.3 Environment and Caveats

1. Runtime: `python3` stdlib `sqlite3`, in-memory DB.
2. This is relative comparison data, not production latency prediction.
3. WAL/fsync/file I/O effects are excluded; focus is schema/path overhead.

---

## 3. Benchmark Results

| Metric | Option A (Normalized) | Option B (Snapshot/Event JSON) | Delta |
|---|---:|---:|---:|
| Replay p50 | 6.903 ms | 18.970 ms | Option B ~2.75x slower |
| Replay p95 | 7.311 ms | 22.296 ms | Option B ~3.05x slower |
| Replay avg | 6.971 ms | 19.274 ms | Option B ~2.76x slower |
| Explain p50 | 0.024 ms | 0.056 ms | Option B ~2.33x slower |
| Explain p95 | 0.030 ms | 0.068 ms | Option B ~2.27x slower |
| Explain avg | 0.025 ms | 0.058 ms | Option B ~2.32x slower |

Interpretation:

1. replay is materially more efficient with normalized indexed metadata than snapshot decode/sort,
2. explain paths also favor normalized trace rows over JSON event parsing,
3. relative deltas support keeping Option A as default for deterministic CPQ core.

---

## 4. Decision Matrix

Scores: 1 (poor) to 5 (strong)

| Criteria | Option A | Option B | Rationale |
|---|---:|---:|---|
| Deterministic ordering enforceability | 5 | 3 | Option A persists ordering metadata and supports index-backed order keys |
| Replay cost | 5 | 2 | Benchmark shows ~2.75x replay advantage for Option A |
| Explainability query cost | 5 | 3 | Benchmark shows ~2.3x explain advantage for Option A |
| Operator tooling friendliness | 5 | 3 | SQL/CLI inspection clearer in normalized tables |
| Schema evolution agility | 4 | 5 | Option B is more schema-flexible via JSON blobs |
| Audit/legal defensibility | 5 | 3 | Option A has stronger row-level traceability |
| Overall | **29** | **19** | Option A recommended |

---

## 5. Recommended Migration Path

## 5.1 Phase M1: Baseline Hardening (Immediate)

1. enforce common rule metadata columns and NOT NULL constraints for ordering fields,
2. enforce deterministic ordering key:
   1. `(stage ASC, priority DESC, specificity DESC, rule_id ASC)`,
3. enforce rule payload validation at write-time.

## 5.2 Phase M2: Replay/Explain Conformance

1. add replay conformance check command for sampled historical quotes,
2. add explain trace completeness assertion per evaluation stage,
3. benchmark guard in CI for regression trend (relative thresholds).

## 5.3 Phase M3: Optional Hybrid Optimization (If Needed)

1. allow derived materialized snapshot cache for read optimization,
2. keep normalized tables as source of truth,
3. invalidate cache atomically by `ruleset_version_id` activation.

Guardrail:

1. do not switch primary storage to snapshot-only JSON unless replay/explain benchmarks beat Option A at target scale and deterministic/audit guarantees remain equivalent.

---

## 6. Risks and Mitigations

| Risk | Impact | Mitigation |
|---|---|---|
| JSON payload drift in Option A | validation gaps, runtime failures | strict schema validation + migration checks |
| Index regression under growth | replay latency increase | periodic benchmark and index-review gate |
| Partial rollout of ordering fields | non-deterministic behavior | enforce required fields before activation |
| Overfitting to in-memory benchmark | false confidence | add file-backed WAL benchmark in follow-on implementation phase |

---

## 7. Follow-On Implementation Notes

1. Attach benchmark harness shape to future reliability/perf verification commands.
2. Keep trace tables queryable by `quote_id + quote_version`.
3. Preserve immutable ruleset version references in pricing snapshots.

Recommended consumers:

1. `bd-3d8.7` CPQ core evaluator seams,
2. future rule tooling and preview commands,
3. replay verification work in quality gates.

---

## 8. Acceptance Mapping for `bd-3d8.11.3.1`

Deliverable: evaluate at least two schema approaches  
Completed: Sections 1 and 4.

Deliverable: benchmark replay/explain cost  
Completed: Sections 2 and 3.

Acceptance: objective decision matrix and recommendation  
Completed: Section 4.

Acceptance: migration path included  
Completed: Section 5.
