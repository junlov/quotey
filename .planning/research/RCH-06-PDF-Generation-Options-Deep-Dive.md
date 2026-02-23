# RCH-06: PDF Generation Options Deep Dive

**Bead:** `bd-256v.6`  
**Status:** Complete  
**Date:** 2026-02-23  
**Researcher:** IvoryBear (Codex)

## 1. Executive Summary

For Quotey, the best near-term PDF strategy is:

1. **Primary renderer:** Chromium `Page.printToPDF` via CDP (using `headless_chrome` or equivalent CDP client).
2. **Architecture:** renderer trait with pluggable backends and deterministic render request model.
3. **Fallback policy:** keep wkhtmltopdf support optional and isolated behind feature flag if required by specific deployments.

Rationale:

1. Highest fidelity for modern HTML/CSS layouts.
2. Strong control over page sizing/margins/header/footer via CDP options.
3. Aligns with existing project decision leaning toward HTML-to-PDF for design velocity.

## 2. Constraints and Goals

From project context:

1. Local-first deployment with minimal operator friction.
2. Deterministic and auditable quote outputs.
3. Fast iteration on quote template design.
4. Pragmatic implementation complexity for alpha delivery.

Environment check (current dev machine):

1. `wkhtmltopdf` not installed.
2. `chromium`/`google-chrome` not installed.

Implication:

- renderer strategy must handle missing binary/runtime prerequisites with clear diagnostics.

## 3. Evaluation Criteria

Scoring dimensions:

1. HTML/CSS fidelity for business templates.
2. Pagination and header/footer control.
3. Determinism/reproducibility.
4. Operational complexity (install/runtime requirements).
5. Rust integration effort.
6. Performance and concurrency behavior.
7. Long-term maintenance risk.

## 4. Options Compared

### 4.1 Option A: Chromium via CDP (`headless_chrome` + `Page.printToPDF`)

Pros:

1. Modern browser rendering fidelity.
2. Fine-grained PDF options (`landscape`, `scale`, margins, header/footer templates).
3. Good path for HTML templates authored with contemporary CSS.

Cons:

1. Requires Chromium/Chrome runtime in deployment environment.
2. Heavier startup/runtime footprint than pure-Rust options.
3. `headless_chrome` API ecosystem less mainstream than Playwright/Puppeteer tooling.

Best fit:

- production-quality quote PDF with rich CSS and predictable visual output.

### 4.2 Option B: `wkhtmltopdf` External Binary

Pros:

1. Historically popular and simple command-line integration.
2. Easy to call from Rust process wrapper.

Cons:

1. Project status marks wkhtmltopdf as archived and recommends migration to Puppeteer/Playwright.
2. Uses older WebKit rendering model with weaker modern CSS support.
3. Adds external binary lifecycle and security patch burden.

Best fit:

- legacy environments that already standardize on wkhtmltopdf and accept feature limits.

### 4.3 Option C: `genpdf` (Pure Rust)

Pros:

1. No browser runtime dependency.
2. Simpler distribution in constrained environments.

Cons:

1. Layout model is document-API-driven rather than HTML/CSS-first.
2. Less suitable for modern branded templates maintained by non-Rust contributors.
3. Lower feature velocity in ecosystem compared to browser-based rendering workflows.

Best fit:

- simple invoice/statement-like outputs where programmatic layout is acceptable.

### 4.4 Option D: `printpdf` (Pure Rust, low-level)

Pros:

1. Full low-level control over PDF primitives.
2. No browser dependency.

Cons:

1. Higher implementation complexity for rich layout.
2. HTML-to-PDF support is explicitly experimental in current crate docs.
3. Engineering cost high for CPQ template iteration speed goals.

Best fit:

- specialized PDF drawing pipelines, not fast HTML template workflows.

### 4.5 Option E: WeasyPrint (Python, non-Rust sidecar)

Pros:

1. Strong HTML/CSS print model.
2. Mature print-oriented feature set.

Cons:

1. Introduces Python runtime and cross-language ops burden.
2. Increases packaging complexity versus Rust-only stack direction.

