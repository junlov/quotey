# RCH-03: Constraint Solver Algorithm Research

**Research Task:** bd-256v.3  
**Status:** Complete  
**Date:** 2026-02-23

---

## Executive Summary

For Quotey's CPQ constraint engine, we recommend a **custom constraint propagation algorithm** rather than a general-purpose CSP/SAT solver. This approach:

- ✅ Better fits our domain - CPQ constraints are structured, not arbitrary
- ✅ Faster for interactive use - Sub-millisecond validation
- ✅ Simpler to implement - No external dependencies
- ✅ Easier to extend - Native Rust, full control
- ✅ Enables explanation generation - Clear constraint tracing

**Algorithm:** Arc Consistency (AC-3) with forward checking for our specific constraint types.

---

## 1. Constraint Types in CPQ

### 1.1 Our Constraint Taxonomy

Quotey needs to handle these constraint types:

| Constraint Type | Example | Complexity |
|----------------|---------|------------|
| **Requires** | Product A requires Product B | Binary |
| **Excludes** | Product A incompatible with Product C | Binary |
| **Attribute** | If tier="Enterprise", API must be enabled | Unary |
| **Quantity** | Min 50 seats for Enterprise tier | Numeric |
| **Bundle** | Starter pack = Base + Support | Composite |
| **Cross-product** | Total seats ≤ license cap | Global |

### 1.2 Why Not General CSP?

General Constraint Satisfaction Problems (CSPs) are NP-complete. However, CPQ constraints have special structure:

- Binary constraints dominate - Most are product-to-product
- No disjunction - Constraints are AND, not OR
- Finite domains - Product selection is discrete
- Incremental changes - Users add one item at a time

This structure allows much more efficient algorithms than general CSP.

---

## 2. Algorithm Comparison

### 2.1 Option 1: General CSP Solver (Choco, OR-Tools)

**Approach:** Use an existing constraint programming library.

**Verdict:** ❌ Not recommended - Java/C++ dependencies, overkill for our needs.

### 2.2 Option 2: SAT Solver (RustSAT, MiniSat)

**Approach:** Encode constraints as Boolean satisfiability problem.

**Verdict:** ⚠️ Overkill - Numeric constraints are awkward, adds complexity.

### 2.3 Option 3: Linear Programming (Cassowary)

**Approach:** Use Cassowary algorithm (used in UI layout).

**Verdict:** ❌ Wrong domain - Designed for continuous values, not discrete selection.

### 2.4 Option 4: Custom Constraint Propagation ✅ Recommended

**Approach:** Implement domain-specific constraint propagation.

**Verdict:** ✅ **Recommended** - Tailored to our constraint types, fast, native Rust.

---

## 3. Recommended Algorithm: AC-3 with Forward Checking

### 3.1 Algorithm Overview

**Arc Consistency (AC-3)** ensures that for every constraint, all values in the domain have a valid partner.

### 3.2 Simplified for CPQ

Our constraints are simpler than general CSP. We implement direct validation:

```rust
fn validate_config(config: &Configuration, constraints: &[Constraint]) 
    -> ValidationResult {
    let mut violations = Vec::new();
    
    for constraint in constraints {
        match constraint {
            Constraint::Requires { source, target } => {
                if config.has_product(source) && !config.has_product(target) {
                    violations.push(ConstraintViolation {
                        message: format!("{} requires {}", source, target),
                        fix: Some(ConstraintFix::AddProduct(target.clone())),
                    });
                }
            }
            // ... other constraint types
        }
    }
    
    ValidationResult {
        valid: violations.is_empty(),
        violations,
    }
}
```

### 3.3 Explanation Generation

The key differentiator: explaining WHY a configuration is invalid.

```rust
fn explain_why_blocked(
    config: &Configuration,
    blocked_product: ProductId,
    constraints: &[Constraint],
) -> Explanation {
    // Find all constraints that block this product
    // Trace back to root causes
    // Generate suggestions
}
```

---

## 4. Performance Analysis

| Operation | Complexity | Typical Time |
|-----------|-----------|--------------|
| Add product (validation) | O(k) where k = constraints per product | < 1ms |
| Full validation | O(n + m) where n = products, m = constraints | < 10ms |
| Explanation generation | O(d) where d = dependency depth | < 5ms |

**Target:** < 50ms for any operation, < 10ms for incremental validation.

---

## 5. ADR: Constraint Solver Architecture

### Decision

Implement a **custom constraint propagation engine** using AC-3 with forward checking.

### Consequences

**Positive:**
- Optimal performance for our specific constraint types
- Native Rust, no external dependencies
- Easy to add custom constraint types
- Clear explanation generation

**Negative:**
- Implementation effort required
- Must write comprehensive tests

---

## 6. Summary

| Aspect | Recommendation |
|--------|----------------|
| **Algorithm** | Custom AC-3 with forward checking |
| **Performance target** | < 10ms for validation |
| **Implementation** | Native Rust, no external deps |
| **Key feature** | Explanation generation |

**Tacton insight:** Replaced thousands of business rules with just a few hundred constraints. Our approach achieves similar benefits.
