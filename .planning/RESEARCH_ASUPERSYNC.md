# Asupersync Research Document

**Research Agent:** ResearchAgent  
**Date:** 2026-02-24  
**Source:** https://github.com/Dicklesworthstone/asupersync  

---

## Executive Summary

**Asupersync** is a spec-first, cancel-correct, capability-secure async runtime for Rust. Created by Dicklesworthstone (same author as MCP Agent Mail), it addresses fundamental flaws in existing async ecosystems by making correctness structural rather than conventional.

### Key Innovation
Unlike Tokio/async-std which provide tools without guarantees, Asupersync enforces:
- **No orphan tasks** - Structured concurrency by construction
- **Cancel-correctness** - Cancellation is a protocol, not a flag
- **Bounded cleanup** - Cleanup budgets are sufficient conditions
- **Deterministic testing** - Lab runtime with virtual time and replay
- **Capability security** - No ambient authority

---

## Core Guarantees

| Guarantee | Description |
|-----------|-------------|
| **No orphan tasks** | Every spawned task is owned by a region; region close waits for all children |
| **Cancel-correctness** | Cancellation is request → drain → finalize, never silent data loss |
| **Bounded cleanup** | Cleanup budgets are sufficient conditions, not hopes |
| **No silent drops** | Two-phase effects (reserve/commit) make data loss impossible |
| **Deterministic testing** | Lab runtime: virtual time, deterministic scheduling, trace replay |
| **Capability security** | All effects flow through explicit `Cx`; no ambient authority |

---

## Architecture Overview

### 1. Structured Concurrency by Construction

```rust
// Asupersync: scope guarantees quiescence
scope.region(|sub| async {
    sub.spawn(task_a);
    sub.spawn(task_b);
}).await;
// ← guaranteed: nothing from inside is still running
```

**Key Concepts:**
- **Regions** own tasks in a tree structure
- When a region closes, all children complete, finalizers run, obligations resolve
- The "no orphans" invariant is enforced by the type system and runtime

### 2. Cancellation as Protocol

```
Running → CancelRequested → Cancelling → Finalizing → Completed(Cancelled)
            ↓                    ↓             ↓
         (bounded)          (cleanup)    (finalizers)
```

**Phases:**
1. **Request** - Propagates down the tree
2. **Drain** - Tasks run to cleanup points (bounded by budgets)
3. **Finalize** - Finalizers run (masked, budgeted)
4. **Complete** - Outcome is `Cancelled(reason)`

### 3. Two-Phase Effects

```rust
// Cancel-safe: reserve then commit
let permit = tx.reserve(cx).await?;  // Can cancel here
permit.send(message);                 // Linear: must happen
```

Dropping a permit aborts cleanly - message never partially sent.

### 4. Capability Security (Cx)

```rust
async fn my_task(cx: &mut Cx) {
    cx.spawn(...);        // Need spawn capability
    cx.sleep_until(...);  // Need time capability
    cx.trace(...);        // Need trace capability
}
```

Swap `Cx` to change interpretation: production vs. lab vs. distributed.

### 5. Lab Runtime (Deterministic Testing)

```rust
#[test]
fn test_cancellation_is_bounded() {
    let lab = LabRuntime::new(LabConfig::default().seed(42));

    lab.run(|cx| async {
        cx.region(|scope| async {
            scope.spawn(task_under_test);
        }).await
    });

    assert!(lab.obligation_leak_oracle().is_ok());
    assert!(lab.quiescence_oracle().is_ok());
}
```

**Features:**
- Virtual time (sleeps complete instantly)
- Deterministic scheduling (same seed → same execution)
- Trace capture/replay
- Schedule exploration (DPOR-class coverage)

---

## "Alien Artifact" Quality Algorithms

### 1. Formal Semantics (Lean)

The runtime has small-step operational semantics with Lean mechanization:
- Budget composes as semiring-like object
- Componentwise `min`, priority as `max`
- Makes "who constrains whom?" algebraic

```
combine(b1, b2) =
  deadline   := min(b1.deadline,   b2.deadline)
  pollQuota  := min(b1.pollQuota,  b2.pollQuota)
  costQuota  := min(b1.costQuota,  b2.costQuota)
  priority   := max(b1.priority,   b2.priority)
```

### 2. DPOR-Style Schedule Exploration

- Uses Mazurkiewicz traces (commutation of independent events)
- Foata fingerprints for equivalence class tracking
- Coverage by equivalence class, not "vibes"

### 3. Anytime-Valid Invariant Monitoring (e-processes)

Uses Ville's inequality:
```
P_H0(∃ t : E_t ≥ 1/α) ≤ α
```

Allows peeking after every scheduling step while controlling type-I error.

---

## Project Structure

