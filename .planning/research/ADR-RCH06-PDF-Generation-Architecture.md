# ADR: PDF Generation Architecture for Quotey

**Status:** Accepted  
**Date:** 2026-02-23  
**Related Bead:** `bd-256v.6`

## Context

Quotey must generate polished quote PDFs from structured quote data while preserving deterministic, auditable behavior.

Key constraints:

1. high-fidelity business document rendering,
2. practical implementation effort for alpha,
3. local-first operational model,
4. ability to evolve renderer backend without domain churn.

## Decision

Adopt a **renderer-abstraction architecture** with:

1. **Primary backend:** Chromium via CDP `Page.printToPDF`.
2. **Interface boundary:** `QuotePdfRenderer` trait in core/app interface layer.
3. **Optional fallback backend:** wkhtmltopdf adapter behind explicit config/feature flag.
4. **Deterministic render request model**:
   - `quote_id`,
   - `quote_version`,
   - template id/version,
   - normalized HTML payload hash,
   - render options schema version.

## Rationale

1. Chromium/CDP provides strongest modern HTML/CSS fidelity.
2. Trait boundary keeps renderer implementation replaceable.
3. Optional fallback addresses constrained environments without forcing lowest-common-denominator rendering.
4. Hashing render inputs preserves replay/audit traceability.

## Consequences

### Positive

1. Better visual quality and layout control for customer-facing quotes.
2. Faster template iteration using HTML/CSS + Tera.
3. Controlled migration path if renderer backend changes later.

### Negative

1. Browser runtime dependency increases ops footprint.
2. Cold-start and memory profile higher than pure-Rust PDF libraries.
3. Requires explicit runtime health checks and diagnostics.

## Guardrails

1. PDF generation failures must not mutate authoritative quote state.
2. Every render attempt must emit auditable events with correlation id.
3. Render requests are idempotent by `quote_id + quote_version + template_version`.
4. No unbounded retries on renderer failure; use bounded retry + operator-visible failure state.

## Verification Plan

1. Integration tests for successful render flow and timeout/failure behavior.
2. Idempotency tests for duplicate render requests.
3. Startup/doctor checks for backend availability.
4. Benchmark suite comparing latency and resource profile by backend.

## Revisit Triggers

1. Persistent renderer instability in target deployment environments.
2. Significant template fidelity gaps discovered in production use.
3. Requirement changes toward stricter packaging constraints.

## References

1. https://chromedevtools.github.io/devtools-protocol/tot/Page/#method-printToPDF
2. https://docs.rs/headless_chrome/latest/headless_chrome/
3. https://wkhtmltopdf.org/status
4. https://docs.rs/genpdf/latest/genpdf/
5. https://docs.rs/printpdf/latest/printpdf/
