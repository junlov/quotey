# ADR-009: Distribution and Update Architecture

**Status:** Accepted  
**Date:** 2026-02-23  
**Related Bead:** `bd-256v.9`

## Context

Quotey is local-first and must be operable as a single binary across common enterprise endpoints.  
Current workspace foundations already include deterministic startup validation and SQL migration execution, but there is no release/update architecture yet.

Without a defined distribution strategy, the project risks:

1. inconsistent platform support,
2. unsafe upgrade behavior for local SQLite data,
3. release process drift between agents/operators.

## Decision

Adopt the following architecture:

1. **Release orchestration split**
   - `release-plz` owns versioning/changelog/release PR flow.
   - `cargo-dist` owns multi-target artifacts and installer generation.

2. **Primary delivery channel**
   - GitHub Releases with prebuilt binaries + generated installers (shell/PowerShell).

3. **Cross-platform target baseline**
   - support Linux x86_64, Linux ARM64, macOS Intel/Apple Silicon, Windows x86_64.
   - use `cross` / `cargo-zigbuild` as fallback or specialization tooling when required.

4. **Release build profile policy**
   - use explicit release profile tuning (`thin` LTO, symbol stripping, controlled codegen units).
   - do not default to UPX compression in v1.

5. **Update safety model**
   - verify artifact integrity,
   - snapshot executable + SQLite files before replacement,
   - execute migration convergence after binary swap,
   - rollback binary + data snapshot on migration failure.

## Rationale

1. Keeps installation simple for non-Rust operators.
2. Uses mature Rust ecosystem tooling for repeatable releases.
3. Preserves deterministic, auditable behavior across upgrades.
4. Minimizes update blast radius in local-first deployments.

## Consequences

### Positive

1. Consistent and reproducible release outputs across platforms.
2. Cleaner contributor workflow through release PR automation.
3. Safer update posture for SQLite-backed deployments.

### Negative

1. Added CI/release pipeline complexity.
2. Ongoing maintenance burden for target matrix and signing/provenance.
3. Need explicit runbooks for rollback and migration recovery.

## Guardrails

1. No release artifact is published without checksums and provenance metadata.
2. No in-place update runs without pre-update snapshot of SQLite data files.
3. No migration-breaking release is published without rollback validation.
4. No bypass path allows LLM/runtime adapters to mutate update policy decisions.

## Verification Plan

1. CI release dry-run validates matrix artifact generation.
2. Per-platform smoke tests run `doctor`, `migrate`, and startup checks.
3. Update/rollback integration test verifies snapshot-restore path under forced migration failure.
4. Periodic audit of release provenance/signing configuration.

## Revisit Triggers

1. Need for enterprise-native package channels (`deb`, `rpm`, `msi`) at scale.
2. Requirement for unattended/automatic updates with policy controls.
3. Introduction of multi-tenant/cloud-hosted control plane requirements.
4. Repeated operational incidents tied to upgrade workflow.

## References

1. https://github.com/axodotdev/cargo-dist
2. https://axodotdev.github.io/cargo-dist/book/
3. https://release-plz.dev/docs/introduction/
4. https://release-plz.dev/docs/commands/release-pr/
5. https://github.com/cross-rs/cross
6. https://github.com/rust-cross/cargo-zigbuild
7. https://doc.rust-lang.org/cargo/reference/profiles.html
8. https://github.com/axodotdev/axoupdater
