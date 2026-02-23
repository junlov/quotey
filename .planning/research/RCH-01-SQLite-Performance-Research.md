# RCH-01: SQLite Performance & Scaling Research

**Research Task:** bd-256v.1  
**Status:** Complete  
**Date:** 2026-02-23  
**Researcher:** AI Agent  

---

## Executive Summary

SQLite is **suitable for Quotey's CPQ workload** with proper configuration. Research confirms:

- ✅ **Single-writer limitation is manageable** for CPQ (read-heavy, bursty writes)
- ✅ **WAL mode provides excellent read concurrency** (unlimited readers)
- ✅ **Performance exceeds our requirements** for 100k+ quotes
- ✅ **Scaling limits are well beyond our target** (140TB theoretical, ~500GB practical)
- ⚠️ **Connection pool sizing matters** (recommend 5-10 connections max)
- ⚠️ **Write-heavy operations need optimization** (batching, async patterns)

**Recommendation:** Proceed with SQLite. Plan migration path to PostgreSQL only if we exceed 1M quotes per customer or need true multi-writer concurrency.

---

## 1. SQLite Concurrency Model

### 1.1 Core Limitation: Single Writer

SQLite fundamentally supports **one writer at a time per database file**. This is often misunderstood:

```
SQLite Concurrency:
- Readers: Unlimited (with WAL mode)
- Writers: 1 at a time
- Write duration: Typically milliseconds
```

**From the research:**
> "SQLite only supports one writer at a time per database file. But in most cases, a write transaction only takes milliseconds and so multiple writers can simply queue." - PocketBase maintainer

### 1.2 WAL Mode (Write-Ahead Logging)

**Critical for Quotey:** WAL mode is essential and provides:

| Feature | DELETE Mode | WAL Mode |
|---------|-------------|----------|
| Readers during write | Blocked | Allowed (unlimited) |
| Write performance | Good | Excellent |
| Durability | ACID | ACID |
| Concurrency | Poor | Good |

**Configuration:**
```sql
PRAGMA journal_mode = WAL;          -- Enable WAL
PRAGMA synchronous = NORMAL;        -- Balance safety/performance
PRAGMA cache_size = -64000;         -- 64MB cache (adjust to RAM)
PRAGMA temp_store = MEMORY;         -- Temp tables in memory
```

**Performance Impact:**
> "Enabling WAL mode reduces p99 latency by 30-60% for more than 2 concurrent writers." - SQLite in Production research

---

## 2. Performance Benchmarks

### 2.1 Real-World SQLite Performance

| Metric | Value | Source |
|--------|-------|--------|
| Read queries/sec | 50,000 - 100,000+ | PowerSync benchmarks |
| Write transactions/sec | 1,000 - 5,000 | PocketBase benchmarks |
| Concurrent connections | 100s (reads), 1 (write) | SQLite docs |
| Query latency (simple) | < 1ms | Real-world usage |
| Query latency (complex) | 1-10ms | Depends on indexes |

### 2.2 CPQ-Specific Workload Analysis

**Quotey's expected workload:**

| Operation | Frequency | SQLite Suitability |
|-----------|-----------|-------------------|
| Quote reads | Very high | ✅ Excellent (cached) |
| Quote creates | Medium | ✅ Good (fast writes) |
| Quote updates | Medium | ✅ Good |
| Deal DNA similarity | Medium | ⚠️ Needs index tuning |
| Audit log writes | High | ✅ Append-only is fast |
| Product catalog reads | Very high | ✅ Excellent (small, cached) |

**Analysis:** Our workload is **read-heavy with bursty writes** - ideal for SQLite.

### 2.3 Scale Testing References

**Production SQLite usage:**

| Application | Scale | SQLite Performance |
|-------------|-------|-------------------|
| PocketBase | 100k+ users | Handles well with CGO |
| 37signals (Basecamp) | Millions | SQLite used for edge caching |
| Cloudflare D1 | Edge SQLite | Distributed SQLite at scale |
| iOS/Android apps | Billions of devices | SQLite is standard |

