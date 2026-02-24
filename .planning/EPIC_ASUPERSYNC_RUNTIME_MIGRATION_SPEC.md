# EPIC Asupersync Runtime Migration Spec

## Purpose
Define scope, migration strategy, safety guardrails, and rollout phases for migrating Quotey from Tokio to Asupersync async runtime to achieve structured concurrency, cancel-correctness, deterministic testing, and capability security.

## Scope
### In Scope
- Replace tokio runtime with asupersync runtime
- Migrate all async functions to use Asupersync Cx (capability context) pattern
- Replace tokio::spawn with asupersync structured regions
- Replace tokio::sync primitives with asupersync two-phase channels
- Replace axum HTTP server with asupersync http module
- Replace tokio::net with asupersync net module
- Replace sqlx async with asupersync database module (or rusqlite sync)
- Add deterministic testing with Lab runtime
- Maintain existing behavior and API contracts

### Out of Scope (for Wave 1)
- Distributed/clustered deployment features
- Advanced asupersync features (gRPC, RaptorQ FEC)
- Lab runtime schedule exploration (DPOR) - basic replay only
- Migration of non-critical path code (can use compatibility shim)

## Rollout Slices
- `Slice A` (foundation): Add asupersync dependency, create runtime adapter layer, establish Cx propagation
- `Slice B` (infrastructure): Migrate HTTP server, networking, database layer
- `Slice C` (core services): Migrate Slack socket mode, agent runtime, execution queue
- `Slice D` (testing): Add Lab runtime tests, deterministic replay, parity verification

`Slice A/B` must be complete before `Slice C`; `Slice C` must be complete before `Slice D` production use.

## KPI Contract
| KPI | Baseline | Wave-1 Target | Owner | Measurement |
|---|---:|---:|---|---|
| Runtime compatibility | 0% | 100% | Platform | `% of existing tests passing` |
| Cancel-correctness | N/A | All tasks | Runtime | `audit: no orphan tasks detected` |
| Test determinism | Non-deterministic | 100% deterministic | QA | `same seed → same execution` |
| Performance regression | Tokio baseline | <= 10% slower | Platform | `latency p95 comparison` |
| Migration coverage | 0% | 100% critical path | Platform | `% of tokio deps removed` |

## Deterministic Safety Constraints
- No tokio::spawn allowed in production code (use asupersync regions)
- All async functions must accept &mut Cx parameter
- All tasks must be spawned within regions (structured concurrency)
- Cancellation must use asupersync protocol (not tokio AbortHandle)
- Database operations must be cancel-safe (two-phase or sync rusqlite)

## Interface Boundaries (Draft)
### Domain Contracts
- `QuoteyContext`: Cx with spawn, time, trace, db capabilities
- `RegionGuard`: scope-based task ownership
- `CancelToken`: cancellation request with bounded cleanup
- `TwoPhaseChannel`: reserve → send (cancel-safe)

### Service Contracts
- `RuntimeAdapter::initialize() -> QuoteyRuntime`
- `QuoteyRuntime::spawn_region(f) -> RegionGuard`
- `QuoteyRuntime::run_main(f) -> Result`
- `QuoteyRuntime::shutdown() -> Result`

### HTTP/Server Contracts
- Replace axum with asupersync http::Server
- Route handlers: `async fn handler(cx: &mut Cx, req: Request) -> Response`
- Middleware: Cx propagation, tracing, cancellation

### Database Contracts
- Option A: rusqlite (sync) wrapped in spawn_blocking
- Option B: asupersync database module (async, cancel-safe)
- All queries must be cancel-safe (no partial writes)

### Slack/Network Contracts
- Socket mode: asupersync net::TcpStream + websockets
- Reconnect logic: structured region with retry policy
- Message handling: spawn per-message region

