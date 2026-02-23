# ADR-003: Constraint Solver Architecture

**Status:** Accepted  
**Date:** 2026-02-23  
**Author:** Codex Agent  
**Related:** bd-256v.3 (Research Task)

## Context

Quotey needs a constraint solver for its CPQ (Configure, Price, Quote) functionality. The constraint solver must:

1. Validate product configurations (ensure selected products are compatible)
2. Support requires/excludes relationships between products
3. Support attribute and quantity constraints
4. Provide human-readable explanations for constraint violations
5. Run efficiently for interactive use in Slack

We evaluated multiple approaches: CSP (Constraint Satisfaction Problem), SAT (Boolean Satisfiability), SMT (Satisfiability Modulo Theories), and MILP (Mixed Integer Linear Programming).

## Decision

We will implement a **custom constraint solver** using the **AC-3 (Arc Consistency 3)** algorithm as the core propagation mechanism.

### Architecture Overview

```
┌─────────────────────────────────────────────────────────────┐
│              quotey-constraint-solver crate                 │
├─────────────────────────────────────────────────────────────┤
│  Core Types:                                                │
│    - Variable (represents a product/attribute)              │
│    - Domain (possible values)                               │
│    - Constraint (requires, excludes, attribute, etc.)       │
├─────────────────────────────────────────────────────────────┤
│  Algorithms:                                                │
│    - AC-3 propagation engine                                │
│    - Backtracking search (for finding solutions)            │
│    - Explanation generation                                 │
└─────────────────────────────────────────────────────────────┘
```

## Consequences

### Positive

1. **Optimal fit for problem domain** - CPQ constraints map naturally to CSP
2. **Human-readable explanations** - Custom solver allows tracking justification chains
3. **Incremental solving** - AC-3 supports efficient re-propagation after user changes
4. **No external dependencies** - Pure Rust implementation, no C++ libraries
5. **Maintainable codebase** - Well-understood algorithm (40+ years of research)
6. **CPQ-specific optimizations** - Can tailor for product configuration patterns

### Negative

1. **Implementation effort** - Must build and maintain our own solver
2. **No battle-tested library** - Risk of edge cases not covered by existing research
3. **Performance unknown** - Must benchmark for our specific use case

### Mitigations

- Algorithm is well-documented in academic literature
- Tacton CPQ (commercial leader) validates constraint-based approach
- Can fallback to SAT encoding if performance issues arise
- Incremental implementation allows early validation

## Alternatives Considered

### Alternative 1: SAT Solver (varisat)

**Why rejected:**
- Encoding CPQ constraints as boolean formulas is complex and non-intuitive
- Explanation generation is much harder (conflict clauses are not user-friendly)
- Numeric constraints require cumbersome bit-vector encodings
- While SAT solvers are fast, the encoding overhead negates benefits

### Alternative 2: SMT Solver (Z3 via rust bindings)

**Why rejected:**
- Heavyweight dependency (Z3 is a large C++ library)
- Overkill for CPQ constraints (we don't need uninterpreted functions, arrays, etc.)
- Limited control over explanation generation
- Adds complexity to deployment (additional native library)

### Alternative 3: MILP Solver (good_lp)

**Why rejected:**
- Poor fit for discrete configuration choices
- Requires/excludes constraints are non-linear and hard to express
- Better suited for pricing optimization than configuration validation

### Alternative 4: OR-Tools (via FFI)

**Why rejected:**
- C++ dependency complicates Rust build
- FFI overhead for interactive use
- Google's CP-SAT is excellent but overkill for our needs

## Implementation Notes

### Constraint Types Supported

| Type | Description | Example |
|------|-------------|---------|
| `Requires` | Product A needs Product B | "SSO requires Enterprise" |
| `Excludes` | Product A incompatible with B | "Basic support excludes 24/7" |
| `Attribute` | Conditional on attribute value | "If EU, currency must be EUR" |
| `Quantity` | Min/max quantity bounds | "Enterprise needs 50+ seats" |
| `CrossProduct` | Aggregate constraints | "Total seats < license cap" |

### Algorithm Complexity

- **AC-3 Time:** O(ed³) where e = constraints, d = domain size
- **AC-3 Space:** O(e) for the arc queue
- For typical CPQ: ~1000 constraints × 20³ domain = 8M operations (sub-millisecond)

### Future Extensions

If performance becomes an issue:
1. **AC-4 algorithm** - Better worst-case complexity O(ed²)
2. **Lazy clause generation** - Hybrid SAT/CP approach
3. **Constraint learning** - Cache nogoods across solves

## References

- Research Report: `.planning/research/RCH-03-Constraint-Solver-Research.md`
- Bead: `bd-256v.3`
- Academic: Mackworth (1977) "Consistency in networks of relations"
- Academic: Mohr & Henderson (1986) "Arc and path consistency revised"
