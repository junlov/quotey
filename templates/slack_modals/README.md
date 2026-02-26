# Slack Modal Templates for Quotey

This directory contains Slack Block Kit modal JSON templates for use with the Quotey Slack bot.

## Available Modals

### 1. Product Suggestions (`product_suggestions.json`)
Displays AI-powered product recommendations when a customer is selected.
- Shows match scores and confidence levels
- One-click add to quote
- Dismiss and search alternatives

### 2. Pricing Rule Builder (`pricing_rule_builder.json`)
Visual interface for creating pricing rules without SQL.
- Condition builder (field, operator, value)
- Action selector with multiple pricing actions
- SQL preview and testing

### 3. Constraint Rule Builder (`constraint_rule_builder.json`)
Define product relationships and constraints.
- Requires/Excludes/Recommends relationships
- Attribute and quantity constraints
- Bundle composition rules

### 4. Discount Policy Builder (`discount_policy_builder.json`)
Create discount limits and approval workflows.
- Tiered discount limits (auto-approve, approval required, hard cap)
- Segment and product category targeting
- Deal size filters

### 5. Quote Confirmation (`quote_confirmation.json`)
Final review before sending a quote.
- Line item summary
- Policy validation results
- Delivery options

### 6. Approval Request (`approval_request.json`)
Submit quotes requiring approval.
- Auto-generated justification
- Deal context and competitive intelligence
- Urgency and expiry settings

## Usage

These templates use [Tera](https://keats.github.io/tera/) templating syntax for dynamic content:

```rust
use tera::Tera;

// Load template
let mut tera = Tera::new("templates/slack_modals/**/*").unwrap();

// Prepare context
let mut context = Context::new();
context.insert("quote", &quote);
context.insert("customer", &customer);
context.insert("suggestions", &suggestions);

// Render
let modal_json = tera.render("product_suggestions.json", &context).unwrap();

// Use with Slack API
let modal_view: serde_json::Value = serde_json::from_str(&modal_json).unwrap();
// Send to Slack views.open API
```

## Block Kit Components Used

- **Header**: Section titles
- **Section**: Text and fields layout
- **Input**: Text inputs, number inputs, datepickers
- **Static Select**: Dropdown menus
- **External Select**: Dynamic product search
- **Users Select**: User selection
- **Button**: Actions
- **Context**: Helpful hints and metadata
- **Divider**: Visual separation

## Customization

Each modal includes:
- Default values for all fields
- Optional vs required field indicators
- Emoji support throughout
- Mobile-optimized layouts

## Integration

These modals integrate with:
- `/quotey suggest` - Product suggestions
- `/quotey rules` - Rule builders
- `/quotey policy` - Discount policies
- Quote workflow actions - Confirmation and approval
