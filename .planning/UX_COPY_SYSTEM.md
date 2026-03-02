# Quotey Interaction Language and Copy System

## Purpose
Establish a consistent, predictable vocabulary across all user touchpoints (Slack, Portal, PDFs) to reduce cognitive load and build user confidence.

---

## Copy Contract (Five Sections)

The interaction contract is fixed to the five sections below. All UX stories must map to one or more sections.

1. `Status Vocabulary` - canonical state words and meanings.
2. `Action Labels` - canonical button/CTA labels.
3. `Message Patterns` - canonical success/warning/error/loading templates.
4. `Assumption + Pricing Disclosure` - canonical assumption cards and numeric format.
5. `Surface Adoption + Conflict Rules` - where tokens are used and how to prevent semantic drift.

---

## Core Principles

1. **Consistency**: Same words mean same things everywhere
2. **Action-Oriented**: Labels describe what happens, not just the control type
3. **Transparency**: Users always know what's happening and why
4. **Recovery**: Every state has a clear path forward

---

## Status Vocabulary (Canonical)

| Status | Meaning | Slack Display | Portal Display | Color |
|--------|---------|---------------|----------------|-------|
| `draft` | Quote created, missing required info | 📝 Draft | Draft | Gray |
| `pending` | Awaiting user input or action | ⏳ Pending | Pending | Yellow |
| `validated` | All required fields present, ready to price | ✅ Validated | Validated | Blue |
| `priced` | Pricing calculated, ready for review | 💰 Priced | Priced | Green |
| `approval` | Requires approval before proceeding | 🔔 Approval Required | Approval Required | Orange |
| `approved` | Approved, ready to finalize | ✓ Approved | Approved | Green |
| `rejected` | Approval denied | ✗ Rejected | Declined | Red |
| `finalized` | Locked, ready to send | 🔒 Finalized | Finalized | Blue |
| `sent` | Delivered to customer | 📧 Sent | Sent | Purple |
| `expired` | Past valid_until date | ⏰ Expired | Expired | Gray |
| `cancelled` | Manually cancelled | 🚫 Cancelled | Cancelled | Gray |

**Rule**: Never deviate from these status labels. If the underlying state changes, update the mapping, not the display text.

---

## Action Button Labels

### Primary Actions (Green/Blue)
| Action | Label | When to Use |
|--------|-------|-------------|
| Create quote | "Create Quote" | Initial quote creation |
| Confirm line items | "Confirm & Price" | After adding/editing lines |
| Request approval | "Request Approval" | When policy requires it |
| Approve quote | "Approve Quote" | Approver action |
| Finalize quote | "Finalize Quote" | After approval |
| Generate PDF | "Generate PDF" | Creating document |
| Send to customer | "Send to Customer" | Delivering quote |

### Secondary Actions (Gray)
| Action | Label | When to Use |
|--------|-------|-------------|
| Edit quote | "Edit" | Modifying existing quote |
| Add line item | "Add Line" | Adding products |
| Remove line | "Remove" | Deleting a line |
| Cancel action | "Cancel" | Aborting current flow |
| Go back | "Back" | Returning to previous step |

### Destructive Actions (Red)
| Action | Label | When to Use |
|--------|-------|-------------|
| Reject quote | "Decline Quote" | Approver rejection |
| Cancel quote | "Cancel Quote" | Abandoning quote |
| Delete line | "Delete" | Removing line item |

---

## Message Patterns

### Confirmation Messages
```
✅ [Action] successful

[Quote ID] is now [status].
[Next action if applicable]
```

Example:
```
✅ Quote priced successfully

Q-2026-0042 is now priced at $20,400.00.
Review the details and click "Confirm" to proceed.
```

### Error Messages
```
⚠️ [What went wrong]

[Why it happened]
[What to do about it]
```

Example:
```
⚠️ Could not calculate pricing

The selected product "Enterprise Plan" requires a minimum quantity of 50 seats.
Update the quantity and try again.
```

### Information Messages
```
ℹ️ [Context/Information]

[Details]
[Action if applicable]
```

Example:
```
ℹ️ Approval required

This quote exceeds the 15% discount threshold for your segment.
Request approval from your sales manager to proceed.
```

### Loading States
```
⏳ [Action]...

[Expected duration or progress indicator]
```

Example:
```
⏳ Generating PDF...

This may take a few seconds.
```

