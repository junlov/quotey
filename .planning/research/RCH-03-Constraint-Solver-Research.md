# RCH-03: Constraint Solver Algorithm Research

**Status:** Complete  
**Date:** 2026-02-23  
**Researcher:** Codex Agent  
**Bead:** bd-256v.3

---

## Executive Summary

This research evaluates constraint solver algorithms for Quotey's CPQ (Configure, Price, Quote) system. After analyzing academic literature, commercial implementations (Tacton, Choco), and the Rust ecosystem, we recommend a **custom Arc Consistency (AC-3) based constraint propagator** combined with **backtracking search** for the following reasons:

1. **CPQ constraints are naturally expressed as CSPs** (Constraint Satisfaction Problems)
2. **AC-3 provides optimal cost/benefit** for preprocessing and incremental validation
3. **Rust ecosystem lacks mature CP libraries**, making a custom implementation viable
4. **Commercial CPQ leaders (Tacton) use constraint-based approaches**, validating the approach

---

## 1. Background: Constraint-Based vs Rules-Based Configuration

### The Problem with Rules-Based Systems
Rules-based systems use if-then logic:
- "If customer selects Option A, then Option B is required"
- "If customer selects Option A, then Option C is excluded"

For N options, this requires O(N²) rules to cover all combinations, leading to:
- Combinatorial explosion
- Difficult maintenance
- Brittle configuration models

### Constraint-Based Approach
Constraint-based systems define what **must be true**:
- "Motor voltage must match enclosure voltage rating"
- "Enclosure thermal rating must exceed motor heat output"

A few constraints replace hundreds of rules. This is the approach used by Tacton CPQ.

### Tacton's Architecture Insights
From Tacton's documentation and academic papers:

1. **Constraint solver does not depend on sequence or order** - each constraint states an independent fact
2. **Users can answer questions in any order** and change their minds
3. **Solver validates choices and automatically finds optimal solutions** based on incomplete answers
4. **Splits large configuration problems** into smaller chunks to avoid performance issues
5. **Uses constraint propagation** to narrow search space

---

## 2. Algorithm Comparison

### 2.1 CSP (Constraint Satisfaction Problem)

**Definition:** A triple ⟨X, D, C⟩ where:
- X = {X₁, ..., Xₙ} is a set of variables
- D = {D₁, ..., Dₙ} is a set of domains
- C = {C₁, ..., Cₘ} is a set of constraints

**Characteristics:**
- Natural fit for product configuration
- Variables = product options/attributes
- Domains = possible values
- Constraints = product rules (requires, excludes, etc.)

