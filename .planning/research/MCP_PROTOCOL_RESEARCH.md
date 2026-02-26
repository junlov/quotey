# MCP Protocol Research for Quotey

**Task:** quotey-001-1: Research MCP protocol and existing SDKs  
**Date:** 2026-02-26  
**Researcher:** Kimi (AI Agent)  

---

## Executive Summary

MCP (Model Context Protocol) is the right choice for enabling AI agents to interact with Quotey programmatically. The official Rust SDK (`rmcp`) is mature (v0.8.0) and production-ready. Implementation complexity is **Medium** - estimated 2-3 weeks for a full server implementation.

**Recommendation:** Use `rmcp` crate with server feature, implement stdio transport for local agent communication.

---

## 1. Protocol Overview

### What is MCP?

Model Context Protocol is an open protocol that standardizes how LLM applications connect to external data sources and tools. It uses **JSON-RPC 2.0** messages over stateful connections.

### Core Concepts

| Concept | Description | For Quotey |
|---------|-------------|------------|
| **Host** | LLM application initiating connections | Claude Code, Kimi CLI |
| **Client** | Connector within host | Built into agent frameworks |
| **Server** | Service providing capabilities | **Quotey MCP Server** |
| **Tools** | Functions for AI to execute | `quote.create`, `catalog.search` |
| **Resources** | Context/data for AI | Product catalog, price books |
| **Prompts** | Templated workflows | Quote creation templates |

### Protocol Features

```
Server can expose:
├── Tools (functions AI can call)
├── Resources (data AI can read)
├── Prompts (templates for interactions)
└── Logging (audit trail)

Client can offer:
├── Sampling (server-initiated LLM calls)
├── Roots (filesystem boundaries)
└── Elicitation (user input requests)
```

---

## 2. Existing SDKs

### Official Rust SDK: `rmcp`

**Repository:** https://github.com/modelcontextprotocol/rust-sdk  
**Latest Version:** 0.8.0  
**License:** MIT/Apache-2.0  
**Async Runtime:** Tokio  

**Key Features:**
- Full protocol implementation
- Server and client support
- Procedural macros for tool generation (`rmcp-macros`)
- Multiple transport options (stdio, TCP, WebSocket)
- OAuth support
- Schema generation via schemars

**Dependencies:**
```toml
rmcp = { version = "0.8.0", features = ["server"] }
tokio = "1.43"
serde = "1.0"
schemars = "0.8"  # For JSON Schema generation
```

### Alternative Rust SDKs

| SDK | Status | Notes |
|-----|--------|-------|
| `mcp-protocol-sdk` | Production | Third-party, claims 100% compliance |
| `rust-mcp-sdk` | Active | Focus on simplicity |
| `prism-mcp-sdk` | v0.1.0 | Production-grade claims |

**Recommendation:** Use official `rmcp` for long-term maintenance and compatibility.

---

## 3. Technical Architecture

### Transport Options

| Transport | Use Case | For Quotey |
|-----------|----------|------------|
| **stdio** | Local processes | ✅ Primary - agents run locally |
| TCP | Networked services | Optional for remote access |
| WebSocket | Browser integration | Future web portal |
| SSE | HTTP streaming | Alternative option |

**Decision:** Stdio transport for local-first design. Quotey runs on user's machine, agents connect via stdin/stdout.

### Server Lifecycle

```
1. Transport established (stdio)
2. Initialization handshake
   - Client sends initialize request
   - Server responds with capabilities
   - Client sends initialized notification
3. Normal operation
   - Tools/Resources/Prompts available
4. Shutdown
   - Graceful cleanup
```

### Capability Negotiation

```json
// Server capabilities
{
  "tools": { "listChanged": true },
  "resources": { 
    "subscribe": true,
    "listChanged": true 
  },
  "prompts": { "listChanged": true },
  "logging": {}
}
```

---

## 4. Tool Design for Quotey

### Recommended Tools (10 total)

| Tool | Purpose | Input | Output |
|------|---------|-------|--------|
| `catalog_search` | Find products | query: string | Product[] |
| `catalog_get` | Get product details | product_id: string | Product |
| `quote_create` | Create new quote | customer_id, lines | Quote |
| `quote_get` | Get quote details | quote_id: string | Quote |
| `quote_price` | Price a quote | quote_id: string | PricingResult |
| `quote_list` | List all quotes | filters?: object | Quote[] |
| `quote_pdf` | Generate PDF | quote_id: string | FilePath |
| `approval_request` | Request approval | quote_id, reason | ApprovalRequest |
| `approval_status` | Check approval status | quote_id: string | ApprovalStatus |
| `approval_pending` | List pending approvals | - | ApprovalRequest[] |

### Tool Schema Example