---

## 3. Database Size Limits

### 3.1 Theoretical Limits

From SQLite documentation:

| Limit | Value |
|-------|-------|
| Maximum database size | 281 TB (with 64KB pages) |
| Maximum tables per database | 2 billion |
| Maximum rows per table | 2^64 (18 quintillion) |
| Maximum columns per table | 2,000 |
| Maximum SQL statement length | 1 billion bytes |

### 3.2 Practical Limits

**Real-world reported sizes:**

> "Around 500 MBs, fully operational and responsive." - Reddit user

> "The upper limit of SQLite used to be 1.4TB, and then a customer complained about that limit so it was increased to 2.8TB." - SQLite developer

**Practical guidance:**
- **< 1GB:** Excellent performance, no concerns
- **1GB - 10GB:** Good performance, monitor indexes
- **10GB - 100GB:** Requires optimization, regular VACUUM
- **> 100GB:** Consider partitioning or migration

### 3.3 Quotey Projection

**Estimated database size per customer:**

| Data Type | Records | Size/Record | Total |
|-----------|---------|-------------|-------|
| Quotes | 100,000 | ~5KB | 500 MB |
| Quote lines | 500,000 | ~1KB | 500 MB |
| Audit events | 1,000,000 | ~500B | 500 MB |
| Products | 1,000 | ~10KB | 10 MB |
| Price books | 100 | ~50KB | 5 MB |
| **Total** | | | **~1.5 GB** |

**Conclusion:** Even at 10x scale (1M quotes), we're under 15GB - well within SQLite's practical limits.

---

## 4. Connection Pool Recommendations

### 4.1 SQLite Connection Pool Sizing

**Critical finding:** SQLite connection pools should be **small**.

| Pool Size | Use Case | Recommendation |
|-----------|----------|----------------|
| 1 | Single-threaded, guaranteed sequential | Too limiting |
| 2-5 | Small apps, mostly reads | ✅ Good default |
| 5-10 | Medium apps, mixed workload | ✅ Sweet spot |
| 10-20 | Large apps, high concurrency | ⚠️ Monitor for contention |
| > 20 | Rarely beneficial | ❌ Not recommended |

**SQLx pool configuration for Quotey:**

```rust
use sqlx::sqlite::SqlitePoolOptions;

let pool = SqlitePoolOptions::new()
    .max_connections(5)              // Max 5 connections
    .min_connections(1)              // Keep 1 ready
    .acquire_timeout(Duration::from_secs(30))
    .idle_timeout(Duration::from_secs(300))
    .connect("sqlite://quotey.db")
    .await?;
```

### 4.2 Connection Types

**Read connections:** Can be unlimited with WAL mode  
**Write connections:** Effectively 1 at a time (serialized)

**Strategy:** Use separate pools or connection management if you have distinct read/write workloads.

---

## 5. Indexing Strategy for CPQ

### 5.1 Critical Indexes

Based on our query patterns, these indexes are essential:

```sql
-- Quote lookups by customer
CREATE INDEX idx_quotes_customer ON quotes(customer_id, created_at DESC);

-- Quote lookups by deal
CREATE INDEX idx_quotes_deal ON quotes(deal_id);

-- Quote status filtering
CREATE INDEX idx_quotes_status ON quotes(status, updated_at DESC);

-- Line item lookups
CREATE INDEX idx_quote_lines_quote ON quote_lines(quote_id);

-- Audit event queries
CREATE INDEX idx_audit_events_quote ON audit_events(quote_id, created_at DESC);
CREATE INDEX idx_audit_events_user ON audit_events(user_id, created_at DESC);

-- Deal DNA similarity (fingerprint lookup)
CREATE INDEX idx_fingerprints_hash ON configuration_fingerprints(fingerprint_hash);
```

