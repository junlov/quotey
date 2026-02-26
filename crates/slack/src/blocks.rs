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
    let status_tokens = status.to_ascii_lowercase();
    let (status_icon, status_style, status_hint) = if status_tokens.contains("error")
        || status_tokens.contains("failed")
        || status_tokens.contains("rejected")
        || status_tokens.contains("denied")
    {
        (
            "🚨",
            "requires your attention",
            "Capture the details from this snapshot, then review the specific action path (approve/reject/discuss) before reattempting.",
        )
    } else if status_tokens.contains("approved")
        || status_tokens.contains("complete")
        || status_tokens.contains("finalized")
        || status_tokens.contains("sent")
    {
        (
            "✅",
            "ready",
            "Use `/quote send` or `/quote clone` from this thread context once stakeholders confirm.",
        )
    } else if status_tokens.contains("waiting")
        || status_tokens.contains("pending")
        || status_tokens.contains("requested")
        || status_tokens.contains("processing")
    {
        (
            "🕒",
            "in progress",
            "The workflow is moving through the next deterministic state. Refresh this card to see the latest state.",
        )
    } else {
        (
            "📄",
            "active",
            "You can still query the full thread history with `/quote status` if you'd like a replay trace.",
        )
    };

    MessageBuilder::new(format!("Quote {quote_id} status: {status}"))
        .section("quote.status.header.v1", |section| {
            section
                .mrkdwn(format!("{status_icon} *Quote snapshot:* `{quote_id}` · {status_style}"));
        })
        .section("quote.status.state.v1", |section| {
            section.mrkdwn(format!(
                "*Current status:* {status}\n\
*Decision mode:* {status_style}\n\
*Next step:* {status_hint}"
            ));
        })
        .context("quote.status.context.v1", |context| {
            context.plain("Need guidance? run `/quote help` in this thread.");
            context.plain("All quote state transitions are deterministic and auditable.");
        })
        .actions("quote.status.actions.v1", |actions| {
            actions
                .button(
                    ButtonElement::new("quote.refresh.v1", "Refresh status")
                        .style(ButtonStyle::Primary)
                        .value(quote_id),
                )
                .button(ButtonElement::new("quote.help.v1", "Command help").value("help"));
        })
        .build()
}

