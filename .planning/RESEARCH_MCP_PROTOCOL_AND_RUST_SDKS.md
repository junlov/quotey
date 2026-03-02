# MCP Protocol + Rust SDK Research (quotey-001-1)

Date: 2026-03-02  
Bead: `quotey-001-1`  
Scope: protocol overview, SDK landscape, implementation recommendation for Quotey

## 1) Protocol overview (what matters for Quotey)

Model Context Protocol (MCP) is a JSON-RPC based protocol for exposing tools/resources/prompts to AI clients through a standard transport (commonly stdio for local agents, HTTP/SSE for remote scenarios). For Quotey, the critical MCP surface is:

- Tool discovery (`tools/list`)
- Tool invocation (`tools/call`)
- Structured errors and deterministic tool outputs
- Optional resources/prompts for read-only context and guided workflows
- Authentication/authorization layered by the host deployment

The official spec supports version negotiation and tracks protocol updates explicitly via the versioning docs/changelog.

## 2) Current spec status (from primary sources)

- MCP versioning/changelog docs currently show a newer protocol line (`2025-11-25`) and a previous stable line (`2025-06-18`) with explicit backward-compatibility guidance for clients/servers.
- This implies servers should be conservative about protocol pinning and plan for compatibility windows rather than hard single-version assumptions.

## 3) Rust SDK landscape

## Option A: Official MCP Rust SDK (`modelcontextprotocol/rust-sdk`)

- Maintained by the MCP organization.
- Repository currently labels the project as early/experimental, with active development status.
- Pros:
  - Highest long-term alignment with the official protocol model
  - Lower risk of semantic drift from spec conventions
- Cons:
  - Early-stage stability risk for production CPQ runtime paths
  - Potential churn while protocol evolves

## Option B: `rmcp` crate (current Quotey choice)

- `rmcp` is a Rust MCP implementation with client/server support and a production-oriented feature set.
- Quotey already uses `rmcp` in `crates/mcp` (server routing + tool handlers are implemented and tested).
- Pros:
  - Already integrated in repo; no migration tax now
  - Practical server ergonomics for tool-first MCP workflows
- Cons:
  - Additional dependency on non-official SDK semantics
  - Must continuously verify protocol-version compatibility against official changes

## Option C: Hand-rolled JSON-RPC MCP implementation

- Maximum control, minimum dependency risk.
- Not recommended for Quotey now due to avoidable maintenance and conformance burden.

## 4) Existing implementation assessment in Quotey

Observed in codebase:

- `crates/mcp` already implements a working MCP server with:
  - Catalog tools
  - Quote create/get/price/list tools
  - Approval request/status/pending tools
  - PDF generation tool
  - API key auth + rate limiting
  - Integration tests for auth/rate-limit behavior
- Server advertises a pinned protocol version in code (`2024-11-05`) and should be reviewed against current spec lines.

Conclusion: `quotey-001-3` (implement server) is substantially underway/implemented; the immediate value is compatibility hardening + productization rather than greenfield server creation.

## 5) Recommendation

Recommended near-term plan:

1. Keep `rmcp` for the current release stream (lowest delivery risk).
2. Add explicit protocol-compatibility gate:
   - Pin supported protocol versions in docs/tests.
   - Add compatibility tests for at least one newer spec line.
3. Add a migration spike bead for official Rust SDK readiness:
   - Trigger migration only when official SDK leaves experimental status and parity checklist is met.
4. Treat MCP tool outputs as stable contracts:
   - Publish JSON schema snapshots for each tool response.
   - Add regression tests for tool payload determinism.

This keeps Quotey shipping while reducing long-term protocol drift risk.

## 6) Estimated complexity and dependencies

- Immediate hardening: **medium** (1-3 focused beads)
  - Protocol version support audit
  - Tool schema snapshot tests
  - Error-model normalization checks
- Official SDK migration: **medium-high**
  - Depends on official Rust SDK maturity and feature parity
  - Requires staged dual-stack or adapter approach for safe cutover

Dependencies:

- `rmcp` lifecycle and compatibility cadence
- Official MCP Rust SDK stabilization progress
- Internal contract tests around `crates/mcp` tool outputs

## 7) Primary references

- MCP spec docs: https://modelcontextprotocol.io/specification/2025-06-18
- MCP versioning/changelog: https://modelcontextprotocol.io/specification/draft/basic/changelog
- Official MCP organization: https://github.com/modelcontextprotocol
- Official Rust SDK repo: https://github.com/modelcontextprotocol/rust-sdk
- MCP servers examples: https://github.com/modelcontextprotocol/servers
- `rmcp` crate docs: https://docs.rs/rmcp/latest/rmcp/