### Crate Boundaries
- `quotey-core`: Domain logic (mostly sync, minimal Cx changes)
- `quotey-db`: Database adapter (rusqlite or asupersync-db)
- `quotey-slack`: Socket mode with asupersync net, regions for handlers
- `quotey-agent`: Runtime with Cx propagation
- `quotey-server`: HTTP server with asupersync http
- `quotey-cli`: Minimal changes (mostly sync)

## Risk Register
| Risk | Impact | Likelihood | Mitigation | Owner |
|---|---|---|---|---|
| Asupersync API instability | High | Medium | Pin to specific version, fork if needed, abstraction layer | Platform |
| Performance regression | High | Low | Benchmark suite, gradual rollout, keep Tokio fallback | Platform |
| Cancellation bugs (data loss) | Critical | Medium | Audit all cancellation points, two-phase patterns, Lab tests | Runtime |
| Database compatibility | High | Medium | rusqlite fallback option, comprehensive migration tests | Data |
| Learning curve / team ramp-up | Medium | High | Documentation, training, pair programming, gradual adoption | Team |

## Guardrail Checklist (Pre-implementation Exit)
- [ ] Scope and non-goals agreed.
- [ ] KPI owner and metric formula explicitly documented.
- [ ] Deterministic constraints copied into implementation task templates.
- [ ] Interface contracts reviewed against existing crate boundaries.
- [ ] Risks and mitigations acknowledged by feature owner.

## Migration Contract
### Dependency Changes
```toml
# Remove
tokio = { version = "1.43", ... }
axum = "..."

# Add
asupersync = { git = "https://github.com/Dicklesworthstone/asupersync", rev = "..." }
# OR local path if forked
```

### Code Migration Patterns

**Before (Tokio):**
```rust
#[tokio::main]
async fn main() {
    tokio::spawn(task_a());
    tokio::spawn(task_b());
}
```

**After (Asupersync):**
```rust
fn main() {
    asupersync::runtime::run(|cx| async {
        cx.region(|scope| async {
            scope.spawn(|cx| async { task_a(cx).await });
            scope.spawn(|cx| async { task_b(cx).await });
        }).await
    });
}
```

### Database Migration
Option A (Recommended for Wave 1):
```rust
// rusqlite sync with spawn_blocking
cx.spawn_blocking(move || {
    let conn = rusqlite::Connection::open("...")?;
    conn.execute("...", params)
}).await?
```

### Rollback Behavior
- Keep Tokio behind feature flag initially
- Dual-runtime testing period
- Gradual cutover per crate
- Instant rollback via config switch

## Sub-Tasks Overview
| Phase | Task | Description |
|---|---|---|
| 0 | Add asupersync dependency | Workspace Cargo.toml, feature flags |
| 1 | Create runtime adapter | QuoteyRuntime, Cx wrapper |
| 2 | Migrate server/main.rs | Entry point, asupersync::runtime::run |
| 3 | Migrate health.rs | HTTP server with asupersync http |
| 4 | Replace axum routes | asupersync http handlers |
| 5 | Replace tokio::spawn | Structured regions |
| 6 | Replace tokio::net | asupersync net (socket.rs) |
| 7 | Migrate sqlx to rusqlite | Sync DB with spawn_blocking |
| 8 | Add Lab runtime tests | Deterministic testing harness |
| 9 | Remove tokio dependency | Final cleanup, verification |

## Research Document
- `.planning/RESEARCH_ASUPERSYNC.md`: Full technical analysis

## Key Benefits
1. **Structured Concurrency**: No orphan tasks, guaranteed cleanup
2. **Cancel-Correctness**: Protocol-based cancellation, no data loss
3. **Deterministic Testing**: Lab runtime with virtual time and replay
4. **Capability Security**: Explicit effects, no ambient authority
5. **Formal Semantics**: Lean-verified correctness

## Acceptance Criteria
- [ ] All existing tests pass with Asupersync
- [ ] No tokio dependencies in production code
- [ ] Lab runtime tests for critical paths
- [ ] Performance within 10% of Tokio baseline
- [ ] Cancellation stress tests pass
- [ ] Team trained on Asupersync patterns
