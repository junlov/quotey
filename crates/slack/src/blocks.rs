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

#[cfg(test)]
mod tests {
    use super::{
        approval_request_message, error_message, quote_status_message, Block, ButtonStyle,
        MessageBuilder, TextObject,
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
}