#[derive(Clone, Debug, PartialEq)]
pub struct SimulationVariantView {
    pub variant_key: String,
    pub rank_order: i32,
    pub total: f64,
    pub total_delta: f64,
    pub approval_required: bool,
    pub summary: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct SimulationComparisonView {
    pub quote_id: String,
    pub baseline_total: f64,
    pub variants: Vec<SimulationVariantView>,
    pub request_id: String,
}

pub fn simulation_promotion_action_value(quote_id: &str, variant_key: &str) -> String {
    format!(
        "action=promote;quote={};variant={}",
        encode_action_value_component(quote_id),
        encode_action_value_component(variant_key)
    )
}

pub fn simulation_comparison_message(view: &SimulationComparisonView) -> MessageTemplate {
    let fallback = format!(
        "Simulation comparison for {} ({} variant{})",
        view.quote_id,
        view.variants.len(),
        if view.variants.len() == 1 { "" } else { "s" }
    );

    let mut builder = MessageBuilder::new(fallback)
        .section("quote.simulation.header.v1", |section| {
            section.mrkdwn(format!("🧪 *What-if Lab* for `{}`", view.quote_id));
        })
        .section("quote.simulation.baseline.v1", |section| {
            section.mrkdwn(format!("*Baseline total:* {}", format_currency(view.baseline_total)));
        });

    for (index, variant) in view.variants.iter().enumerate() {
        let variant_slug = sanitize_simulation_slug(&variant.variant_key);
        let section_block_id = format!("quote.simulation.variant.{index}.{variant_slug}.v1");
        let actions_block_id = format!("quote.simulation.actions.{index}.{variant_slug}.v1");
        let delta_icon = if variant.total_delta < 0.0 {
            "📉"
        } else if variant.total_delta > 0.0 {
            "📈"
        } else {
            "➖"
        };
        let approval = if variant.approval_required { "approval required" } else { "no approval" };

        builder = builder
            .section(section_block_id, |section| {
                section.mrkdwn(format!(
                    "*#{rank} `{key}`*\nTotal: {total} ({icon} {delta}) • {approval}\n{summary}",
                    rank = variant.rank_order + 1,
                    key = variant.variant_key,
                    total = format_currency(variant.total),
                    icon = delta_icon,
                    delta = format_currency(variant.total_delta),
                    approval = approval,
                    summary = variant.summary
                ));
            })
            .actions(actions_block_id, |actions| {
                actions.button(
                    ButtonElement::new("quote.simulate.promote.v1", "Promote Variant")
                        .style(ButtonStyle::Primary)
                        .value(simulation_promotion_action_value(
                            &view.quote_id,
                            &variant.variant_key,
                        )),
                );
            });
    }

    builder
        .context("quote.simulation.context.v1", |context| {
            context
                .plain(format!("Request ID: {}", view.request_id))
                .plain("Scenario results are hypothetical until a variant is promoted.");
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

#[derive(Clone, Debug, PartialEq)]
pub struct PolicyApprovalPacketView {
    pub packet_id: String,
    pub packet_version: String,
    pub candidate_id: String,
    pub proposed_policy_version: i32,
    pub candidate_diff_summary: String,
    pub replay_evidence_summary: String,
    pub risk_score_bps: i32,
    pub blast_radius_summary: String,
    pub fallback_plan_summary: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PolicyApprovalDecisionKind {
    Approve,
    Reject,
    RequestChanges,
}

impl PolicyApprovalDecisionKind {
    fn as_str(&self) -> &'static str {
        match self {
            Self::Approve => "approve",
            Self::Reject => "reject",
            Self::RequestChanges => "request_changes",
        }
    }

    fn reason_required(&self) -> bool {
        matches!(self, Self::Reject | Self::RequestChanges)
    }
}

pub fn policy_approval_packet_action_value(
    packet: &PolicyApprovalPacketView,
    decision: PolicyApprovalDecisionKind,
) -> String {
    let idempotency_key = format!(
        "pkt:{}:{}:{}:{}:{}",
        encode_action_value_component(&packet.packet_id),
        encode_action_value_component(&packet.packet_version),
        encode_action_value_component(&packet.candidate_id),
        packet.proposed_policy_version,
        decision.as_str()
    );

    format!(
        "action=policy_packet_review;version={};packet={};candidate={};proposed={};decision={};reason_required={};idempotency={}",
        encode_action_value_component(&packet.packet_version),
        encode_action_value_component(&packet.packet_id),
        encode_action_value_component(&packet.candidate_id),
        packet.proposed_policy_version,
        decision.as_str(),
        decision.reason_required(),
        encode_action_value_component(&idempotency_key),
    )
}

pub fn policy_approval_packet_message(packet: &PolicyApprovalPacketView) -> MessageTemplate {
    let fallback = format!(
        "Policy approval packet {} for candidate {} (v{})",
        packet.packet_id, packet.candidate_id, packet.proposed_policy_version
    );

    MessageBuilder::new(fallback)
        .section("policy.packet.header.v1", |section| {
            section.mrkdwn(format!(
                "*Policy Review Packet* `{}`\nCandidate `{}` targeting policy version `{}`",
                packet.packet_id, packet.candidate_id, packet.proposed_policy_version
            ));
        })
        .section("policy.packet.candidate_diff.v1", |section| {
            section.mrkdwn(format!("*Candidate Diff*\n{}", packet.candidate_diff_summary));
        })
        .section("policy.packet.replay_evidence.v1", |section| {
            section.mrkdwn(format!("*Replay Evidence*\n{}", packet.replay_evidence_summary));
        })
        .section("policy.packet.risk.v1", |section| {
            section.mrkdwn(format!(
                "*Risk Score:* {} bps\n*Blast Radius:* {}",
                packet.risk_score_bps, packet.blast_radius_summary
            ));
        })
        .section("policy.packet.fallback.v1", |section| {
            section.mrkdwn(format!("*Fallback Plan*\n{}", packet.fallback_plan_summary));
        })
        .actions("policy.packet.actions.v1", |actions| {
            actions
                .button(
                    ButtonElement::new("policy.packet.approve.v1", "Approve")
                        .style(ButtonStyle::Primary)
                        .value(policy_approval_packet_action_value(
                            packet,
                            PolicyApprovalDecisionKind::Approve,
                        )),
                )
                .button(
                    ButtonElement::new("policy.packet.reject.v1", "Reject (Reason Required)")
                        .style(ButtonStyle::Danger)
                        .value(policy_approval_packet_action_value(
                            packet,
                            PolicyApprovalDecisionKind::Reject,
                        )),
                )
                .button(
                    ButtonElement::new(
                        "policy.packet.request_changes.v1",
                        "Request Changes (Reason Required)",
                    )
                    .value(policy_approval_packet_action_value(
                        packet,
                        PolicyApprovalDecisionKind::RequestChanges,
                    )),
                );
        })
        .context("policy.packet.context.v1", |context| {
            context
                .plain(format!("Packet version: {}", packet.packet_version))
                .plain("Actions are idempotent and version-bound.");
        })
        .build()
}

pub fn error_message(summary: &str, correlation_id: &str) -> MessageTemplate {
    let builder = MessageBuilder::new(summary.to_owned())
        .section("quote.error.summary.v1", |section| {
            section.mrkdwn(format!(":warning: {summary}"));
        })
        .section("quote.error.recovery.v1", |section| {
            section.mrkdwn(
                "*Fast recovery*\n\
1. Re-run the same action with explicit args (for example `/quote status Q-2026-0001`).\n\
2. If this was in a thread, use explicit slash commands in that thread.\n\
3. If you want the full command index, tap **Show supported commands**."
            );
        })
        .context("quote.error.context.v1", |context| {
            context.plain(format!("Correlation ID: {correlation_id}"));
            context.plain(
                "If this issue repeats, share this correlation ID with support and include the exact `/quote` command text.",
            );
        });

    builder
        .actions("quote.error.actions.v1", |actions| {
            actions.button(
                ButtonElement::new("quote.help.v1", "Show supported commands")
                    .style(ButtonStyle::Primary)
                    .value("help"),
            );
        })
        .build()
}

pub fn preview_mode_message(
    operation: &str,
    quote_id: Option<&str>,
    detail: &str,
    request_id: &str,
) -> MessageTemplate {
    let quote_label = quote_id.unwrap_or("this request");
    MessageBuilder::new(format!("Preview mode active: {operation}"))
        .section("quote.preview.header.v1", |section| {
            section.mrkdwn(
                ":warning: *Preview mode active* — deterministic UI intent is captured, but no live quote state changes are persisted yet.",
            );
        })
        .section("quote.preview.detail.v1", |section| {
            section.mrkdwn(format!(
                "*Operation:* `{operation}`\n*Target:* `{quote_label}`\n*Interpretation:* {detail}"
            ));
        })
        .section("quote.preview.enablement.v1", |section| {
            section.mrkdwn(
                "When Slack socket transport and runtime services are connected, this same action will execute deterministically with audit logs and policy routing."
            );
        })
        .context("quote.preview.context.v1", |context| {
            context.plain(format!("Request ID: {request_id}"));
            context.plain("Tip: run `/quote help` for deterministic command examples.");
        })
        .actions("quote.preview.actions.v1", |actions| {
            actions.button(
                ButtonElement::new("quote.help.v1", "Open command guide")
                    .style(ButtonStyle::Primary)
                    .value("help"),
            );
        })
        .build()
}

pub fn command_shortcut_message(command: &str, use_case: &str, example: &str) -> MessageTemplate {
    MessageBuilder::new(format!("Recommended next step: Run command: {command}"))
        .section("quote.command_shortcut.header.v1", |section| {
            section.mrkdwn(format!(
                "🎯 *Recommended next step:* {use_case}\n\
`{command}`"
            ));
        })
        .section("quote.command_shortcut.example.v1", |section| {
            section.mrkdwn(format!("Recommended form:\n`{example}`"));
        })
        .context("quote.command_shortcut.context.v1", |context| {
            context.plain("For best results, use explicit `/quote` commands in the thread.");
        })
        .actions("quote.command_shortcut.actions.v1", |actions| {
            actions.button(
                ButtonElement::new("quote.help.v1", "Open command guide")
                    .style(ButtonStyle::Primary)
                    .value("help"),
            );
        })
        .build()
}

pub fn help_message() -> MessageTemplate {
    MessageBuilder::new("Quotey command guide")
        .section("quote.help.hero.v1", |section| {
            section.mrkdwn(
                ":sparkles: *Quotey Quote Command Center*\nFast, deterministic, and fully auditable quote operations from Slack.",
            );
        })
        .section("quote.help.quickstart.v1", |section| {
            section.mrkdwn(
                "*Recommended start (in-thread or slash)*\n\
1. `/quote new for <customer>` → initialize a quote thread.\n\
2. `/quote status <quote_id>` → inspect state, checkpoints, and constraints.\n\
3. `/quote audit <quote_id>` → review pricing rationale and policy posture.\n\
4. `/quote send <quote_id>` → execute approval + delivery orchestration."
            );
        })
        .section("quote.help.command-reference.v1", |section| {
            section.mrkdwn(
                "*Core command matrix*\n\
• `/quote help` · open this command card\n\
• `/quote list [mine|open|all]` · locate candidate quotes\n\
• `/quote new [for <customer>]` · create a new quote draft\n\
• `/quote edit <quote_id> ...` · mutate quote configuration intent\n\
• `/quote add-line <quote_id> <sku>:<delta>` · apply deterministic line adjustments\n\
• `/quote discount <quote_id> ...` → request discount exceptions with reasoning\n\
• `/quote clone <quote_id>` · duplicate a quote version\n\
• `/quote simulate <quote_id> ...` → run controlled what-if scenarios\n\
• `/quote suggest [<quote_id>] [for <customer>]` → get smart product suggestions"
            );
        })
        .actions("quote.help.quickstart-actions.v1", |actions| {
            actions
                .button(
                    ButtonElement::new("quote.help.command.new.v1", "Start new quote")
                        .style(ButtonStyle::Primary)
                        .value("new"),
                )
                .button(
                    ButtonElement::new("quote.help.command.status.v1", "Check status")
                        .value("status"),
                )
                .button(
                    ButtonElement::new("quote.help.command.list.v1", "List quotes")
                        .value("list"),
                )
                .button(
                    ButtonElement::new("quote.help.command.audit.v1", "Audit quote")
                        .value("audit"),
                )
                .button(
                    ButtonElement::new("quote.help.command.simulate.v1", "Run simulation")
                        .value("simulate"),
                );
        })
        .actions("quote.help.quick-actions.v2", |actions| {
            actions
                .button(
                    ButtonElement::new("quote.help.command.discount.v1", "Request discount")
                        .style(ButtonStyle::Danger)
                        .value("discount"),
                )
                .button(
                    ButtonElement::new("quote.help.command.send.v1", "Send quote")
                        .style(ButtonStyle::Primary)
                        .value("send"),
                )
                .button(
                    ButtonElement::new("quote.help.command.clone.v1", "Clone draft")
                        .value("clone"),
                )
                .button(
                    ButtonElement::new("quote.help.command.edit.v1", "Edit quote")
                        .value("edit"),
                )
                .button(
                    ButtonElement::new("quote.help.command.add-line.v1", "Add/remove line")
                        .value("add-line"),
                );
        })
        .section("quote.help.thread-intelligence.v1", |section| {
            section.mrkdwn(
                "*Thread mode (natural language)*\n\
Use these in active quote threads for fast intent capture:\n\
• \"check status for Q-2026-0001\"\n\
• \"simulate addon:+1 for Q-2026-0001\"\n• \"I need a new quote for Acme\""
            );
        })
        .section("quote.help.interaction-hints.v1", |section| {
            section.mrkdwn(
                "*Interaction tips for deterministic accuracy*\n\
• Use explicit quote IDs: `Q-YYYY-NNNN`\n\
• Use command args for numeric controls: `discount=12.5`, `margin=40`\n\
• Keep line deltas strict: `addon:+1`, `support:-2`\n\
• Emoji approvals in thread: `👍`/`💬` approve or discuss, `👎` reject."
            );
        })
        .section("quote.help.reliability.v1", |section| {
            section.mrkdwn(
                "*Reliability guarantees*\n\
• Deterministic state transitions per quote.\n\
• Replayable audit trail for every action.\n\
• Thread-specific context (`quote_id` + `request_id`) is included in operational messages."
            );
        })
        .section("quote.help.experiment-tips.v1", |section| {
            section.mrkdwn(
                "*Troubleshooting shortcut*\n\
If behavior seems ambiguous, switch to explicit command mode and rerun:\n\
`/quote status <quote_id>` or `/quote help`."
            );
        })
        .context("quote.help.context.v1", |context| {
            context.plain("Tip: explicit inputs dramatically reduce parsing variance for first-pass command quality.");
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
                .value(execution_action_value(quote_id, task_id, "refresh", &status))],
        ),
        ExecutionTaskStatus::Running { worker_id, started_at } => (
            "🔄",
            format!("Processing (worker: {worker_id}, started: {started_at})"),
            vec![ButtonElement::new("exec.refresh.v1", "Refresh")
                .value(execution_action_value(quote_id, task_id, "refresh", &status))],
        ),
        ExecutionTaskStatus::Completed { result_summary } => (
            "✅",
            format!("Completed: {result_summary}"),
            vec![ButtonElement::new("exec.view_result.v1", "View Result")
                .style(ButtonStyle::Primary)
                .value(execution_action_value(quote_id, task_id, "view_result", &status))],
        ),
        ExecutionTaskStatus::RetryableFailed { error, retry_count, max_retries } => (
            "⚠️",
            format!("Failed (attempt {retry_count}/{max_retries}): {error}"),
            vec![
                ButtonElement::new("exec.retry_now.v1", "Retry Now")
                    .style(ButtonStyle::Primary)
                    .value(execution_action_value(quote_id, task_id, "retry_now", &status)),
                ButtonElement::new("exec.cancel.v1", "Cancel")
                    .style(ButtonStyle::Danger)
                    .value(execution_action_value(quote_id, task_id, "cancel", &status)),
            ],
        ),
        ExecutionTaskStatus::FailedTerminal { error } => (
            "❌",
            format!("Failed permanently: {error}"),
            vec![
                ButtonElement::new("exec.view_error.v1", "View Details")
                    .value(execution_action_value(quote_id, task_id, "view_error", &status)),
                ButtonElement::new("exec.contact_support.v1", "Contact Support")
                    .value(execution_action_value(quote_id, task_id, "contact_support", &status)),
            ],
        ),
        ExecutionTaskStatus::Recovered { previous_error } => (
            "🔄",
            format!("Recovered from: {previous_error}"),
            vec![ButtonElement::new("exec.view_details.v1", "View Details")
                .value(execution_action_value(quote_id, task_id, "view_details", &status))],
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
            context
                .plain(format!("Quote: {quote_id}"))
                .plain(format!("Task ID: {task_id}"))
                .plain(format!("Chronology state: {}", execution_status_token(&status)))
                .plain("Controls are idempotent per quote/task/action/state.");
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
        let chronology = execution_status_token(status);
        let block_id = format!(
            "exec.summary.task.{}.v1",
            task_id.replace(|ch: char| !ch.is_ascii_alphanumeric(), "_")
        );
        builder = builder.context(block_id, |context| {
            context
                .plain(format!("{icon} {operation_kind} ({task_id}): {status_str} [{chronology}]"));
        });
    }

    builder
        .context("exec.summary.footer.v1", |context| {
            context.plain(format!(
                "Quote `{quote_id}` thread chronology preserved in listed task order."
            ));
        })
        .build()
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
                    .value(execution_action_value(
                        quote_id,
                        task_id,
                        "view_status",
                        &ExecutionTaskStatus::Recovered {
                            previous_error: previous_error.to_string(),
                        },
                    )),
            );
        })
        .build()
}

fn execution_status_token(status: &ExecutionTaskStatus) -> String {
    match status {
        ExecutionTaskStatus::Queued => "queued".to_string(),
        ExecutionTaskStatus::Running { .. } => "running".to_string(),
        ExecutionTaskStatus::Completed { .. } => "completed".to_string(),
        ExecutionTaskStatus::RetryableFailed { retry_count, max_retries, .. } => {
            format!("retryable_failed_{retry_count}_of_{max_retries}")
        }
        ExecutionTaskStatus::FailedTerminal { .. } => "failed_terminal".to_string(),
        ExecutionTaskStatus::Recovered { .. } => "recovered".to_string(),
    }
}

fn execution_action_value(
    quote_id: &str,
    task_id: &str,
    action: &str,
    status: &ExecutionTaskStatus,
) -> String {
    format!(
        "quote={quote_id};task={task_id};action={action};state={}",
        execution_status_token(status)
    )
}

fn encode_action_value_component(value: &str) -> String {
    let mut encoded = String::with_capacity(value.len());
    for byte in value.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b'~') {
            encoded.push(char::from(byte));
        } else {
            encoded.push_str(&format!("%{byte:02X}"));
        }
    }
    encoded
}