**Solving Approach:**
- Constraint propagation (reduce domains)
- Backtracking search (when propagation alone isn't sufficient)

**Complexity:**
- General CSP is NP-complete
- But many practical CPQ constraints are tractable

### 2.2 SAT (Boolean Satisfiability)

**Definition:** Determines if a boolean formula can be made true by assigning values to variables.

**Characteristics:**
- Very fast solvers exist (CDCL algorithm)
- Requires encoding CPQ constraints as boolean clauses
- All variables must be boolean (or encoded as such)

**Trade-offs:**
- ✅ Extremely optimized solvers (Varisat, Splr, Kissat)
- ✅ Rust ecosystem has mature SAT libraries
- ❌ Encoding CPQ constraints is complex and non-intuitive
- ❌ Explanation generation is harder
- ❌ Numeric constraints require cumbersome encodings

**Verdict:** SAT is the "machine code" of constraint solving. Better for verification, less suitable for interactive CPQ.

### 2.3 SMT (Satisfiability Modulo Theories)

**Definition:** SAT extended with theories (arithmetic, arrays, etc.)

**Characteristics:**
- More expressive than SAT
- Supports linear arithmetic, arrays, bitvectors
- Used heavily in program verification

**Trade-offs:**
- ✅ Richer constraint language than SAT
- ✅ Good for complex arithmetic constraints
- ❌ Heavier than needed for most CPQ constraints
- ❌ Limited Rust library support (mostly Z3 bindings)

**Verdict:** Overkill for Quotey's needs. Better suited for formal verification than CPQ.

### 2.4 Linear Programming (LP/MILP)

**Definition:** Optimize a linear objective subject to linear constraints.

**Characteristics:**
- Good for numeric optimization
- Requires linear constraints

**Trade-offs:**
- ✅ Excellent Rust support (good_lp, microlp, clarabel)
- ✅ Fast for pure optimization problems
- ❌ Poor fit for discrete configuration choices
- ❌ Struggles with requires/excludes constraints

**Verdict:** Useful for pricing optimization, not for configuration validation.

---

## 3. Arc Consistency Algorithms

### 3.1 AC-1, AC-3, AC-4 Comparison

| Algorithm | Time Complexity | Space Complexity | Key Insight |
|-----------|-----------------|------------------|-------------|
| **AC-1** | O(n³d³) | O(nd) | Revises all arcs until no change |
| **AC-3** | O(ed³) | O(e) | Only revises affected arcs (queue-based) |
| **AC-4** | O(ed²) | O(ed) | Uses counters for optimal worst-case |

Where:
- n = number of variables
- d = maximum domain size
- e = number of arcs (constraints)

### 3.2 AC-3 Algorithm Details

```
function AC-3(csp):
    queue = all arcs in csp
    while queue not empty:
        (Xi, Xj) = REMOVE-FIRST(queue)
        if REVISE(Xi, Xj):
            if domain(Xi) is empty: return failure
            for each Xk in neighbors(Xi) except Xj:
                add (Xk, Xi) to queue
    return success

function REVISE(Xi, Xj):
    removed = false
    for each x in domain(Xi):
        if no y in domain(Xj) satisfies constraint(x, y):
            delete x from domain(Xi)
            removed = true
    return removed
```

### 3.3 Why AC-3 for Quotey

1. **Simplicity:** Easier to implement and maintain than AC-4
2. **Good Enough:** For CPQ-sized problems (hundreds of products), AC-3 is fast
3. **Incremental:** Can be run after each user selection for real-time validation
4. **Explanation-Friendly:** Easy to track which constraints removed which values

### 3.4 AC-4 Considerations

AC-4 has better worst-case complexity but:
- Higher initialization cost
- More complex implementation
- Requires maintaining support sets
- Benefits only apparent with very large domains

**Recommendation:** Start with AC-3. If profiling shows constraint propagation as a bottleneck, consider AC-4 or specialized algorithms.

---

## 4. Rust Ecosystem Analysis

### 4.1 Existing Constraint Libraries

| Library | Type | Status | Notes |
|---------|------|--------|-------|
| **cspsolver** | CSP | ⚠️ Yanked | No longer maintained |
| **good_lp** | MILP | ✅ Active | Good for optimization, not configuration |
| **microlp** | LP | ✅ Active | Pure Rust, but linear only |
| **varisat** | SAT | ✅ Active | CDCL SAT solver |
| **splr** | SAT | ✅ Active | Modern CDCL solver |
| **batsat** | SAT | ⚠️ Inactive | MiniSat reimplementation |

### 4.2 Assessment

**No mature CP library exists in Rust.** This means:
- We need to implement our own constraint solver
- But we can learn from existing SAT solver implementations
- Opportunity to create a focused, lightweight CPQ-specific solver

### 4.3 SAT Solvers as Alternative

If we chose SAT:
- **varisat:** Pure Rust, well-documented, maintained
- **splr:** High performance, competition-grade

But encoding CPQ constraints would be complex and explanations harder.

---

## 5. Constraint Types for CPQ

### 5.1 Requires (Binary Constraint)
```
Product A requires Product B
```
Arc consistency handles this naturally.

### 5.2 Excludes (Binary Constraint)
```
Product A excludes Product B
```
Also handled naturally by arc consistency.

### 5.3 Attribute Constraints (Unary)
```
If billing_country is EU, then currency must be EUR
```
Node consistency (simpler than arc consistency).

### 5.4 Quantity Constraints (Numeric)
```
Enterprise tier requires minimum 50 seats
```
May need bounds propagation in addition to arc consistency.

### 5.5 Cross-Product Constraints (Complex)
```
Total seat count across all line items must not exceed license cap
```
May require global constraints or custom propagators.

---

## 6. Recommended Architecture

### 6.1 Core Components

```
┌─────────────────────────────────────────────────────────────┐
│                  Constraint Solver                          │
├─────────────────────────────────────────────────────────────┤
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐       │
│  │   Variable   │  │   Domain     │  │  Constraint  │       │
│  │   Registry   │  │   Store      │  │   Graph      │       │
│  └──────────────┘  └──────────────┘  └──────────────┘       │
├─────────────────────────────────────────────────────────────┤
│  ┌──────────────────────────────────────────────────────┐   │
│  │           Propagation Engine (AC-3)                  │   │
│  │  - Queue-based arc revision                          │   │
│  │  - Incremental updates                               │   │
│  │  - Conflict detection                                  │   │
│  └──────────────────────────────────────────────────────┘   │
├─────────────────────────────────────────────────────────────┤
│  ┌──────────────────────────────────────────────────────┐   │
│  │           Explanation Generator                      │   │
│  │  - Track constraint justifications                   │   │
│  │  - Build human-readable violation messages           │   │
│  └──────────────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────────────┘
```

### 6.2 Data Structures

```rust
// Variable represents a configurable aspect
pub struct Variable {
    pub id: VariableId,
    pub domain: Domain,
    pub assignment: Option<Value>,
}

// Domain of possible values
pub enum Domain {
    Boolean,                           // true/false
    Finite(Vec<Value>),               // enumerated values
    Range(i64, i64),                  // numeric range
}

// Constraint between variables
pub enum Constraint {
    Requires { source: VarId, target: VarId },
    Excludes { source: VarId, target: VarId },
    Attribute { var: VarId, condition: Condition },
    Quantity { var: VarId, min: Option<i64>, max: Option<i64> },
    Custom(Box<dyn CustomConstraint>),
}

// Arc for AC-3 algorithm
pub struct Arc {
    pub from: VarId,
    pub to: VarId,
    pub constraint: ConstraintId,
}
```

### 6.3 Algorithm Pseudocode

```rust
pub fn propagate(&mut self, changed_var: VarId) -> Result<(), Conflict> {
    let mut queue: VecDeque<Arc> = self.arcs_involving(changed_var);
    
    while let Some(arc) = queue.pop_front() {
        if self.revise(&arc)? {
            let domain = self.domain_of(arc.from);
            
            if domain.is_empty() {
                return Err(Conflict::DomainWipeout(arc.from));
            }
            
            // Add affected arcs back to queue
            for neighbor_arc in self.arcs_to(arc.from) {
                if neighbor_arc.from != arc.to {
                    queue.push_back(neighbor_arc);
                }
            }
        }
    }
    
    Ok(())
}

fn revise(&mut self, arc: &Arc) -> Result<bool, Conflict> {
    let constraint = self.constraints.get(arc.constraint);
    let old_size = self.domain_of(arc.from).size();
    
    constraint.filter_domain(
        &mut self.domains[arc.from],
        &self.domains[arc.to]
    )?;
    
    let new_size = self.domain_of(arc.from).size();
    Ok(new_size < old_size)
}
```

---

## 7. Explanation Generation

A key requirement for CPQ is explaining **why** a configuration is invalid.

### 7.1 Approach

1. **Track justifications:** When a value is removed from a domain, record which constraint caused it
2. **Build explanation graphs:** Trace back from conflict to root causes
3. **Generate suggestions:** Offer valid alternatives based on remaining domain values

### 7.2 Example Output

```rust
pub struct ViolationExplanation {
    pub constraint_id: ConstraintId,
    pub message: String,
    pub violated_by: Vec<(VariableId, Value)>,
    pub suggestions: Vec<Suggestion>,
}

// Example:
ViolationExplanation {
    constraint_id: "req_sso_enterprise",
    message: "SSO add-on requires Enterprise tier or above",
    violated_by: vec![
        ("tier", "Professional"),
        ("sso_addon", true)
    ],
    suggestions: vec![
        Suggestion::UpgradeTier("Enterprise"),
        Suggestion::RemoveProduct("sso_addon"),
    ],
}
```

---

## 8. Performance Considerations

### 8.1 Expected Problem Sizes

| Metric | Expected | Maximum |
|--------|----------|---------|
| Variables (products) | 50-200 | 1000 |
| Constraints | 100-500 | 5000 |
| Domain sizes | 2-20 | 100 |

### 8.2 Complexity Analysis

With AC-3:
- Worst case: O(ed³) where e = constraints, d = domain size
- Typical CPQ case: O(1000 × 20³) = 8 million operations
- Modern CPUs: sub-millisecond propagation

### 8.3 Optimization Strategies

1. **Lazy propagation:** Only propagate when user interacts
2. **Constraint indexing:** Fast lookup of relevant constraints
3. **Domain representation:** Bitsets for small domains, ranges for numeric
4. **Incremental solving:** Reuse previous propagation state

---

## 9. Comparison Summary

| Approach | Complexity | Rust Support | CPQ Fit | Recommendation |
|----------|-----------|--------------|---------|----------------|
| **Custom AC-3** | Medium | N/A (we build) | ⭐⭐⭐⭐⭐ | ✅ **Primary** |
| SAT (varisat) | Low (wrapper) | ⭐⭐⭐⭐⭐ | ⭐⭐⭐ | ❌ Encoding too complex |
| SMT (Z3) | Low (bindings) | ⭐⭐⭐ | ⭐⭐⭐ | ❌ Overkill |
| MILP (good_lp) | Low | ⭐⭐⭐⭐⭐ | ⭐⭐ | ❌ Wrong problem type |
| OR-Tools (via FFI) | High | ⭐⭐ | ⭐⭐⭐⭐ | ❌ C++ dependency |

---

## 10. Conclusion and Recommendations

### 10.1 Recommendation: Custom AC-3 Based Solver

We recommend implementing a **custom constraint solver** using:

1. **AC-3 algorithm** for constraint propagation
2. **Domain store** with efficient representation (bitsets/ranges)
3. **Constraint graph** for fast neighbor lookups
4. **Explanation tracking** for user-friendly error messages
5. **Pluggable constraint types** for extensibility

### 10.2 Rationale

1. **No suitable Rust library exists** - we must build our own
2. **CPQ constraints are well-understood** - don't need full CP solver power
3. **AC-3 is proven and well-understood** - 40+ years of research
4. **Custom solver allows CPQ-specific optimizations** - explanations, incremental solving
5. **Tacton's success validates the approach** - constraint-based CPQ works

### 10.3 Implementation Priority

| Phase | Deliverable | Complexity |
|-------|-------------|------------|
| 1 | Basic AC-3 with Requires/Excludes | 2 days |
| 2 | Attribute and Quantity constraints | 1 day |
| 3 | Explanation generation | 2 days |
| 4 | Performance optimization | 2 days |
| 5 | Advanced constraints (cross-product) | 3 days |

### 10.4 Risk Mitigation

| Risk | Mitigation |
|------|------------|
| Performance issues with large catalogs | Benchmark early; fallback to SAT encoding if needed |
| Complex constraints hard to express | Iterative design with product team |
| Explanation quality | User testing; iterate on message generation |

---

## 11. References

1. Mackworth, A.K. (1977). "Consistency in networks of relations." Artificial Intelligence 8.
2. Mohr, R. & Henderson, T.C. (1986). "Arc and path consistency revised." Artificial Intelligence 28.
3. Bessiere, C. (1994). "Arc-consistency and arc-consistency again." Artificial Intelligence 65.
4. Tacton CPQ Documentation (2023). "Constraint-Based Configuration"
5. Choco Solver Documentation (2024). https://choco-solver.org/
6. OR-Tools Documentation (2024). https://developers.google.com/optimization/
7. Barto, L. & Kozik, M. (2014). "Constraint Satisfaction Problems Solvable by Local Consistency Methods." J. ACM 61(1).

---

## Appendix: Example Constraint Models

### A.1 Simple Requires/Excludes

```rust
let mut solver = ConstraintSolver::new();

// Products
let enterprise = solver.add_variable("enterprise_tier", Domain::boolean());
let sso = solver.add_variable("sso_addon", Domain::boolean());

// Constraints
solver.add_constraint(Constraint::requires(sso, enterprise))
      .with_message("SSO requires Enterprise tier");
```

### A.2 Attribute Constraints

```rust
let billing_country = solver.add_variable("billing_country", 
    Domain::finite(vec!["US", "EU", "UK"]));
let currency = solver.add_variable("currency", 
    Domain::finite(vec!["USD", "EUR", "GBP"]));

solver.add_constraint(Constraint::implies(
    billing_country.eq("EU"),
    currency.eq("EUR")
));
```

### A.3 Quantity Constraints

```rust
let tier = solver.add_variable("tier", 
    Domain::finite(vec!["Starter", "Pro", "Enterprise"]));
let seats = solver.add_variable("seats", 
    Domain::range(1, 10000));

solver.add_constraint(Constraint::conditional(
    tier.eq("Enterprise"),
    seats.gte(50)
));
```

---

*End of Research Report*
