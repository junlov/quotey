use serde::Serialize;

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum TextObject {
    Plain { text: String },
    Mrkdwn { text: String },
}

impl TextObject {
    pub fn plain(text: impl Into<String>) -> Self {
        Self::Plain { text: text.into() }
    }

    pub fn mrkdwn(text: impl Into<String>) -> Self {
        Self::Mrkdwn { text: text.into() }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ButtonStyle {
    Primary,
    Danger,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct ButtonElement {
    pub action_id: String,
    pub text: TextObject,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub style: Option<ButtonStyle>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value: Option<String>,
}

impl ButtonElement {
    pub fn new(action_id: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            action_id: action_id.into(),
            text: TextObject::plain(label),
            style: None,
            value: None,
        }
    }

    pub fn style(mut self, style: ButtonStyle) -> Self {
        self.style = Some(style);
        self
    }

    pub fn value(mut self, value: impl Into<String>) -> Self {
        self.value = Some(value.into());
        self
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Block {
    Section { block_id: String, text: TextObject },
    Actions { block_id: String, elements: Vec<ButtonElement> },
    Context { block_id: String, elements: Vec<TextObject> },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct MessageTemplate {
    pub fallback_text: String,
    pub blocks: Vec<Block>,
}

pub struct MessageBuilder {
    fallback_text: String,
    blocks: Vec<Block>,
}

impl MessageBuilder {
    pub fn new(fallback_text: impl Into<String>) -> Self {
        Self { fallback_text: fallback_text.into(), blocks: Vec::new() }
    }

    pub fn section<F>(mut self, block_id: impl Into<String>, build: F) -> Self
    where
        F: FnOnce(&mut SectionBuilder),
    {
        let mut builder = SectionBuilder::default();
        build(&mut builder);
        self.blocks.push(Block::Section { block_id: block_id.into(), text: builder.build() });
        self
    }

    pub fn actions<F>(mut self, block_id: impl Into<String>, build: F) -> Self
    where
        F: FnOnce(&mut ActionsBuilder),
    {
        let mut builder = ActionsBuilder::default();
        build(&mut builder);
        self.blocks.push(Block::Actions { block_id: block_id.into(), elements: builder.build() });
        self
    }

    pub fn context<F>(mut self, block_id: impl Into<String>, build: F) -> Self
    where
        F: FnOnce(&mut ContextBuilder),
    {
        let mut builder = ContextBuilder::default();
        build(&mut builder);
        self.blocks.push(Block::Context { block_id: block_id.into(), elements: builder.build() });
        self
    }

    pub fn build(self) -> MessageTemplate {
        MessageTemplate { fallback_text: self.fallback_text, blocks: self.blocks }
    }
}

#[derive(Default)]
pub struct SectionBuilder {
    text: Option<TextObject>,
}

impl SectionBuilder {
    pub fn plain(&mut self, text: impl Into<String>) -> &mut Self {
        self.text = Some(TextObject::plain(text));
        self
    }

    pub fn mrkdwn(&mut self, text: impl Into<String>) -> &mut Self {
        self.text = Some(TextObject::mrkdwn(text));
        self
    }

    fn build(self) -> TextObject {
        self.text.unwrap_or_else(|| TextObject::plain(""))
    }
}

#[derive(Default)]
pub struct ActionsBuilder {
    elements: Vec<ButtonElement>,
}

impl ActionsBuilder {
    pub fn button(&mut self, button: ButtonElement) -> &mut Self {
        self.elements.push(button);
        self
    }

    fn build(self) -> Vec<ButtonElement> {
        self.elements
    }
}

#[derive(Default)]
pub struct ContextBuilder {
    elements: Vec<TextObject>,
}

impl ContextBuilder {
    pub fn plain(&mut self, text: impl Into<String>) -> &mut Self {
        self.elements.push(TextObject::plain(text));
        self
    }

    pub fn mrkdwn(&mut self, text: impl Into<String>) -> &mut Self {
        self.elements.push(TextObject::mrkdwn(text));
        self
    }

    fn build(self) -> Vec<TextObject> {
        self.elements
    }
}

pub fn quote_status_message(quote_id: &str, status: &str) -> MessageTemplate {
    MessageBuilder::new(format!("Quote {quote_id} status: {status}"))
        .section("quote.status.header.v1", |section| {
            section.mrkdwn(format!("*Quote:* `{quote_id}`"));
        })
        .section("quote.status.state.v1", |section| {
            section.plain(format!("Current status: {status}"));
        })
        .actions("quote.status.actions.v1", |actions| {
            actions
                .button(
                    ButtonElement::new("quote.refresh.v1", "Refresh")
                        .style(ButtonStyle::Primary)
                        .value(quote_id),
                )
                .button(ButtonElement::new("quote.help.v1", "Help").value("help"));
        })
        .build()
}

pub fn approval_request_message(quote_id: &str, approver_role: &str) -> MessageTemplate {
    MessageBuilder::new(format!("Approval required for quote {quote_id} ({approver_role})"))
        .section("quote.approval.summary.v1", |section| {
            section.mrkdwn(format!(
                "*Approval required*\nQuote `{quote_id}` needs `{approver_role}` review."
            ));
        })
        .actions("quote.approval.actions.v1", |actions| {
            actions
                .button(
                    ButtonElement::new("approval.approve.v1", "Approve")
                        .style(ButtonStyle::Primary)
                        .value(quote_id),
                )
                .button(
                    ButtonElement::new("approval.reject.v1", "Reject")
                        .style(ButtonStyle::Danger)
                        .value(quote_id),
                )
                .button(
                    ButtonElement::new("approval.request_changes.v1", "Request Changes")
                        .value(quote_id),
                );
        })
        .build()
}

/// Rich approval request context for building detailed approval cards
#[derive(Clone, Debug, PartialEq)]
pub struct ApprovalRequestContext {
    pub quote_id: String,
    pub customer_name: String,
    pub quote_value: f64,
    pub discount_percent: f64,
    pub approver_role: String,
    pub approver_name: Option<String>,
    pub requester_name: String,
    pub threshold_percent: f64,
    pub urgency: ApprovalUrgency,
    pub context_lines: Vec<String>,
}

/// Urgency level for styling approval requests
#[derive(Clone, Debug, PartialEq)]
pub enum ApprovalUrgency {
    Normal,
    High,
    Critical,
}

impl ApprovalUrgency {
    fn emoji(&self) -> &'static str {
        match self {
            ApprovalUrgency::Normal => "📋",
            ApprovalUrgency::High => "⚠️",
            ApprovalUrgency::Critical => "🚨",
        }
    }

    fn label(&self) -> &'static str {
        match self {
            ApprovalUrgency::Normal => "Normal",
            ApprovalUrgency::High => "High Priority",
            ApprovalUrgency::Critical => "Critical",
        }
    }
}

/// Rich approval request card with detailed context and emoji actions
#[derive(Clone, Debug, PartialEq)]
pub struct ApprovalRequestCard {
    context: ApprovalRequestContext,
}

impl ApprovalRequestCard {
    /// Create a new approval request card
    pub fn new(context: ApprovalRequestContext) -> Self {
        Self { context }
    }

    /// Render the card as a Slack message template
    pub fn render(&self) -> MessageTemplate {
        let ctx = &self.context;
        let urgency = &ctx.urgency;
        let emoji = urgency.emoji();
        let label = urgency.label();

        let mention =
            ctx.approver_name.as_ref().map(|name| format!("@{} ", name)).unwrap_or_default();

        let fallback = format!(
            "{} Approval required: {} for {} (${:.0}, {:.0}% discount)",
            emoji, ctx.quote_id, ctx.customer_name, ctx.quote_value, ctx.discount_percent
        );

        let mut builder = MessageBuilder::new(&fallback)
            .section("quote.approval_card.header.v1", |section| {
                section.mrkdwn(format!(
                    "{} *Approval Required* • *{}*\n{}{}",
                    emoji, label, mention, ctx.approver_role
                ));
            })
            .section("quote.approval_card.quote_summary.v1", |section| {
                section.mrkdwn(format!(
                    "*Quote:* `{}`\n*Customer:* {}\n*Value:* {}\n*Discount:* {:.1}%",
                    ctx.quote_id,
                    ctx.customer_name,
                    format_currency(ctx.quote_value),
                    ctx.discount_percent
                ));
            })
            .section("quote.approval_card.threshold.v1", |section| {
                section.mrkdwn(format!(
                    "*Threshold exceeded:* {:.0}% discount cap for {}",
                    ctx.threshold_percent, ctx.approver_role
                ));
            });

        // Add context lines if any
        if !ctx.context_lines.is_empty() {
            let context_text = ctx.context_lines.join("\n");
            builder = builder.section("quote.approval_card.context.v1", |section| {
                section.mrkdwn(format!("*Additional context:*\n{}", context_text));
            });
        }

        // Emoji action buttons
        builder = builder.actions("quote.approval_card.emoji_actions.v1", |actions| {
            actions
                .button(
                    ButtonElement::new("approval.approve.emoji.v1", "👍 Approve")
                        .style(ButtonStyle::Primary)
                        .value(&ctx.quote_id),
                )
                .button(
                    ButtonElement::new("approval.reject.emoji.v1", "👎 Reject")
                        .style(ButtonStyle::Danger)
                        .value(&ctx.quote_id),
                )
                .button(
                    ButtonElement::new("approval.discuss.emoji.v1", "💬 Discuss")
                        .value(&ctx.quote_id),
                );
        });

        // Secondary text actions
        builder = builder.actions("quote.approval_card.text_actions.v1", |actions| {
            actions
                .button(
                    ButtonElement::new("approval.view_quote.v1", "View Full Quote")
                        .value(&ctx.quote_id),
                )
                .button(
                    ButtonElement::new("approval.view_policy.v1", "View Policy")
                        .value(&ctx.quote_id),
                );
        });

        builder
            .context("quote.approval_card.footer.v1", |context| {
                context
                    .plain(format!("Requested by {} • Quote {}", ctx.requester_name, ctx.quote_id));
            })
            .build()
    }
}

pub fn error_message(summary: &str, correlation_id: &str) -> MessageTemplate {
    MessageBuilder::new(summary.to_owned())
        .section("quote.error.summary.v1", |section| {
            section.mrkdwn(format!(":warning: {summary}"));
        })
        .context("quote.error.context.v1", |context| {
            context.plain(format!("Correlation ID: {correlation_id}"));
        })
        .build()
}

pub fn help_message() -> MessageTemplate {
    MessageBuilder::new("Quote command help")
        .section("quote.help.summary.v1", |section| {
            section.mrkdwn(
                "*Available commands*\n• `/quote new`\n• `/quote status <quote_id>`\n• `/quote list`\n• `/quote help`",
            );
        })
        .build()
}

pub const DEAL_DNA_MAX_DEALS: usize = 5;

#[derive(Clone, Debug, PartialEq)]
pub struct DealDnaSimilarDeal {
    pub quote_id: String,
    pub customer_name: String,
    pub similarity_score: f64,
    pub outcome: String,
    pub final_price: f64,
    pub discount_percent: f64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct DealDnaCard {
    quote_id: String,
    similar_deals: Vec<DealDnaSimilarDeal>,
}

impl DealDnaCard {
    pub fn new(quote_id: impl Into<String>, similar_deals: Vec<DealDnaSimilarDeal>) -> Self {
        Self { quote_id: quote_id.into(), similar_deals }
    }

    pub fn render(&self) -> MessageTemplate {
        let visible_deals: Vec<&DealDnaSimilarDeal> =
            self.similar_deals.iter().take(DEAL_DNA_MAX_DEALS).collect();
        let shown_count = visible_deals.len();
        let total_count = self.similar_deals.len();

        if visible_deals.is_empty() {
            return MessageBuilder::new(format!("Deal DNA insights for quote {}", self.quote_id))
                .section("quote.deal_dna.header.v1", |section| {
                    section.mrkdwn(format!("📊 *Deal DNA* for `{}`", self.quote_id));
                })
                .section("quote.deal_dna.empty.v1", |section| {
                    section.plain("No similar closed deals found yet.");
                })
                .context("quote.deal_dna.context.v1", |context| {
                    context.plain("Compact thread view is ready once comparable deals exist.");
                })
                .build();
        }

        let mut min_price = f64::INFINITY;
        let mut max_price = f64::NEG_INFINITY;
        let mut min_discount = f64::INFINITY;
        let mut max_discount = f64::NEG_INFINITY;
        let mut wins = 0usize;

        for deal in &visible_deals {
            min_price = min_price.min(deal.final_price);
            max_price = max_price.max(deal.final_price);

            let normalized_discount = deal.discount_percent.clamp(0.0, 100.0);
            min_discount = min_discount.min(normalized_discount);
            max_discount = max_discount.max(normalized_discount);

            if deal.outcome.eq_ignore_ascii_case("won") {
                wins += 1;
            }
        }

        let win_rate = ((wins as f64 / shown_count as f64) * 100.0).round() as u32;
        let deal_lines = visible_deals
            .iter()
            .map(|deal| {
                format!(
                    "• *{}* (`{}`) · 🎯 {} match · {} · 💰 {} · 📉 {:.0}% off",
                    deal.customer_name,
                    deal.quote_id,
                    format_similarity(deal.similarity_score),
                    deal.outcome,
                    format_currency(deal.final_price),
                    deal.discount_percent.clamp(0.0, 100.0)
                )
            })
            .collect::<Vec<_>>()
            .join("\n");

        MessageBuilder::new(format!("Deal DNA insights for quote {}", self.quote_id))
            .section("quote.deal_dna.header.v1", |section| {
                section.mrkdwn(format!("📊 *Deal DNA* for `{}`", self.quote_id));
            })
            .section("quote.deal_dna.summary.v1", |section| {
                section.mrkdwn(format!(
                    "🎯 *Win rate:* {win_rate}% ({wins}/{shown_count})\n💰 *Price range:* {} - {}\n📉 *Discount range:* {:.0}% - {:.0}%",
                    format_currency(min_price),
                    format_currency(max_price),
                    min_discount,
                    max_discount
                ));
            })
            .section("quote.deal_dna.list.v1", |section| {
                section.mrkdwn(format!("*Similar deals (compact)*\n{deal_lines}"));
            })
            .actions("quote.deal_dna.toggle.v1", |actions| {
                actions.button(
                    ButtonElement::new("quote.deal_dna.expand.v1", "Expand Similar Deals")
                        .value(self.quote_id.clone()),
                );
            })
            .actions("quote.deal_dna.details.v1", |actions| {
                for (index, deal) in visible_deals.iter().enumerate() {
                    actions.button(
                        ButtonElement::new(
                            format!("quote.deal_dna.view_details.{}.v1", index + 1),
                            "View Details",
                        )
                        .value(deal.quote_id.clone()),
                    );
                }
            })
            .context("quote.deal_dna.context.v1", |context| {
                context.plain(format!(
                    "Compact thread view: showing {shown_count} of {total_count} similar deals."
                ));
            })
            .build()
    }
}

fn format_currency(value: f64) -> String {
    format!("${value:.0}")
}

fn format_similarity(similarity_score: f64) -> String {
    let normalized = similarity_score.clamp(0.0, 1.0);
    format!("{:.0}%", normalized * 100.0)
}

// Execution Queue Status Types
#[derive(Clone, Debug, PartialEq)]
pub enum ExecutionTaskStatus {
    Queued,
    Running { worker_id: String, started_at: String },
    Completed { result_summary: String },
    RetryableFailed { error: String, retry_count: u32, max_retries: u32 },
    FailedTerminal { error: String },
    Recovered { previous_error: String },
}

/// Build execution task progress message for Slack thread
pub fn execution_task_progress_message(
    quote_id: &str,
    task_id: &str,
    operation_kind: &str,
    status: ExecutionTaskStatus,
) -> MessageTemplate {
    let (icon, status_text, actions) = match &status {
        ExecutionTaskStatus::Queued => (
            "⏳",
            "Queued for processing".to_string(),
            vec![ButtonElement::new("exec.refresh.v1", "Check Status")
                .value(format!("{quote_id}:{task_id}"))],
        ),
        ExecutionTaskStatus::Running { worker_id, started_at } => (
            "🔄",
            format!("Processing (worker: {worker_id}, started: {started_at})"),
            vec![ButtonElement::new("exec.refresh.v1", "Refresh")
                .value(format!("{quote_id}:{task_id}"))],
        ),
        ExecutionTaskStatus::Completed { result_summary } => (
            "✅",
            format!("Completed: {result_summary}"),
            vec![ButtonElement::new("exec.view_result.v1", "View Result")
                .style(ButtonStyle::Primary)
                .value(format!("{quote_id}:{task_id}"))],
        ),
        ExecutionTaskStatus::RetryableFailed { error, retry_count, max_retries } => (
            "⚠️",
            format!("Failed (attempt {retry_count}/{max_retries}): {error}"),
            vec![
                ButtonElement::new("exec.retry_now.v1", "Retry Now")
                    .style(ButtonStyle::Primary)
                    .value(format!("{quote_id}:{task_id}")),
                ButtonElement::new("exec.cancel.v1", "Cancel")
                    .style(ButtonStyle::Danger)
                    .value(format!("{quote_id}:{task_id}")),
            ],
        ),
        ExecutionTaskStatus::FailedTerminal { error } => (
            "❌",
            format!("Failed permanently: {error}"),
            vec![
                ButtonElement::new("exec.view_error.v1", "View Details")
                    .value(format!("{quote_id}:{task_id}")),
                ButtonElement::new("exec.contact_support.v1", "Contact Support")
                    .value(format!("{quote_id}:{task_id}")),
            ],
        ),
        ExecutionTaskStatus::Recovered { previous_error } => (
            "🔄",
            format!("Recovered from: {previous_error}"),
            vec![ButtonElement::new("exec.view_details.v1", "View Details")
                .value(format!("{quote_id}:{task_id}"))],
        ),
    };

    let fallback = format!("Execution {operation_kind} for quote {quote_id}: {status_text}");

    MessageBuilder::new(&fallback)
        .section("exec.status.header.v1", |section| {
            section.mrkdwn(format!("{icon} *{operation_kind}* for `{quote_id}`"));
        })
        .section("exec.status.detail.v1", |section| {
            section.plain(status_text);
        })
        .actions("exec.status.actions.v1", |actions_builder| {
            for button in actions {
                actions_builder.button(button);
            }
        })
        .context("exec.status.context.v1", |context| {
            context.plain(format!("Task ID: {task_id}"));
        })
        .build()
}

/// Build execution summary message showing all tasks for a quote
pub fn execution_summary_message(
    quote_id: &str,
    tasks: &[(String, String, ExecutionTaskStatus)],
) -> MessageTemplate {
    let completed =
        tasks.iter().filter(|(_, _, s)| matches!(s, ExecutionTaskStatus::Completed { .. })).count();
    let failed = tasks
        .iter()
        .filter(|(_, _, s)| matches!(s, ExecutionTaskStatus::FailedTerminal { .. }))
        .count();
    let in_progress = tasks
        .iter()
        .filter(|(_, _, s)| {
            matches!(
                s,
                ExecutionTaskStatus::Queued
                    | ExecutionTaskStatus::Running { .. }
                    | ExecutionTaskStatus::RetryableFailed { .. }
            )
        })
        .count();

    let summary =
        format!("✅ {completed} completed • 🔄 {in_progress} in progress • ❌ {failed} failed");

    let mut builder = MessageBuilder::new(format!("Execution summary for quote {quote_id}"))
        .section("exec.summary.header.v1", |section| {
            section.mrkdwn(format!("*Execution Summary* for `{quote_id}`"));
        })
        .section("exec.summary.stats.v1", |section| {
            section.mrkdwn(summary);
        });

    // Add each task as a context line
    for (task_id, operation_kind, status) in tasks {
        let (icon, status_str) = match status {
            ExecutionTaskStatus::Queued => ("⏳", "queued".to_string()),
            ExecutionTaskStatus::Running { .. } => ("🔄", "running".to_string()),
            ExecutionTaskStatus::Completed { result_summary } => ("✅", result_summary.clone()),
            ExecutionTaskStatus::RetryableFailed { retry_count, max_retries, .. } => {
                ("⚠️", format!("retry {retry_count}/{max_retries}"))
            }
            ExecutionTaskStatus::FailedTerminal { .. } => ("❌", "failed".to_string()),
            ExecutionTaskStatus::Recovered { .. } => ("🔄", "recovered".to_string()),
        };
        builder = builder.context("exec.summary.task.v1", |context| {
            context.plain(format!("{icon} {operation_kind} ({task_id}): {status_str}"));
        });
    }

    builder.build()
}

/// Build recovery notification message
pub fn execution_recovery_message(
    quote_id: &str,
    task_id: &str,
    operation_kind: &str,
    previous_error: &str,
    retry_count: u32,
) -> MessageTemplate {
    MessageBuilder::new(format!("Recovered {operation_kind} for quote {quote_id}"))
        .section("exec.recovery.header.v1", |section| {
            section.mrkdwn(format!("🔄 *Recovered* `{operation_kind}` for `{quote_id}`"));
        })
        .section("exec.recovery.detail.v1", |section| {
            section.plain(format!(
                "Task recovered after transient failure.\nPrevious error: {previous_error}\nRetry attempt: {retry_count}"
            ));
        })
        .actions("exec.recovery.actions.v1", |actions| {
            actions.button(
                ButtonElement::new("exec.view_status.v1", "View Status")
                    .style(ButtonStyle::Primary)
                    .value(format!("{quote_id}:{task_id}")),
            );
        })
        .build()
}

#[cfg(test)]
mod tests {
    use super::{
        approval_request_message, error_message, execution_task_progress_message,
        quote_status_message, Block, ButtonStyle, DealDnaCard, DealDnaSimilarDeal,
        ExecutionTaskStatus, MessageBuilder, TextObject,
    };

    #[test]
    fn message_builder_creates_typed_block_structure() {
        let message = MessageBuilder::new("fallback")
            .section("quote.summary.v1", |section| {
                section.mrkdwn("*Quote Summary*");
            })
            .actions("quote.summary.actions.v1", |actions| {
                actions.button(super::ButtonElement::new("quote.confirm.v1", "Confirm"));
            })
            .build();

        assert_eq!(message.blocks.len(), 2);
        assert!(matches!(
            &message.blocks[0],
            Block::Section {
                block_id,
                text: TextObject::Mrkdwn { .. }
            } if block_id == "quote.summary.v1"
        ));
        assert!(matches!(
            &message.blocks[1],
            Block::Actions { block_id, elements } if block_id == "quote.summary.actions.v1" && elements.len() == 1
        ));
    }

    #[test]
    fn approval_template_has_primary_and_danger_buttons() {
        let message = approval_request_message("Q-2026-0001", "sales_manager");
        assert_eq!(message.blocks.len(), 2);

        let elements = if let Block::Actions { elements, .. } = &message.blocks[1] {
            Some(elements)
        } else {
            None
        };
        assert!(elements.is_some(), "expected actions block");
        let elements = elements.expect("actions block asserted above");
        assert_eq!(elements.len(), 3);
        assert_eq!(
            elements.first().and_then(|element| element.style.clone()),
            Some(ButtonStyle::Primary)
        );
        assert_eq!(
            elements.get(1).and_then(|element| element.style.clone()),
            Some(ButtonStyle::Danger)
        );
    }

    #[test]
    fn error_template_contains_correlation_id() {
        let message = error_message("Cannot process request", "req-123");
        let elements = if let Block::Context { elements, .. } = &message.blocks[1] {
            Some(elements)
        } else {
            None
        };
        assert!(elements.is_some(), "expected context block");
        let elements = elements.expect("context block asserted above");
        assert!(matches!(
            elements.first(),
            Some(TextObject::Plain { text }) if text.contains("req-123")
        ));
    }

    #[test]
    fn quote_status_template_includes_refresh_action() {
        let message = quote_status_message("Q-2026-0042", "draft");
        let elements = if let Block::Actions { elements, .. } = &message.blocks[2] {
            Some(elements)
        } else {
            None
        };
        assert!(elements.is_some(), "expected actions block");
        let elements = elements.expect("actions block asserted above");

        assert!(matches!(
            elements.first(),
            Some(element) if element.action_id == "quote.refresh.v1"
        ));
    }

    #[test]
    fn deal_dna_card_renders_metrics_and_limits_to_top_five() {
        let card = DealDnaCard::new(
            "Q-2026-0001",
            vec![
                DealDnaSimilarDeal {
                    quote_id: "Q-2026-0002".to_string(),
                    customer_name: "Acme".to_string(),
                    similarity_score: 0.91,
                    outcome: "won".to_string(),
                    final_price: 45_000.0,
                    discount_percent: 10.0,
                },
                DealDnaSimilarDeal {
                    quote_id: "Q-2026-0003".to_string(),
                    customer_name: "Globex".to_string(),
                    similarity_score: 0.86,
                    outcome: "lost".to_string(),
                    final_price: 47_000.0,
                    discount_percent: 12.0,
                },
                DealDnaSimilarDeal {
                    quote_id: "Q-2026-0004".to_string(),
                    customer_name: "Initech".to_string(),
                    similarity_score: 0.82,
                    outcome: "won".to_string(),
                    final_price: 51_000.0,
                    discount_percent: 18.0,
                },
                DealDnaSimilarDeal {
                    quote_id: "Q-2026-0005".to_string(),
                    customer_name: "Umbrella".to_string(),
                    similarity_score: 0.80,
                    outcome: "lost".to_string(),
                    final_price: 53_000.0,
                    discount_percent: 22.0,
                },
                DealDnaSimilarDeal {
                    quote_id: "Q-2026-0006".to_string(),
                    customer_name: "Hooli".to_string(),
                    similarity_score: 0.78,
                    outcome: "won".to_string(),
                    final_price: 59_000.0,
                    discount_percent: 28.0,
                },
                DealDnaSimilarDeal {
                    quote_id: "Q-2026-0007".to_string(),
                    customer_name: "Soylent".to_string(),
                    similarity_score: 0.72,
                    outcome: "won".to_string(),
                    final_price: 64_000.0,
                    discount_percent: 31.0,
                },
            ],
        );

        let message = card.render();
        let summary =
            if let Block::Section { text: TextObject::Mrkdwn { text }, .. } = &message.blocks[1] {
                Some(text)
            } else {
                None
            };
        assert!(summary.is_some(), "expected markdown summary section");
        let summary = summary.expect("summary section asserted above");

        assert!(summary.contains("🎯 *Win rate:* 60% (3/5)"));
        assert!(summary.contains("💰 *Price range:* $45000 - $59000"));
        assert!(summary.contains("📉 *Discount range:* 10% - 28%"));

        let list =
            if let Block::Section { text: TextObject::Mrkdwn { text }, .. } = &message.blocks[2] {
                Some(text)
            } else {
                None
            };
        assert!(list.is_some(), "expected markdown list section");
        let list = list.expect("list section asserted above");

        assert!(list.contains("Q-2026-0006"));
        assert!(!list.contains("Q-2026-0007"));

        let detail_actions = if let Block::Actions { block_id, elements } = &message.blocks[4] {
            assert_eq!(block_id, "quote.deal_dna.details.v1");
            Some(elements)
        } else {
            None
        };
        assert!(detail_actions.is_some(), "expected per-deal actions block");
        let detail_actions = detail_actions.expect("actions block asserted above");

        assert_eq!(detail_actions.len(), 5);
        assert_eq!(detail_actions[0].value.as_deref(), Some("Q-2026-0002"));
        assert_eq!(detail_actions[4].value.as_deref(), Some("Q-2026-0006"));
    }

    #[test]
    fn deal_dna_card_renders_empty_state_without_detail_buttons() {
        let message = DealDnaCard::new("Q-2026-404", vec![]).render();

        assert!(message.fallback_text.contains("Q-2026-404"));
        assert!(message.blocks.iter().any(|block| matches!(
            block,
            Block::Section {
                block_id,
                text: TextObject::Plain { text }
            } if block_id == "quote.deal_dna.empty.v1" && text.contains("No similar closed deals")
        )));
        assert!(
            !message.blocks.iter().any(
                |block| matches!(block, Block::Actions { block_id, .. } if block_id == "quote.deal_dna.details.v1")
            ),
            "empty card should not render per-deal detail actions"
        );
    }

    #[test]
    fn execution_task_progress_shows_queued_status() {
        let message = execution_task_progress_message(
            "Q-2026-001",
            "task-123",
            "send_slack_message",
            ExecutionTaskStatus::Queued,
        );

        assert!(message.fallback_text.contains("Queued for processing"));
        assert!(message.blocks.iter().any(|block| matches!(
            block,
            Block::Section { block_id, text: TextObject::Mrkdwn { text } }
            if block_id == "exec.status.header.v1" && text.contains("send_slack_message")
        )));
    }

    #[test]
    fn execution_task_progress_shows_running_status() {
        let message = execution_task_progress_message(
            "Q-2026-002",
            "task-456",
            "generate_pdf",
            ExecutionTaskStatus::Running {
                worker_id: "worker-001".to_string(),
                started_at: "2026-02-23T10:00:00Z".to_string(),
            },
        );

        assert!(message.fallback_text.contains("Processing"));
        assert!(message.fallback_text.contains("worker-001"));
    }

    #[test]
    fn execution_task_progress_shows_completed_status() {
        let message = execution_task_progress_message(
            "Q-2026-003",
            "task-789",
            "crm_sync",
            ExecutionTaskStatus::Completed { result_summary: "Synced 3 records".to_string() },
        );

        assert!(message.fallback_text.contains("Completed"));
        assert!(message.fallback_text.contains("Synced 3 records"));
    }

    #[test]
    fn execution_task_progress_shows_retryable_failed_with_buttons() {
        let message = execution_task_progress_message(
            "Q-2026-004",
            "task-abc",
            "pdf_generation",
            ExecutionTaskStatus::RetryableFailed {
                error: "Network timeout".to_string(),
                retry_count: 1,
                max_retries: 3,
            },
        );

        assert!(message.fallback_text.contains("Failed"));
        assert!(message.fallback_text.contains("attempt 1/3"));

        let actions_block = message.blocks.iter().find(|block| {
            matches!(block, Block::Actions { block_id, .. } if block_id == "exec.status.actions.v1")
        });
        assert!(actions_block.is_some(), "expected actions block for retryable failure");
    }

    #[test]
    fn execution_task_progress_shows_terminal_failure() {
        let message = execution_task_progress_message(
            "Q-2026-005",
            "task-def",
            "validation",
            ExecutionTaskStatus::FailedTerminal { error: "Invalid configuration".to_string() },
        );

        assert!(message.fallback_text.contains("Failed permanently"));
        assert!(message.fallback_text.contains("Invalid configuration"));
    }

    #[test]
    fn approval_request_card_renders_with_emoji_buttons() {
        use super::{ApprovalRequestCard, ApprovalRequestContext, ApprovalUrgency};

        let card = ApprovalRequestCard::new(ApprovalRequestContext {
            quote_id: "Q-2026-001".to_string(),
            customer_name: "Acme Corp".to_string(),
            quote_value: 67_000.0,
            discount_percent: 23.0,
            approver_role: "VP Sales".to_string(),
            approver_name: Some("sarah-vp".to_string()),
            requester_name: "john-ae".to_string(),
            threshold_percent: 20.0,
            urgency: ApprovalUrgency::High,
            context_lines: vec!["Strategic account".to_string(), "2-year commitment".to_string()],
        });

        let message = card.render();

        // Check header contains urgency indicator
        assert!(message.fallback_text.contains("Approval required"));
        assert!(message.fallback_text.contains("Acme Corp"));

        // Find the header section
        let header = message.blocks.iter().find(|block| {
            matches!(block, Block::Section { block_id, .. } if block_id == "quote.approval_card.header.v1")
        });
        assert!(header.is_some(), "expected header section");

        // Check quote summary section exists
        let summary = message.blocks.iter().find(|block| {
            matches!(block, Block::Section { block_id, .. } if block_id == "quote.approval_card.quote_summary.v1")
        });
        assert!(summary.is_some(), "expected quote summary section");

        // Check emoji actions block exists with correct buttons
        let emoji_actions = message.blocks.iter().find(|block| {
            matches!(block, Block::Actions { block_id, .. } if block_id == "quote.approval_card.emoji_actions.v1")
        });
        assert!(emoji_actions.is_some(), "expected emoji actions block");

        if let Block::Actions { elements, .. } = emoji_actions.unwrap() {
            assert_eq!(elements.len(), 3);
            assert!(elements.iter().any(|e| e.action_id == "approval.approve.emoji.v1"));
            assert!(elements.iter().any(|e| e.action_id == "approval.reject.emoji.v1"));
            assert!(elements.iter().any(|e| e.action_id == "approval.discuss.emoji.v1"));
        }

        // Check context lines are included
        let context_section = message.blocks.iter().find(|block| {
            matches!(block, Block::Section { block_id, .. } if block_id == "quote.approval_card.context.v1")
        });
        assert!(context_section.is_some(), "expected context section with additional info");
    }

    #[test]
    fn approval_request_card_normal_urgency_styling() {
        use super::{ApprovalRequestCard, ApprovalRequestContext, ApprovalUrgency};

        let card = ApprovalRequestCard::new(ApprovalRequestContext {
            quote_id: "Q-2026-002".to_string(),
            customer_name: "Globex".to_string(),
            quote_value: 45_000.0,
            discount_percent: 15.0,
            approver_role: "Sales Manager".to_string(),
            approver_name: None,
            requester_name: "jane-ae".to_string(),
            threshold_percent: 15.0,
            urgency: ApprovalUrgency::Normal,
            context_lines: vec![],
        });

        let message = card.render();

        // Normal urgency should not have context section (empty lines)
        let context_section = message.blocks.iter().find(|block| {
            matches!(block, Block::Section { block_id, .. } if block_id == "quote.approval_card.context.v1")
        });
        assert!(
            context_section.is_none(),
            "normal urgency with no context should skip context section"
        );

        // Should still have all action buttons
        let emoji_actions = message.blocks.iter().find(|block| {
            matches!(block, Block::Actions { block_id, .. } if block_id == "quote.approval_card.emoji_actions.v1")
        });
        assert!(emoji_actions.is_some());
    }

    #[test]
    fn approval_request_card_critical_urgency_includes_all_sections() {
        use super::{ApprovalRequestCard, ApprovalRequestContext, ApprovalUrgency};

        let card = ApprovalRequestCard::new(ApprovalRequestContext {
            quote_id: "Q-2026-003".to_string(),
            customer_name: "Initech".to_string(),
            quote_value: 120_000.0,
            discount_percent: 35.0,
            approver_role: "CFO".to_string(),
            approver_name: Some("mike-cfo".to_string()),
            requester_name: "tom-vp".to_string(),
            threshold_percent: 25.0,
            urgency: ApprovalUrgency::Critical,
            context_lines: vec![
                "Competitive situation".to_string(),
                "Customer threatening churn".to_string(),
                "End-of-quarter deal".to_string(),
            ],
        });

        let message = card.render();

        // Should have header, summary, threshold, context, two action blocks, and footer
        assert!(message.blocks.len() >= 6);

        // Verify footer exists with requester info
        let footer = message.blocks.iter().find(|block| {
            matches!(block, Block::Context { block_id, .. } if block_id == "quote.approval_card.footer.v1")
        });
        assert!(footer.is_some(), "expected footer context block");
    }
}
