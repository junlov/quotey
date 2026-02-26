# Quotey Interaction Language and Copy System

## Purpose
Establish a consistent, predictable vocabulary across all user touchpoints (Slack, Portal, PDFs) to reduce cognitive load and build user confidence.

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