```rust
#[derive(Debug, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
pub struct QuoteCreateInput {
    pub customer_id: String,
    pub line_items: Vec<LineItemInput>,
    pub notes: Option<String>,
}

#[derive(Debug, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
pub struct LineItemInput {
    pub product_id: String,
    pub quantity: u32,
    pub attributes: Option<serde_json::Value>,
}
```

---

## 5. Implementation Approach

### Recommended Architecture

```
crates/
├── mcp/                    # New crate: quotey-mcp
│   ├── Cargo.toml
│   └── src/
│       ├── lib.rs          # Main exports
│       ├── server.rs       # MCP server implementation
│       ├── tools/          # Tool handlers
│       │   ├── catalog.rs
│       │   ├── quote.rs
│       │   └── approval.rs
│       └── transport.rs    # Transport setup
```

### Dependencies

```toml
[package]
name = "quotey-mcp"
version = "0.1.0"
edition = "2021"

[dependencies]
# MCP
rmcp = { version = "0.8.0", features = ["server"] }

# Quotey internal
core = { path = "../core" }
db = { path = "../db" }

# Async
tokio = { version = "1.43", features = ["rt-multi-thread"] }

# Serialization
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
schemars = "0.8"

# Error handling
anyhow = "1.0"
thiserror = "2.0"

# Logging
tracing = "0.1"
```

### Server Implementation Pattern

```rust
use rmcp::{ServerHandler, model::*};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone)]
pub struct QuoteyMcpServer {
    db_pool: sqlx::SqlitePool,
}

#[rmcp::tool]
impl QuoteyMcpServer {
    #[tool(name = "catalog_search")]
    async fn catalog_search(
        &self,
        #[tool(param)] query: String,
    ) -> Result<CallToolResult, rmcp::Error> {
        // Implementation
    }
}
```

---

## 6. Security Considerations

### Authentication

| Approach | Complexity | Recommendation |
|----------|------------|----------------|
| API Key | Low | ✅ Primary - simple header validation |
| OAuth 2.0 | Medium | Optional for enterprise |
| mTLS | High | Future enhancement |

### Rate Limiting

- Per-key rate limits (requests/minute)
- Configurable via quotey.toml
- 429 responses with retry-after header

### Audit Trail

**Critical requirement:** ALL agent actions must be logged

```rust
// Log to audit_event table
audit_log.record(AuditEvent {
    actor: "agent:kimi",
    actor_type: "agent",
    event_type: "quote.created",
    event_category: "quote",
    payload: json!({...}),
    ...
});
```

---

## 7. Complexity Estimation

| Component | Effort | Notes |
|-----------|--------|-------|
| SDK integration | 2 days | Add rmcp, basic server |
| Tool: catalog | 1 day | Search, get products |
| Tool: quotes | 2 days | Create, get, price, list |
| Tool: approvals | 1 day | Request, status, pending |
| Tool: PDF | 0.5 day | Generate PDF |
| Authentication | 1 day | API key validation |
| Rate limiting | 1 day | Per-key limits |
| Testing | 2 days | Integration tests |
| Documentation | 1 day | API docs |
| **Total** | **~11.5 days** | **~2-3 weeks** |

---

## 8. Next Steps

### Immediate (Task quotey-001-2)

1. Define complete JSON schema for all 10 tools
2. Document input/output types
3. Define error cases and response formats

### Implementation Phase (Task quotey-001-3)

1. Create `quotey-mcp` crate
2. Add `rmcp` dependency
3. Implement basic server with stdio transport
4. Implement each tool incrementally

### Testing Phase (Task quotey-001-5)

1. Test with Claude Code
2. Test with Kimi CLI
3. Verify audit trail completeness

---

## 9. References

- **MCP Specification:** https://modelcontextprotocol.io/specification/2025-06-18
- **Rust SDK:** https://github.com/modelcontextprotocol/rust-sdk
- **Schema:** https://github.com/modelcontextprotocol/specification/blob/main/schema.ts
- **Examples:** https://github.com/modelcontextprotocol/rust-sdk/tree/main/examples

---

## Appendix: MCP vs REST API

| Aspect | MCP | REST API |
|--------|-----|----------|
| **Protocol** | JSON-RPC 2.0 | HTTP + JSON |
| **Transport** | stdio, TCP, WebSocket | HTTP only |
| **Discovery** | Built-in capability negotiation | OpenAPI spec |
| **Streaming** | Native support | Requires SSE/WebSocket |
| **Tool calls** | First-class primitive | Endpoint calls |
| **Context** | Maintained across calls | Stateless |
| **For Agents** | ✅ Native design | Requires wrapper |

**Conclusion:** MCP is the right choice for agent-first architecture.