fn sanitize_simulation_slug(value: &str) -> String {
    let slug: String = value
        .chars()
        .map(|ch| if ch.is_ascii_alphanumeric() { ch.to_ascii_lowercase() } else { '_' })
        .collect();

    if slug.is_empty() {
        "variant".to_owned()
    } else {
        slug
    }
}

// -------------------------------------------------------------------------
// Product Suggestion UI
// -------------------------------------------------------------------------

/// Maximum suggestions to display in a single Slack message
pub const SUGGESTION_MAX_DISPLAY: usize = 5;

/// View model for a single product suggestion in Slack
#[derive(Clone, Debug, PartialEq)]
pub struct SuggestionItemView {
    /// Product ID (used in action values)
    pub product_id: String,
    /// Product name
    pub product_name: String,
    /// Product SKU
    pub product_sku: String,
    /// Overall score (0.0 - 1.0)
    pub score: f64,
    /// Confidence label ("High", "Medium", "Low")
    pub confidence: String,
    /// Category description (e.g. "Similar customers purchased this")
    pub category_description: String,
    /// Human-readable reasoning lines
    pub reasoning: Vec<String>,
    /// Unit price for display
    pub unit_price: Option<f64>,
}

/// View model for the suggestion card
#[derive(Clone, Debug, PartialEq)]
pub struct SuggestionCardView {
    /// Quote ID context (if suggestions are for a specific quote)
    pub quote_id: Option<String>,
    /// Customer name or hint
    pub customer_hint: String,
    /// Suggestions to display
    pub suggestions: Vec<SuggestionItemView>,
    /// Request ID for correlation
    pub request_id: String,
}

