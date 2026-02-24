# Spec Document Template - ResearchAgent Prompt

**Use this template when creating formal spec documents for implementation teams.**

---

## Your Mission

Create spec documents in `.planning/` following this exact format so implementation agents can build features with clear contracts, KPIs, and safety guardrails.

---

## Spec Template (Copy and Fill)

```markdown
# [FEATURE_ID] [Feature Name] Spec

## Purpose
Define scope, KPI contract, deterministic guardrails, and interface boundaries for [bead_id] ([feature name]) so [key benefit].

## Scope
### In Scope
- [Specific capability 1]
- [Specific capability 2]
- [Specific capability 3]

### Out of Scope (for Wave 1)
- [Explicitly excluded 1]
- [Explicitly excluded 2]

## Rollout Slices
- `Slice A` (contracts): [what contracts are defined]
- `Slice B` ([layer]): [what is built]
- `Slice C` (runtime): [what service runs]
- `Slice D` ([ux/integration]): [final integration]

`Slice A/B` must be complete before `Slice C`; `Slice C` must be complete before `Slice D` production use.

## KPI Contract
| KPI | Baseline | Wave-1 Target | Owner | Measurement |
|---|---:|---:|---|---|
| [Metric 1] | [baseline] | <= [target] | [Owner] | [formula] |
| [Metric 2] | [baseline] | >= [target] | [Owner] | [formula] |

## Deterministic Safety Constraints
- [Safety rule 1: what the system must never do]
- [Safety rule 2: source of truth requirements]
- [Safety rule 3: LLM boundaries if applicable]

## Interface Boundaries (Draft)
### Domain Contracts
- `[TypeName]`: [field descriptions]

### Service Contracts
- `ServiceName::method(args) -> ReturnType`

### Persistence Contracts
- `RepoName`: [responsibilities]

### Slack Contract
- [How feature appears in Slack]

### Crate Boundaries
- `quotey-core`: [responsibilities]
- `quotey-db`: [responsibilities]
- `quotey-slack`: [responsibilities]
- `quotey-agent`: [responsibilities]

## Risk Register
| Risk | Impact | Likelihood | Mitigation | Owner |
|---|---|---|---|---|
| [Risk description] | High/Med/Low | High/Med/Low | [Mitigation] | [Owner] |

## Guardrail Checklist (Pre-implementation Exit)
- [ ] Scope and non-goals agreed.
- [ ] KPI owner and metric formula explicitly documented.
- [ ] Deterministic constraints copied into implementation task templates.
- [ ] Interface contracts reviewed against existing crate boundaries.
- [ ] Risks and mitigations acknowledged by feature owner.

## Migration Contract
### Schema Additions (`00XX_feature_name`)
- `[table_name]`: [purpose]

### Version and Audit Semantics
- [How versioning works]

### Migration Behavior and Rollback
- [Migration behavior]
```

---

## Steps to Complete

### 1. Check for Existing Research
```bash
ls .planning/RESEARCH_*[FEATURE]*.md
```
Use these as technical input - they contain algorithms, data structures, and implementation sketches.

### 2. Find the Bead ID
```bash
br ready | grep -i [feature]
```

### 3. Claim the Spec Work
```bash
br update [bead_id] --status in_progress
```

### 4. Create the Spec
- Copy template above
- Fill in all sections
- Save as `.planning/[FEATURE_ID]_[NAME]_SPEC.md`
- Use UPPERCASE for FEATURE_ID (e.g., FEAT_01, W1_REL)
- Use snake_case for name

### 5. Commit and Push
```bash
git add .planning/[SPEC].md
git commit -m "Add spec for [FEATURE_ID]"
git push
```

### 6. Update Bead Status
```bash
br update [bead_id] --status done
```

---

## Reference Specs (Copy Structure From)

| Spec | Why Reference It |
|------|------------------|
| `W1_REL_EXECUTION_QUEUE_SPEC.md` | Cleanest structure, good baseline |
| `FEAT_01_DEAL_DNA_SPEC.md` | Feature spec with research integration |
| `W1_EXP_EXPLAIN_ANY_NUMBER_SPEC.md` | Full example with migration contract |
| `W1_FIX_CONSTRAINT_AUTOREPAIR_TRADEOFF_SPEC.md` | Complex feature with risk register |

---

## Critical Rules

1. **NEVER delete files** per AGENTS.md Rule 1
2. Use existing research docs as input, don't duplicate technical content
3. KPIs must have explicit measurement formulas (not just targets)
4. Deterministic safety constraints are non-negotiable - list all that apply
5. All specs must include Risk Register with named owners
6. Migration contract required for any feature with data/persistence changes
7. Interface boundaries must specify which crate owns what

---

## Quick Checklist Before Commit

- [ ] Feature ID matches bead title
- [ ] All 5 spec sections filled (Purpose, Scope, KPIs, Safety, Interfaces)
- [ ] Risk Register has at least 3 risks with mitigations
- [ ] Rollout slices are ordered (A→B→C→D)
- [ ] Migration contract included if touching database
- [ ] File follows naming convention: `[ID]_[NAME]_SPEC.md`

---

## Research → Spec Workflow

```
RESEARCH_*.md ──┐
              ├──> [FEATURE]_SPEC.md ──> Implementation
bead context ───┘         ^
                          │
                    Your work here
```

Research docs contain technical depth. Specs contain contracts and boundaries. Don't duplicate - reference research docs in specs where needed.

---

## Getting Help

- Check existing specs in `.planning/*_SPEC.md` for examples
- Review `RESEARCH_INDEX.md` for related technical research
- Run `bv --robot-triage` to see spec priorities
- Ask in Agent Mail if blocked on bead assignment

---

*Template version: 2026-02-24*
*Maintained by: ResearchAgent collective*
