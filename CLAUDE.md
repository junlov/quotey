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

## Current Wave 2 Planning Tracks

- `W2_CLO_CLOSED_LOOP_POLICY_OPTIMIZER_SPEC.md` (CLO): deterministic replay-gated, human-approved policy optimization.
- `W2_NXT_DETERMINISTIC_NEGOTIATION_AUTOPILOT_SPEC.md` (NXT): deterministic negotiation autopilot with bounded counteroffers and approval escalation hooks.

## Project Guardrails From `.planning/PROJECT.md`

- Build a Rust, local-first CPQ agent for Slack (Socket Mode primary, CLI secondary).
- Keep pricing/config/policy/approval decisions deterministic and auditable.
- Treat LLMs as translators for intent extraction and summarization, not as financial decision makers.
- Store configuration rules, pricing policies, and approval thresholds in SQLite (CLI-manageable).
- Maintain quote lifecycle and audit trail integrity across all flows.

````markdown
## UBS Quick Reference for AI Agents

UBS stands for "Ultimate Bug Scanner": **The AI Coding Agent's Secret Weapon: Flagging Likely Bugs for Fixing Early On**

**Install:** `curl -sSL https://raw.githubusercontent.com/Dicklesworthstone/ultimate_bug_scanner/master/install.sh | bash`

**Golden Rule:** `ubs <changed-files>` before every commit. Exit 0 = safe. Exit >0 = fix & re-run.

**Commands:**
```bash
ubs file.ts file2.py                    # Specific files (< 1s) — USE THIS
ubs $(git diff --name-only --cached)    # Staged files — before commit
ubs --only=js,python src/               # Language filter (3-5x faster)
ubs --ci --fail-on-warning .            # CI mode — before PR
ubs --help                              # Full command reference
ubs sessions --entries 1                # Tail the latest install session log
ubs .                                   # Whole project (ignores things like .venv and node_modules automatically)
```

**Output Format:**
```
⚠️  Category (N errors)
    file.ts:42:5 – Issue description
    💡 Suggested fix
Exit code: 1
```
Parse: `file:line:col` → location | 💡 → how to fix | Exit 0/1 → pass/fail

**Fix Workflow:**
1. Read finding → category + fix suggestion
2. Navigate `file:line:col` → view context
3. Verify real issue (not false positive)
4. Fix root cause (not symptom)
5. Re-run `ubs <file>` → exit 0
6. Commit

**Speed Critical:** Scope to changed files. `ubs src/file.ts` (< 1s) vs `ubs .` (30s). Never full scan for small edits.

**Bug Severity:**
- **Critical** (always fix): Null safety, XSS/injection, async/await, memory leaks
- **Important** (production): Type narrowing, division-by-zero, resource leaks
- **Contextual** (judgment): TODO/FIXME, console logs

**Anti-Patterns:**
- ❌ Ignore findings → ✅ Investigate each
- ❌ Full scan per edit → ✅ Scope to file
- ❌ Fix symptom (`if (x) { x.y }`) → ✅ Root cause (`x?.y`)
````