/// Encode a suggestion action value for button payloads
pub fn suggestion_action_value(
    quote_id: Option<&str>,
    product_id: &str,
    product_sku: &str,
) -> String {
    let quote_part = quote_id
        .map(|q| format!("quote={};", encode_action_value_component(q)))
        .unwrap_or_default();
    format!(
        "action=add_suggested;{}product={};sku={}",
        quote_part,
        encode_action_value_component(product_id),
        encode_action_value_component(product_sku),
    )
}

fn confidence_emoji(confidence: &str) -> &'static str {
    match confidence.to_ascii_lowercase().as_str() {
        "high" => "🟢",
        "medium" => "🟡",
        _ => "🟠",
    }
}

fn score_bar(score: f64) -> String {
    let pct = (score * 100.0).round() as u32;
    let filled = (score * 5.0).round() as usize;
    let empty = 5usize.saturating_sub(filled);
    format!("{}{} {pct}%", "█".repeat(filled), "░".repeat(empty))
}

/// Build the product suggestions Slack message
pub fn suggestion_message(view: &SuggestionCardView) -> MessageTemplate {
    let visible: Vec<&SuggestionItemView> =
        view.suggestions.iter().take(SUGGESTION_MAX_DISPLAY).collect();
    let total = view.suggestions.len();
    let shown = visible.len();

    let quote_label = view.quote_id.as_deref().map(|q| format!(" for `{q}`")).unwrap_or_default();

    if visible.is_empty() {
        return MessageBuilder::new(format!("Product suggestions for {}", view.customer_hint))
            .section("suggest.header.v1", |section| {
                section.mrkdwn(format!(
                    "💡 *Product Suggestions*{quote_label}\nCustomer: {}",
                    view.customer_hint
                ));
            })
            .section("suggest.empty.v1", |section| {
                section.plain(
                    "No suggestions available. Add products to the quote or check customer profile data.",
                );
            })
            .context("suggest.context.v1", |context| {
                context.plain(format!("Request ID: {}", view.request_id));
            })
            .build();
    }

    let mut builder = MessageBuilder::new(format!(
        "{shown} product suggestion{} for {}",
        if shown == 1 { "" } else { "s" },
        view.customer_hint
    ))
    .section("suggest.header.v1", |section| {
        section.mrkdwn(format!(
            "💡 *Product Suggestions*{quote_label}\n*Customer:* {}",
            view.customer_hint
        ));
    });

    for (index, item) in visible.iter().enumerate() {
        let conf_icon = confidence_emoji(&item.confidence);
        let bar = score_bar(item.score);
        let price_str =
            item.unit_price.map(|p| format!(" • {}", format_currency(p))).unwrap_or_default();

        let top_reason = item.reasoning.first().map(|r| r.as_str()).unwrap_or("—");

        let section_id = format!("suggest.item.{index}.v1");
        let actions_id = format!("suggest.item.actions.{index}.v1");

        builder = builder
            .section(section_id, |section| {
                section.mrkdwn(format!(
                    "*#{rank} {name}* (`{sku}`){price}\n\
                     {conf_icon} {confidence} confidence • {bar}\n\
                     _{category}_\n\
                     {reason}",
                    rank = index + 1,
                    name = item.product_name,
                    sku = item.product_sku,
                    price = price_str,
                    confidence = item.confidence,
                    category = item.category_description,
                    reason = top_reason,
                ));
            })
            .actions(actions_id, |actions| {
                actions
                    .button(
                        ButtonElement::new(format!("suggest.add.{index}.v1"), "Add to Quote")
                            .style(ButtonStyle::Primary)
                            .value(suggestion_action_value(
                                view.quote_id.as_deref(),
                                &item.product_id,
                                &item.product_sku,
                            )),
                    )
                    .button(
                        ButtonElement::new(format!("suggest.details.{index}.v1"), "View Details")
                            .value(format!(
                                "action=view_product;product={}",
                                encode_action_value_component(&item.product_id)
                            )),
                    );
            });
    }

    if total > shown {
        builder = builder.section("suggest.overflow.v1", |section| {
            section.mrkdwn(format!(
                "_Showing {shown} of {total} suggestions. Use `/quote suggest --all` for the full list._"
            ));
        });
    }

    builder
        .context("suggest.context.v1", |context| {
            context
                .plain(format!("Request ID: {}", view.request_id))
                .plain("Suggestions are scored by customer similarity, product relationships, recency, and business rules.");
        })
        .build()
}