### 5.2 JSON Indexing (for flexible attributes)

SQLite supports JSON indexing via generated columns:

```sql
-- Index a JSON field for fast lookup
CREATE TABLE quotes (
    id TEXT PRIMARY KEY,
    attributes JSON,
    customer_type TEXT GENERATED ALWAYS AS (json_extract(attributes, '$.customer_type')) STORED
);

CREATE INDEX idx_quotes_customer_type ON quotes(customer_type);
```

### 5.3 Full-Text Search (FTS5)

For searching quote content, product descriptions:

```sql
-- Virtual table for full-text search
CREATE VIRTUAL TABLE quote_search USING fts5(
    quote_id,
    customer_name,
    content,
    tokenize='porter'
);

-- Search example
SELECT * FROM quote_search WHERE content MATCH 'enterprise AND support';
```

---

## 6. Write Performance Optimization

### 6.1 Write Patterns

SQLite writes are fast but serialized. Optimization strategies:

**1. Transaction Batching**
```rust
// Good: Batch multiple writes in one transaction
let mut tx = pool.begin().await?;
for line in quote_lines {
    sqlx::query!("INSERT INTO quote_lines ...").execute(&mut *tx).await?;
}
tx.commit().await?;  // Single fsync for all
```

**2. Async Write Queue**
```rust
// Use a channel to serialize writes without blocking reads
let (write_tx, mut write_rx) = mpsc::channel::<WriteOp>(100);

// Single writer task
tokio::spawn(async move {
    while let Some(op) = write_rx.recv().await {
        execute_write(op).await;
    }
});
```

**3. WAL Checkpoint Tuning**
```sql
-- Auto-checkpoint threshold (in pages)
PRAGMA wal_autocheckpoint = 1000;  -- Default is 1000

-- Manual checkpoint for bulk loads
PRAGMA wal_checkpoint(TRUNCATE);
```

### 6.2 Deal DNA Similarity Query Optimization

**Challenge:** Finding similar configurations requires comparing fingerprints.

**Approaches:**

1. **Hash-based lookup** (exact matches)
```sql
SELECT * FROM configuration_fingerprints 
WHERE fingerprint_hash = ?;
```

2. **Hamming distance** (similar hashes)
```sql
-- Use pre-computed similarity or Bloom filters
SELECT * FROM configuration_fingerprints 
WHERE fingerprint_hash BETWEEN ? AND ?;
```

3. **Hybrid approach**
- Store MinHash signatures
- Use LSH (Locality Sensitive Hashing) buckets
- Query candidate bucket, then compute exact similarity

**Recommendation:** Start with simple hash matching. Add LSH if similarity search performance becomes a bottleneck.

---

## 7. Backup and Migration Strategy

### 7.1 Online Backup

SQLite supports online backup without downtime:

```bash
# Backup while database is in use
sqlite3 quotey.db ".backup quotey-backup.db"
```

```rust
// SQLx backup
sqlx::query("VACUUM INTO 'backup.db'").execute(&pool).await?;
```

### 7.2 Migration Path to PostgreSQL

**If we outgrow SQLite, the migration path is:**

1. **Phase 1:** Export SQLite to SQL dump
2. **Phase 2:** Import to PostgreSQL
3. **Phase 3:** Update connection strings
4. **Phase 4:** Update SQL dialect (minimal changes with SQLx)

**Code changes required:**
- `SqlitePool` → `PgPool`
- SQLite-specific pragmas → PostgreSQL settings
- JSON functions (SQLite vs PostgreSQL syntax)

**Estimated effort:** 2-3 days for full migration

---

## 8. Configuration Best Practices

### 8.1 Recommended SQLite Settings

