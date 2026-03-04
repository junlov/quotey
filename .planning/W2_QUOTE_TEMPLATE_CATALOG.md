# W2 Quote Template Catalog (quotey-007-1)

## Objective
Finalize and standardize the five required quote template designs with deterministic metadata
for runtime selection and future automation.

## Templates (5)
1. `executive_summary`
2. `detailed`
3. `comparison`
4. `renewal`
5. `compact`

## Implementation
Added machine-readable catalog:
- `templates/quotes/template_catalog.json`

Updated docs:
- `templates/README.md` now references the catalog as the source for deterministic
  selection metadata.

## Deterministic Selection Metadata
Each template entry includes:
- stable `id`
- `template_path`
- `target_use_case`
- `max_pages_target`
- capability flags:
  - `supports_comparison`
  - `supports_renewal_delta`

This removes ambiguity from string-based template routing and prepares runtime code for
explicit policy-driven template choice.