#[cfg(test)]
mod tests {
    use super::{
        approval_request_message, error_message, execution_task_progress_message,
        policy_approval_packet_action_value, policy_approval_packet_message, preview_mode_message,
        quote_status_message, simulation_comparison_message, simulation_promotion_action_value,
        Block, ButtonStyle, DealDnaCard, DealDnaSimilarDeal, ExecutionTaskStatus, MessageBuilder,
        PolicyApprovalDecisionKind, PolicyApprovalPacketView, SimulationComparisonView,
        SimulationVariantView, TextObject,
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
        let elements = message.blocks.iter().find_map(|block| {
            if let Block::Context { elements, .. } = block {
                Some(elements)
            } else {
                None
            }
        });
        assert!(elements.is_some(), "expected context block");
        let elements = elements.expect("context block asserted above");
        assert!(matches!(
            elements.first(),
            Some(TextObject::Plain { text }) if text.contains("req-123")
        ));
    }

    #[test]
    fn preview_mode_message_flags_execution_is_not_live() {
        let message = preview_mode_message(
            "/quote status",
            Some("Q-2026-5555"),
            "status lookup request captured",
            "req-preview-1",
        );

        assert!(message.fallback_text.contains("Preview mode active"));
        assert!(message.blocks.iter().any(|block| matches!(
            block,
            Block::Section { block_id, .. } if block_id == "quote.preview.header.v1"
        )));
        assert!(message.blocks.iter().any(|block| matches!(
            block,
            Block::Context { block_id, .. } if block_id == "quote.preview.context.v1"
        )));
    }

