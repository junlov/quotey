# Anti-Regression UX Gate and Definition of Done

## Purpose
Create a mandatory checklist that every UX-related change must pass: clarity, recoverability, correctness, accessibility, and confidence. No change lands without explicit pass/fail evidence.

---

## UX Definition of Done Checklist

Every user-facing change must satisfy ALL checks in this rubric before being considered complete.

### 1. Discoverability (Required: Yes)

| Check | Owner | Pass Criteria |
|-------|-------|---------------|
| D1: New features have discoverable entry points | Developer | Users can find the feature without guessing |
| D2: Command syntax is validated with helpful errors | Developer | Invalid commands return suggestions, not silent failures |
| D3: Button labels are action-oriented and clear | Developer | Labels describe what happens, not just the control type |

### 2. Clarity (Required: Yes)

| Check | Owner | Pass Criteria |
|-------|-------|---------------|
| C1: Status messages use consistent vocabulary | Developer/Designer | Same status means same thing everywhere |
| C2: Pricing shows source for every derived amount | Developer | No "magic numbers" - each amount has traceable source |
| C3: Assumptions are explicit, not implicit | Developer | Users see what's assumed vs confirmed |
| C4: Next action is always visible | Developer | User never wonders "what now?" |
| C5: Error messages explain what went wrong | Developer | Not just "error" - tells user what to do |

### 3. Trust & Confidence (Required: Yes)

| Check | Owner | Pass Criteria |
|-------|-------|---------------|
| T1: Totals match across all surfaces | Developer | Slack, portal, PDF all show same numbers |
| T2: Policy violations show clear reason | Developer | Users understand why approval is needed |
| T3: Confirmation before destructive actions | Developer | No accidental approvals or deletions |
| T4: State transitions are visible | Developer | User sees when state changes |

### 4. Recoverability (Required: Yes)

| Check | Owner | Pass Criteria |
|-------|-------|---------------|
| R1: Interrupted flows can resume | Developer | No data loss on thread break/restart |
| R2: Errors have clear recovery path | Developer | Every error tells user what to do next |
| R3: Undo available for reversible actions | Developer | Can cancel within grace period |
| R4: Invalid input shows correction path | Developer | Not just "invalid" - shows valid options |

### 5. Accessibility (Required: Yes)

| Check | Owner | Pass Criteria |
|-------|-------|---------------|
| A1: Keyboard navigation works | Developer | All key paths traversable without mouse |
| A2: Text contrast meets WCAG AA | Designer | 4.5:1 for normal text, 3:1 for large |
| A3: Form controls have labels | Developer | Every input has associated label |
| A4: Focus order is logical | Developer | Tab order follows visual flow |

### 6. Performance (Required: Yes)

| Check | Owner | Pass Criteria |
|-------|-------|---------------|
| P1: Response time < 2s for actions | Developer | User doesn't see loading > 2 seconds |
| P2: Loading states are visible | Developer | No silent delays without feedback |
| P3: Page transitions are smooth | Designer | No jarring layout shifts |

---

## Anti-Regression Gate Process

### Before any UX change lands:

1. **Identify affected surfaces** - List all Slack messages, portal pages, modals affected
2. **Run through checklist** - Each check must have explicit PASS/FAIL
3. **Document evidence** - Screenshots, test outputs, or logs proving compliance
4. **Peer review** - Second set of eyes verifies checklist completion

### Story linkage requirement (mandatory)

Every new UX story/task must include a `UX Gate Links` section that references the exact checklist IDs it must satisfy (for example `C3`, `T1`, `R2`).

Required template:

```markdown
## UX Gate Links
- Required checks: C1, C3, C4, T1, R2
- Evidence artifact path: .planning/ux_evidence/<story-id>.md
```

If a story does not declare checklist IDs, it is not ready for implementation.

### Closure enforcement rule (mandatory)

No UX task may close unless all linked checklist IDs are marked PASS with evidence.

Required close comment format:

```markdown
UX gate result: PASS
Checks passed: C1, C3, C4, T1, R2
Evidence: .planning/ux_evidence/<story-id>.md
Reviewer: <name>
```

If any linked check is FAIL, the task must remain open.

### Check completion format:

```markdown
## UX Gate Evidence

### Discoverability
- [x] D1: Feature entry point is /quote new command - documented in blocks.rs
- [x] D2: Invalid verb returns suggestion - see commands.rs:13-30
- [x] D3: Button labels are "Confirm", "Edit", "Add Line" - see blocks.rs

### Clarity
- [x] C1: Status vocabulary - see constants in XXX
- [x] C2: Pricing source traceable - see pricing trace output
...
```

---

## Gate Owner Responsibilities

| Role | Responsibility |
|------|----------------|
| Developer | Implements checks, provides evidence |
| Peer Reviewer | Verifies checklist completion |
| Tech Lead | Approves exceptions with risk documentation |

---

## Severity Classification

| Severity | Definition | Example |
|----------|------------|---------|
| Blocker | User cannot complete core workflow | Quote creation broken |
| Critical | Core workflow works but trust is damaged | Wrong totals shown |
| Major | Workflow works but confusing | Unclear next action |
| Minor | Cosmetic or enhancement | Button could be clearer |

**Gate must pass 100% of checks. No Blocker or Critical issues permitted.**

---

## Version History

| Version | Date | Changes |
|---------|------|---------|
| 1.0 | 2026-02-26 | Initial gate with 20+ checks across 6 categories |