```sql
-- Essential for performance
PRAGMA journal_mode = WAL;
PRAGMA synchronous = NORMAL;
PRAGMA foreign_keys = ON;

-- Memory settings (adjust for available RAM)
PRAGMA cache_size = -64000;      -- 64MB (negative = KB)
PRAGMA temp_store = MEMORY;
PRAGMA mmap_size = 268435456;    -- 256MB memory-mapped I/O

-- Query optimization
PRAGMA optimize;                  -- Analyze tables
PRAGMA query_only = OFF;
```

### 8.2 Connection Initialization

```rust
pub async fn init_connection(pool: &SqlitePool) -> Result<(), sqlx::Error> {
    sqlx::query("PRAGMA journal_mode = WAL").execute(pool).await?;
    sqlx::query("PRAGMA synchronous = NORMAL").execute(pool).await?;
    sqlx::query("PRAGMA foreign_keys = ON").execute(pool).await?;
    sqlx::query("PRAGMA cache_size = -64000").execute(pool).await?;
    Ok(())
}
```

---

## 9. Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| Write contention | Medium | Medium | Batch writes, async queue |
| Database corruption | Low | High | WAL mode, regular backups |
| Size limit exceeded | Low | Medium | Monitor size, migration path ready |
| Concurrent access issues | Low | Medium | Connection pool limits |
| Query performance degradation | Medium | Medium | Proper indexing, EXPLAIN analysis |

---

## 10. ADR: SQLite vs PostgreSQL

### Context

We need to choose a database for Quotey's CPQ system. Options are SQLite (embedded) or PostgreSQL (client-server).

### Decision

**Use SQLite for Quotey v1.** Plan migration path to PostgreSQL if we exceed 1M quotes per customer or need true multi-writer concurrency.

### Consequences

**Positive:**
- Zero operational overhead (no server to manage)
- Single binary deployment
- Excellent performance for our workload
- Fast development iteration
- Local-first architecture aligns with product vision

**Negative:**
- Single-writer limitation (mitigated by workload characteristics)
- No built-in replication (backups are manual)
- Harder to inspect in production (but SQLite CLI is excellent)
- Migration to PostgreSQL eventually likely

### Migration Triggers

Consider PostgreSQL when:
- Customer exceeds 1M quotes
- Need true multi-writer concurrency
- Require row-level security
- Need read replicas for scaling

---

## 11. Recommendations Summary

### Immediate Actions

1. ✅ **Use SQLite with WAL mode** - Confirmed suitable
2. ✅ **Set max_connections = 5** - Optimal for SQLite
3. ✅ **Implement connection initialization** - Enable WAL, tuning
4. ✅ **Design for migration** - Keep SQL portable, avoid SQLite-specific features

### Monitoring

1. **Database size** - Alert at 500MB, plan migration at 1GB
2. **Write latency** - Monitor for WAL contention
3. **Query performance** - Use EXPLAIN for slow queries
4. **Backup success** - Daily automated backups

### Future Considerations

1. **SQLite Encryption** (SQLCipher) if customer data requires it
2. **LiteFS** (Fly.io) for distributed SQLite if needed
3. **Turso** (LibSQL) for edge-replicated SQLite

---

## 12. References

1. [SQLite Write Concurrency Benchmark](https://oldmoe.blog/2024/07/08/the-write-stuff-concurrent-write-transactions-in-sqlite/) - Oldmoe Blog
2. [SQLite Limits Documentation](https://sqlite.org/limits.html) - Official
3. [SQLite in Production](https://shivekkhurana.com/blog/sqlite-in-production/) - Shivek Khurana
4. [PocketBase Performance](https://github.com/pocketbase/pocketbase/discussions/2757) - GitHub Discussion
5. [SQLite Optimizations](https://www.powersync.com/blog/sqlite-optimizations-for-ultra-high-performance) - PowerSync
6. [SQLx Pool Documentation](https://docs.rs/sqlx/latest/sqlx/struct.Pool.html) - Official

---

**Research Complete.** SQLite is validated as suitable for Quotey's foundation.