    #[test]
    fn quote_status_template_includes_refresh_action() {
        let message = quote_status_message("Q-2026-0042", "draft");
        let elements = message.blocks.iter().find_map(|block| {
            if let Block::Actions { elements, .. } = block {
                Some(elements)
            } else {
                None
            }
        });
        assert!(elements.is_some(), "expected actions block");
        let elements = elements.expect("actions block asserted above");

        assert!(matches!(
            elements.first(),
            Some(element) if element.action_id == "quote.refresh.v1"
        ));
        assert!(matches!(
            elements.first(),
            Some(element) if matches!(element.text, TextObject::Plain { ref text } if text == "Refresh status")
        ));
    }

    #[test]
    fn simulation_comparison_template_includes_promote_actions() {
        let message = simulation_comparison_message(&SimulationComparisonView {
            quote_id: "Q-2026-7777".to_string(),
            baseline_total: 50_000.0,
            request_id: "req-sim-1".to_string(),
            variants: vec![
                SimulationVariantView {
                    variant_key: "discounted_10".to_string(),
                    rank_order: 0,
                    total: 45_000.0,
                    total_delta: -5_000.0,
                    approval_required: false,
                    summary: "Lower total with no approval escalation.".to_string(),
                },
                SimulationVariantView {
                    variant_key: "uplift_term_24".to_string(),
                    rank_order: 1,
                    total: 54_000.0,
                    total_delta: 4_000.0,
                    approval_required: true,
                    summary: "Higher total but manager approval required.".to_string(),
                },
            ],
        });

        let promote_buttons: Vec<_> = message
            .blocks
            .iter()
            .filter_map(|block| match block {
                Block::Actions { elements, .. } => elements.first(),
                _ => None,
            })
            .collect();

        assert_eq!(promote_buttons.len(), 2);
        assert_eq!(promote_buttons[0].action_id, "quote.simulate.promote.v1");
        assert_eq!(
            promote_buttons[0].value.as_deref(),
            Some("action=promote;quote=Q-2026-7777;variant=discounted_10")
        );
    }

    #[test]
    fn simulation_comparison_template_keeps_variant_chronology_and_context() {
        let message = simulation_comparison_message(&SimulationComparisonView {
            quote_id: "Q-2026-9001".to_string(),
            baseline_total: 120_000.0,
            request_id: "req-sim-chronology".to_string(),
            variants: vec![
                SimulationVariantView {
                    variant_key: "A/B test".to_string(),
                    rank_order: 0,
                    total: 118_000.0,
                    total_delta: -2_000.0,
                    approval_required: false,
                    summary: "Minor discount".to_string(),
                },
                SimulationVariantView {
                    variant_key: "renewal+uplift".to_string(),
                    rank_order: 1,
                    total: 126_500.0,
                    total_delta: 6_500.0,
                    approval_required: true,
                    summary: "Higher term uplift".to_string(),
                },
            ],
        });

        let mut variant_section_ids = Vec::new();
        let mut variant_action_ids = Vec::new();
        let mut saw_request_context = false;
        let mut saw_hypothetical_notice = false;

        for block in &message.blocks {
            match block {
                Block::Section { block_id, .. }
                    if block_id.starts_with("quote.simulation.variant.") =>
                {
                    variant_section_ids.push(block_id.clone());
                }
                Block::Actions { block_id, elements }
                    if block_id.starts_with("quote.simulation.actions.") =>
                {
                    variant_action_ids.push(block_id.clone());
                    assert!(matches!(
                        elements.first(),
                        Some(button)
                            if button.action_id == "quote.simulate.promote.v1"
                                && button.text == TextObject::Plain {
                                    text: "Promote Variant".to_owned()
                                }
                    ));
                }
                Block::Context { block_id, elements }
                    if block_id == "quote.simulation.context.v1" =>
                {
                    saw_request_context = elements.iter().any(|element| {
                        matches!(
                            element,
                            TextObject::Plain { text } if text == "Request ID: req-sim-chronology"
                        )
                    });
                    saw_hypothetical_notice = elements.iter().any(|element| {
                        matches!(
                            element,
                            TextObject::Plain { text }
                                if text == "Scenario results are hypothetical until a variant is promoted."
                        )
                    });
                }
                _ => {}
            }
        }

        assert_eq!(
            variant_section_ids,
            vec![
                "quote.simulation.variant.0.a_b_test.v1".to_string(),
                "quote.simulation.variant.1.renewal_uplift.v1".to_string(),
            ]
        );
        assert_eq!(
            variant_action_ids,
            vec![
                "quote.simulation.actions.0.a_b_test.v1".to_string(),
                "quote.simulation.actions.1.renewal_uplift.v1".to_string(),
            ]
        );
        assert!(saw_request_context);
        assert!(saw_hypothetical_notice);
    }

    #[test]
    fn simulation_promote_value_builder_is_idempotent() {
        assert_eq!(
            simulation_promotion_action_value("Q-2026-8888", "variant_alpha"),
            "action=promote;quote=Q-2026-8888;variant=variant_alpha"
        );
    }