Best fit:

- organizations already operating Python document pipelines.

## 5. Decision Matrix

| Option | Fidelity | Ops Complexity | Rust Fit | Template Velocity | Recommendation |
|---|---|---|---|---|---|
| Chromium/CDP | High | Medium-High | Medium | High | **Primary** |
| wkhtmltopdf | Medium | Medium | Medium | Medium | Secondary fallback only |
| genpdf | Low-Medium | Low | High | Low | Not preferred for main quote templates |
| printpdf | Low-Medium (high effort) | Low | High | Low | Not preferred for alpha |
| WeasyPrint | High | High | Low | High | Not preferred (cross-runtime overhead) |

## 6. Performance and Resource Considerations

Observed from architecture characteristics (not benchmarked in this session):

1. Browser-based renderers have higher startup/memory overhead but superior layout fidelity.
2. Pure-Rust renderers likely have lower runtime dependencies but higher template engineering effort.
3. Throughput can be improved for Chromium by reusing browser/session where feasible.

Benchmark plan (recommended follow-on):

1. Render identical 1-page and 5-page quote templates for 100 iterations.
2. Measure p50/p95 render latency, RSS memory, and failure rate.
3. Run single-thread and limited parallel (2/4 workers) scenarios.
4. Compare Chromium/CDP vs wkhtmltopdf fallback (if available).

## 7. Template Engine Integration

Recommended pipeline:

1. Build HTML using Tera templates.
2. Inject deterministic quote snapshot data (`quote_id`, `quote_version`, totals, approval refs).
3. Persist normalized HTML payload hash for audit traceability.
4. Render PDF with selected backend.
5. Store checksum + metadata in audit/event stream.

Why this fits Quotey:

1. Keeps template iteration fast.
2. Preserves deterministic input artifacts for replay/audit.
3. Cleanly separates data shaping from renderer mechanics.

## 8. Deployment and Reliability Guidance

### 8.1 Runtime Checks

At startup or `doctor`:

1. verify renderer backend availability (binary path/version),
2. verify writable temp/work directory for render artifacts,
3. verify timeout and retry limits are configured.

### 8.2 Failure Handling

1. Treat render as side effect: never roll back authoritative quote state on render failure.
2. Emit correlation-linked events for `render_started`, `render_failed`, `render_completed`.
3. Expose retry action to operator/user workflow with idempotency key.

### 8.3 Security

1. Disable untrusted remote resource loading in templates where possible.
2. Keep template input source controlled and sanitized.
3. Log renderer command/options without leaking sensitive data payloads.

## 9. Recommendation

Adopt **Chromium/CDP primary + trait-based renderer abstraction**:

1. `QuotePdfRenderer` trait in core interface.
2. `ChromiumPdfRenderer` implementation in adapter layer.
3. Optional `WkhtmltopdfRenderer` fallback behind feature/config switch.
4. Use deterministic render request model with versioned schema.

This maximizes output quality while preserving implementation flexibility and future replacement paths.

## 10. Deliverable Coverage vs Bead Requirements

`bd-256v.6` requested:

1. **Options matrix**: Sections 4 and 5.
2. **Template integration examples**: Section 7.
3. **Deployment impact analysis**: Sections 2 and 8.
4. **Recommendation with tradeoffs**: Sections 5 and 9.
5. **ADR**: `.planning/research/ADR-RCH06-PDF-Generation-Architecture.md`.

Note:

- Live performance benchmarks/PoC execution are defined as a concrete follow-on plan (Section 6) because required external binaries are not present in the current environment.

## 11. Primary Sources

1. CDP `printToPDF` parameters: https://chromedevtools.github.io/devtools-protocol/tot/Page/#method-printToPDF
2. `headless_chrome` crate docs: https://docs.rs/headless_chrome/latest/headless_chrome/
3. wkhtmltopdf status (archived): https://wkhtmltopdf.org/status
4. `genpdf` crate docs: https://docs.rs/genpdf/latest/genpdf/
5. `printpdf` crate docs: https://docs.rs/printpdf/latest/printpdf/
