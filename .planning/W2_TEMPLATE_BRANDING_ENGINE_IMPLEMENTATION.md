# W2 Template Branding Engine Implementation

## Bead
- `quotey-007-2` — Implement template engine with branding

## Delivered
- Added a branding contract in `crates/server/src/pdf.rs` via `TemplateBranding`.
- Branding values are now resolved deterministically from:
  1. `quote_data.branding.*` (preferred)
  2. top-level legacy keys (`company_name`, `primary_color`, etc.)
  3. safe defaults (Quotey palette + non-white-label)
- Injected branding fields into template context for both:
  - `generate_quote_pdf(...)`
  - `generate_print_html(...)`

## Supported Branding Fields
- `company_name`
- `company_logo`
- `company_address`
- `company_email`
- `company_phone`
- `primary_color`
- `secondary_color`
- `accent_color`
- `footer_text`
- `white_label`

## Verification
- Added tests in `crates/server/src/pdf.rs`:
  - defaults when branding data is absent
  - nested branding precedence over legacy top-level keys