    #[test]
    fn simulation_promote_value_builder_encodes_delimiters() {
        assert_eq!(
            simulation_promotion_action_value("Q-2026-8888", "variant;x=1=2/need review",),
            "action=promote;quote=Q-2026-8888;variant=variant%3Bx%3D1%3D2%2Fneed%20review"
        );
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
    fn execution_task_progress_buttons_include_idempotent_action_payloads() {
        let message = execution_task_progress_message(
            "Q-2026-020",
            "task-retry",
            "pdf_generation",
            ExecutionTaskStatus::RetryableFailed {
                error: "Network timeout".to_string(),
                retry_count: 2,
                max_retries: 3,
            },
        );

        let actions = message.blocks.iter().find_map(|block| match block {
            Block::Actions { block_id, elements } if block_id == "exec.status.actions.v1" => {
                Some(elements)
            }
            _ => None,
        });
        assert!(actions.is_some(), "expected actions block");
        let actions = actions.expect("actions block asserted above");
        assert!(
            actions.iter().all(|button| {
                let value = button.value.as_deref().unwrap_or_default();
                value.contains("quote=Q-2026-020")
                    && value.contains("task=task-retry")
                    && value.contains("action=")
                    && value.contains("state=retryable_failed_2_of_3")
            }),
            "all execution controls should carry idempotent state payloads"
        );
    }

    #[test]
    fn execution_task_progress_context_includes_quote_and_chronology() {
        let message = execution_task_progress_message(
            "Q-2026-021",
            "task-run",
            "crm_sync",
            ExecutionTaskStatus::Running {
                worker_id: "worker-009".to_string(),
                started_at: "2026-02-24T00:00:00Z".to_string(),
            },
        );

        let context = message.blocks.iter().find_map(|block| match block {
            Block::Context { block_id, elements } if block_id == "exec.status.context.v1" => {
                Some(elements)
            }
            _ => None,
        });
        assert!(context.is_some(), "expected execution context block");
        let context = context.expect("context block asserted above");
        let joined = context
            .iter()
            .map(|item| match item {
                TextObject::Plain { text } | TextObject::Mrkdwn { text } => text.clone(),
            })
            .collect::<Vec<_>>()
            .join(" | ");
        assert!(joined.contains("Quote: Q-2026-021"));
        assert!(joined.contains("Task ID: task-run"));
        assert!(joined.contains("Chronology state: running"));
        assert!(joined.contains("idempotent"));
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
    fn execution_summary_uses_unique_task_context_block_ids() {
        let tasks = vec![
            ("task-1".to_string(), "send_slack_message".to_string(), ExecutionTaskStatus::Queued),
            (
                "task-2".to_string(),
                "crm_sync".to_string(),
                ExecutionTaskStatus::RetryableFailed {
                    error: "timeout".to_string(),
                    retry_count: 1,
                    max_retries: 3,
                },
            ),
        ];

        let message = super::execution_summary_message("Q-2026-099", &tasks);
        let task_block_ids = message
            .blocks
            .iter()
            .filter_map(|block| match block {
                Block::Context { block_id, .. } if block_id.starts_with("exec.summary.task.") => {
                    Some(block_id.clone())
                }
                _ => None,
            })
            .collect::<Vec<_>>();

        assert_eq!(task_block_ids.len(), 2);
        assert_eq!(task_block_ids[0], "exec.summary.task.task_1.v1");
        assert_eq!(task_block_ids[1], "exec.summary.task.task_2.v1");
    }

    #[test]
    fn execution_task_actions_use_readable_labels_for_accessibility() {
        let message = execution_task_progress_message(
            "Q-2026-022",
            "task-readable",
            "pdf_generation",
            ExecutionTaskStatus::RetryableFailed {
                error: "timeout".to_string(),
                retry_count: 1,
                max_retries: 3,
            },
        );

        let actions = message.blocks.iter().find_map(|block| match block {
            Block::Actions { block_id, elements } if block_id == "exec.status.actions.v1" => {
                Some(elements)
            }
            _ => None,
        });
        assert!(actions.is_some(), "expected actions block");
        let actions = actions.expect("actions block asserted above");

        assert!(
            actions.iter().all(|button| match &button.text {
                TextObject::Plain { text } | TextObject::Mrkdwn { text } => {
                    text.chars().any(|ch| ch.is_ascii_alphabetic())
                }
            }),
            "action labels should contain readable words, not icon-only text"
        );
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

    #[test]
    fn policy_packet_action_payloads_are_idempotent_and_reason_aware() {
        let packet = policy_packet_fixture();

        let approve_value =
            policy_approval_packet_action_value(&packet, PolicyApprovalDecisionKind::Approve);
        let approve_value_again =
            policy_approval_packet_action_value(&packet, PolicyApprovalDecisionKind::Approve);
        assert_eq!(approve_value, approve_value_again);
        assert!(approve_value.contains("decision=approve"));
        assert!(approve_value.contains("reason_required=false"));

        let reject_value =
            policy_approval_packet_action_value(&packet, PolicyApprovalDecisionKind::Reject);
        assert!(reject_value.contains("decision=reject"));
        assert!(reject_value.contains("reason_required=true"));

        let request_changes_value = policy_approval_packet_action_value(
            &packet,
            PolicyApprovalDecisionKind::RequestChanges,
        );
        assert!(request_changes_value.contains("decision=request_changes"));
        assert!(request_changes_value.contains("reason_required=true"));
    }

    #[test]
    fn policy_packet_message_renders_required_sections_and_actions() {
        let message = policy_approval_packet_message(&policy_packet_fixture());

        let required_sections = [
            "policy.packet.header.v1",
            "policy.packet.candidate_diff.v1",
            "policy.packet.replay_evidence.v1",
            "policy.packet.risk.v1",
            "policy.packet.fallback.v1",
        ];
        for block_id in required_sections {
            let present = message.blocks.iter().any(
                |block| matches!(block, Block::Section { block_id: id, .. } if id == block_id),
            );
            assert!(present, "missing required section block: {block_id}");
        }

        let actions = message.blocks.iter().find_map(|block| match block {
            Block::Actions { block_id, elements } if block_id == "policy.packet.actions.v1" => {
                Some(elements)
            }
            _ => None,
        });
        assert!(actions.is_some(), "expected policy packet actions block");
        let actions = actions.expect("actions block asserted above");
        assert_eq!(actions.len(), 3);
        assert!(actions.iter().any(|button| button.action_id == "policy.packet.approve.v1"));
        assert!(actions.iter().any(|button| button.action_id == "policy.packet.reject.v1"));
        assert!(actions
            .iter()
            .any(|button| button.action_id == "policy.packet.request_changes.v1"));
    }

    fn policy_packet_fixture() -> PolicyApprovalPacketView {
        PolicyApprovalPacketView {
            packet_id: "pktv1:abc123".to_string(),
            packet_version: "clo_approval_packet.v1".to_string(),
            candidate_id: "cand-101".to_string(),
            proposed_policy_version: 42,
            candidate_diff_summary:
                "2 rule updates: discount-cap threshold 20->18; margin-floor 25->27".to_string(),
            replay_evidence_summary:
                "Cohort size 240; projected margin +55bps; win-rate proxy +20bps.".to_string(),
            risk_score_bps: 1800,
            blast_radius_summary: "18% impacted quotes across smb+enterprise".to_string(),
            fallback_plan_summary:
                "Rollback to v41 with signed apply rollback within 15m if drift exceeds threshold."
                    .to_string(),
        }
    }

    #[test]
    fn suggestion_message_renders_items_with_add_buttons() {
        use super::{SuggestionCardView, SuggestionItemView};

        let view = SuggestionCardView {
            quote_id: Some("Q-2026-0042".to_string()),
            customer_hint: "Acme Corp".to_string(),
            suggestions: vec![
                SuggestionItemView {
                    product_id: "prod_sso".to_string(),
                    product_name: "SSO Add-on".to_string(),
                    product_sku: "ADDON-SSO-001".to_string(),
                    score: 0.85,
                    confidence: "High".to_string(),
                    category_description: "Complements your current selection".to_string(),
                    reasoning: vec!["85% of similar customers added SSO".to_string()],
                    unit_price: Some(2.0),
                },
                SuggestionItemView {
                    product_id: "prod_support".to_string(),
                    product_name: "Premium Support".to_string(),
                    product_sku: "ADDON-SUP-001".to_string(),
                    score: 0.62,
                    confidence: "Medium".to_string(),
                    category_description: "Frequently purchased together".to_string(),
                    reasoning: vec!["Enterprise customers typically add support".to_string()],
                    unit_price: Some(500.0),
                },
            ],
            request_id: "req-suggest-1".to_string(),
        };

        let message = super::suggestion_message(&view);

        // Fallback should mention suggestions
        assert!(message.fallback_text.contains("suggestion"));
        assert!(message.fallback_text.contains("Acme Corp"));

        // Should have header
        let header = message.blocks.iter().find(|block| {
            matches!(block, Block::Section { block_id, .. } if block_id == "suggest.header.v1")
        });
        assert!(header.is_some(), "expected header section");

        // Should have item sections for each suggestion
        let item_0 = message.blocks.iter().find(|block| {
            matches!(block, Block::Section { block_id, .. } if block_id == "suggest.item.0.v1")
        });
        assert!(item_0.is_some(), "expected first suggestion section");

        let item_1 = message.blocks.iter().find(|block| {
            matches!(block, Block::Section { block_id, .. } if block_id == "suggest.item.1.v1")
        });
        assert!(item_1.is_some(), "expected second suggestion section");

        // Should have action buttons for each suggestion
        let actions_0 = message.blocks.iter().find_map(|block| match block {
            Block::Actions { block_id, elements } if block_id == "suggest.item.actions.0.v1" => {
                Some(elements)
            }
            _ => None,
        });
        assert!(actions_0.is_some(), "expected actions for first suggestion");
        let actions = actions_0.expect("actions asserted above");
        assert_eq!(actions.len(), 2);
        assert!(actions[0].action_id.starts_with("suggest.add."));
        assert!(actions[1].action_id.starts_with("suggest.details."));

        // Context block should exist
        let context = message.blocks.iter().find(|block| {
            matches!(block, Block::Context { block_id, .. } if block_id == "suggest.context.v1")
        });
        assert!(context.is_some(), "expected context block");
    }

    #[test]
    fn suggestion_message_empty_shows_empty_state() {
        use super::SuggestionCardView;

        let view = SuggestionCardView {
            quote_id: None,
            customer_hint: "New Customer".to_string(),
            suggestions: vec![],
            request_id: "req-empty-1".to_string(),
        };

        let message = super::suggestion_message(&view);

        let empty = message.blocks.iter().find(|block| {
            matches!(block, Block::Section { block_id, .. } if block_id == "suggest.empty.v1")
        });
        assert!(empty.is_some(), "expected empty state section");
    }

    #[test]
    fn suggestion_action_value_encodes_correctly() {
        let value = super::suggestion_action_value(Some("Q-2026-001"), "prod_sso", "ADDON-SSO-001");
        assert!(value.contains("action=add_suggested"));
        assert!(value.contains("product=prod_sso"));
        assert!(value.contains("sku=ADDON-SSO-001"));
        assert!(value.contains("quote=Q-2026-001"));
    }

    #[test]
    fn suggestion_action_value_without_quote() {
        let value = super::suggestion_action_value(None, "prod_sso", "ADDON-SSO-001");
        assert!(value.contains("action=add_suggested"));
        assert!(value.contains("product=prod_sso"));
        assert!(!value.contains("quote="));
    }

    #[test]
    fn score_bar_renders_proportionally() {
        assert!(super::score_bar(1.0).starts_with("█████"));
        assert!(super::score_bar(0.0).starts_with("░░░░░"));
        assert!(super::score_bar(0.5).contains("50%"));
    }
}