---

## Assumption Disclosure Pattern

When displaying assumed values:

```
┌─────────────────────────────────────┐
│ ⚠️ Assumptions Made                 │
│                                     │
│ • Currency: USD (Assumed)           │
│   Using default currency            │
│                                     │
│ • Tax Rate: 0% (Assumed)            │
│   Tax not applicable or configured  │
│                                     │
│ • Payment Terms: Net 30 (Assumed)   │
│   Using default payment terms       │
└─────────────────────────────────────┘
```

When all values are explicit:
```
┌─────────────────────────────────────┐
│ ✓ All Values Confirmed              │
│                                     │
│ Currency, tax rate, payment terms,  │
│ and billing country have all been   │
│ explicitly set.                     │
└─────────────────────────────────────┘
```

---

## Pricing Display Format

### Currency Format
- Always show currency symbol: `$1,234.56` not `1234.56`
- Use commas for thousands: `$1,000,000.00`
- Show decimal places even for whole amounts: `$100.00`

### Line Items
```
[Product Name]          [Qty] × [Unit Price] = [Line Total]
[SKU if applicable]
[Description if applicable]
```

### Totals Section
```
Subtotal:              $XX,XXX.00
Discount (X%):         -$X,XXX.00
Tax (X%):              $X,XXX.00
───────────────────────────────
Total:                 $XX,XXX.00
```

---

## Slack-Specific Patterns

### Slash Command Responses

**Success**:
```
📋 [Action] Complete

[Summary of what happened]
[Key details]
[Next action buttons]
```

**Error**:
```
❌ [Action] Failed

[Error message]
[Recovery suggestion]
```

### Thread Messages

**Initial response**:
```
[Status emoji] [Quote ID] — [Status]

[Summary]
[Details]
[Buttons]
```

**Follow-up**:
```
[Update description]

[New status/details]
[Updated buttons]
```

---

## Portal-Specific Patterns

### Page Titles
- Quote viewer: "Quote [ID] - [Customer Name]"
- Portal index: "Quote Portal"

### Form Labels
- Use sentence case: "Email address" not "Email Address"
- Required fields marked with * (asterisk)
- Optional fields have "(Optional)" suffix

### Button Placement
- Primary action: Bottom right
- Secondary actions: Bottom left or next to primary
- Destructive actions: Separated from primary actions

---

## Surface Adoption + Conflict Rules

### Key surface adoption map

| Surface | Primary file(s) | Required token families |
|--------|------------------|-------------------------|
| Slack command + thread responses | `crates/slack/src/commands.rs`, `crates/slack/src/blocks.rs` | Status vocabulary, message patterns, action labels |
| Slack event reactions + approvals | `crates/slack/src/events.rs` | Approval/confirmation tokens (`✅`, warning/error families) |
| Portal approval + comment flow | `crates/server/src/portal.rs` | Status vocabulary, error/warning families, action labels |
| Planning baseline/gate docs | `.planning/UX_BASELINE.md`, `.planning/UX_GATE_CHECKLIST.md` | Canonical wording references and checklist IDs |

### Conflict prevention rules

1. A token has exactly one semantic meaning. Example: `approval` always means "waiting for approver decision", never "priced and awaiting review".
2. New aliases must be added to this contract first, then propagated to surfaces.
3. `warning` language (`⚠️`) signals recoverable risk; `error` language (`❌`) signals action failure needing correction.
4. Slack and portal must not map the same backend state to different user-facing statuses.
5. If a mismatch is found, update this file and all impacted surfaces in the same workstream.

### Story linkage rule

Every UX bead/story must include:

```markdown
## Copy Contract Links
- Sections: [Status Vocabulary], [Message Patterns]
- Tokens touched: approval, approved, rejected
- Surfaces touched: crates/slack/src/blocks.rs, crates/server/src/portal.rs
```

No UX story closes without this linkage block plus gate evidence.

---

## Accessibility Requirements

1. **Icons**: Always paired with text labels
2. **Colors**: Never rely on color alone (use icons + text)
3. **Contrast**: Minimum 4.5:1 for normal text
4. **Focus**: Visible focus indicators on all interactive elements

---

## Version History

| Version | Date | Changes |
|---------|------|---------|
| 1.0 | 2026-02-26 | Initial copy system with status vocabulary, action labels, and message patterns |
