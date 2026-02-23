# CLAUDE.md — quotey

Claude-specific execution guidance for this repository.

## Planning Source of Truth (Read First)

Before implementing non-trivial work, read:

1. `.planning/config.json`
2. `.planning/PROJECT.md`
3. `AGENTS.md`

If instructions conflict, resolve in this order:
1. Explicit user instruction in this session
2. `AGENTS.md`
3. `.planning/config.json`
4. `.planning/PROJECT.md`

## Planning Config JSON (Current)

```json
{
  "mode": "yolo",
  "depth": "comprehensive",
  "parallelization": true,
  "commit_docs": true,
  "model_profile": "quality",
  "workflow": {
    "research": true,
    "plan_check": true,
    "verifier": true
  }
}
```

## Planning Config JSON (Explanation)

- `mode: "yolo"`: default to execution-first delivery. Implement directly unless blocked by ambiguity.
- `depth: "comprehensive"`: gather sufficient context across code, docs, and constraints before editing.
- `parallelization: true`: run independent discovery and checks in parallel when safe.
- `commit_docs: true`: update docs with behavior/architecture changes in the same work stream.
- `model_profile: "quality"`: optimize for correctness, auditability, and maintainability over speed-only shortcuts.
- `workflow.research: true`: inspect relevant files and constraints before implementation.
- `workflow.plan_check: true`: validate planned edits against `.planning/PROJECT.md` scope and decisions.
- `workflow.verifier: true`: run appropriate verification after edits (tests, lint, targeted checks).

## Project Guardrails From `.planning/PROJECT.md`

- Build a Rust, local-first CPQ agent for Slack (Socket Mode primary, CLI secondary).
- Keep pricing/config/policy/approval decisions deterministic and auditable.
- Treat LLMs as translators for intent extraction and summarization, not as financial decision makers.
- Store configuration rules, pricing policies, and approval thresholds in SQLite (CLI-manageable).
- Maintain quote lifecycle and audit trail integrity across all flows.