```
asupersync/
├── src/
│   ├── lib.rs              # Main exports
│   ├── actor.rs            # Actor model implementation
│   ├── app.rs              # Application framework
│   ├── config.rs           # Configuration management
│   ├── console.rs          # Console/debug interface
│   ├── decoding.rs         # Protocol decoding
│   ├── encoding.rs         # Protocol encoding
│   ├── epoch.rs            # Time/epoch management
│   ├── evidence_sink.rs    # Evidence collection
│   ├── gen_server.rs       # Generic server pattern
│   ├── monitor.rs          # Process monitoring
│   ├── process.rs          # Process management
│   ├── remote.rs           # Remote/distributed support
│   ├── server.rs           # Server implementations
│   ├── session.rs          # Session management
│   ├── signal.rs           # Signal handling
│   ├── spork.rs            # Spork (lightweight process)
│   ├── audit/              # Audit trail
│   ├── bin/                # Binary entry points
│   ├── bytes/              # Byte handling
│   ├── cancel/             # Cancellation primitives
│   ├── channel/            # Channels (two-phase)
│   ├── cli/                # Command line interface
│   ├── codec/              # Codecs
│   ├── combinator/         # Async combinators
│   ├── conformance/        # Conformance testing
│   ├── cx/                 # Capability context (Cx)
│   ├── database/           # Database integration
│   ├── distributed/        # Distributed systems
│   ├── error/              # Error types
│   ├── fs/                 # File system
│   ├── grpc/               # gRPC support
│   ├── http/               # HTTP implementation
│   ├── io/                 # I/O primitives
│   ├── lab/                # Lab runtime (deterministic testing)
│   ├── link/               # Link layer
│   ├── messaging/          # Messaging primitives
│   ├── migration/          # Migration support
│   ├── net/                # Networking
│   ├── obligation/         # Obligation tracking
│   ├── observability/      # Observability/metrics
│   ├── plan/               # Planning/scheduling
│   ├── raptorq/            # RaptorQ (FEC)
│   ├── record/             # Record/replay
│   ├── runtime/            # Core runtime
│   ├── security/           # Security primitives
│   ├── service/            # Service framework
│   ├── stream/             # Streams
│   └── ...
├── asupersync-macros/      # Proc macros
├── conformance/            # Conformance test suite
├── franken_kernel/         # FrankenSuite kernel
├── franken_evidence/       # FrankenSuite evidence
├── franken_decision/       # FrankenSuite decision
├── frankenlab/             # FrankenSuite lab
├── formal/                 # Formal semantics (Lean)
├── fuzz/                   # Fuzzing
├── tests/                  # Integration tests
├── benches/                # Benchmarks
└── docs/                   # Documentation
```

---

## Key Dependencies

| Crate | Purpose |
|-------|---------|
| `thiserror` | Error type derivation |
| `crossbeam-queue` | Lock-free concurrent queues |
| `parking_lot` | Fast synchronization |
| `polling` | Portable epoll/kqueue/IOCP |
| `slab` | Pre-allocated storage |
| `smallvec` | Stack-allocated vectors |
| `pin-project` | Safe pin projections |
| `serde` | Serialization |
| `socket2` | Low-level socket config |
| `rustls` | TLS support (optional) |
| `rusqlite` | SQLite async wrapper |
| `proptest` | Property-based testing |
| `criterion` | Benchmarking |

**Forbidden:** Tokio, hyper, reqwest, axum, async-std, smol (anything that transitively depends on Tokio)

---

## Comparison with Existing Runtimes

| Feature | Tokio | async-std | smol | Asupersync |
|---------|-------|-----------|------|------------|
| Structured concurrency | ❌ | ❌ | ❌ | ✅ |
| Cancel-correctness | ❌ | ❌ | ❌ | ✅ |
| Bounded cleanup | ❌ | ❌ | ❌ | ✅ |
| Deterministic testing | ❌ | ❌ | ❌ | ✅ |
| Capability security | ❌ | ❌ | ❌ | ✅ |
| No ambient authority | ❌ | ❌ | ❌ | ✅ |

---

## Use Cases

1. **High-reliability systems** - Where cancellation bugs are unacceptable
2. **Safety-critical applications** - Where cleanup must complete
3. **Distributed systems** - Where determinism enables replay debugging
4. **Testing concurrent code** - Where determinism makes bugs reproducible
5. **Capability-secure systems** - Where ambient authority is a security risk

---

## Relevance to Quotey

### Potential Integration Points

1. **Execution Queue** - Asupersync's structured concurrency could replace the current execution queue implementation
2. **Cancellation handling** - The CPQ quote lifecycle has cancellation points that could benefit from Asupersync's protocol
3. **Deterministic testing** - The Lab runtime could make CPQ tests reproducible
4. **Audit trail** - Asupersync's evidence/audit system aligns with Quotey's audit requirements

### Migration Considerations

- **Tokio dependency** - Quotey currently uses Tokio; would need significant refactoring
- **Rust edition** - Asupersync requires Rust 2024/nightly
- **Learning curve** - The Cx pattern is different from typical async Rust
- **Maturity** - Asupersync is active development; may have breaking changes

---

## References

- Repository: https://github.com/Dicklesworthstone/asupersync
- Documentation: https://docs.rs/asupersync
- AGENTS.md: Comprehensive agent guidelines
- asupersync_v4_formal_semantics.md: Formal semantics
- TESTING.md: Testing methodology

---

*Document Version: 1.0*  
*Research Agent: ResearchAgent*  
*Status: Complete*
