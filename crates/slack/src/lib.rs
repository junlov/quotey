//! Slack Integration - Socket Mode bot interface
//!
//! This crate provides the Slack interface for quotey:
//! - **Socket Mode** (`socket`) - WebSocket connection to Slack (no public URL needed)
//! - **Slash Commands** (`commands`) - `/quote new`, `/quote status`, etc.
//! - **Events** (`events`) - Thread messages, emoji reactions, interactions
//! - **Block Kit** (`blocks`) - Rich message builders (buttons, modals, cards)
//!
//! # Getting Started
//!
//! 1. Create a Slack app at https://api.slack.com/apps
//! 2. Enable Socket Mode and subscribe to events
//! 3. Add slash commands: `/quote new`, `/quote status`, `/quote list`
//! 4. Set env vars: `QUOTEY_SLACK_APP_TOKEN`, `QUOTEY_SLACK_BOT_TOKEN`
//!
//! # Architecture
//!
//! ```text
//! Slack Events → EventDispatcher → Handlers → Agent Runtime → CPQ Core
//!                    ↓
//!              Block Kit UI ← Response
//! ```
//!
//! # Key Types
//!
//! - `SocketModeRunner` - WebSocket event loop with reconnection logic
//! - `EventDispatcher` - Routes events to appropriate handlers
//! - `MessageBuilder` - Constructs rich Slack messages
//! - `QuoteCommandService` - Trait for command handlers

pub mod blocks;
pub mod commands;
pub mod events;
pub mod socket;
