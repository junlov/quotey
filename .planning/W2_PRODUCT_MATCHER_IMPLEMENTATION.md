# W2 Product Matcher Implementation (quotey-005-4)

## Objective
Implement deterministic mapping from extracted requirement text to product catalog entries with:
- confidence-scored matches,
- ambiguity surfacing for close candidates,
- explicit unmatched output.

## Implementation
Added:
- `crates/core/src/cpq/product_matcher.rs`

Core API:
- `ProductMatcher::match_requirements(extracted, catalog) -> ProductMatchResult`

Output:
- `matches[]` (`ProductMatch`)
- `ambiguities[]` (`MatchAmbiguity`)
- `unmatched_requirements[]`

## Scoring Signals
Matcher currently combines deterministic signals:
1. exact/partial product-name overlap
2. SKU overlap
3. description phrase overlap
4. token overlap ratio
5. lightweight domain synonym boost (`sso`, `onboarding`, `support`, `compliance`)

Thresholding:
- minimum match confidence: `0.45`
- ambiguity band: top-2 delta <= `0.12`

## Validation
Unit tests cover:
1. known requirement -> expected product match,
2. low-confidence input -> unmatched,
3. close scores -> ambiguity output.
