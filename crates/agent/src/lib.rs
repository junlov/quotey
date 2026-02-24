//! Agent Runtime - LLM-powered intent extraction and orchestration
//!
//! This crate provides the "brain" of the quotey system - the agent runtime that:
//! - Extracts structured intent from natural language (Slack messages)
//! - Manages conversation context across multiple messages
//! - Enforces guardrails and safety policies
//! - Orchestrates tool execution (CPQ operations, Slack actions)
//!
//! # Architecture
//!
//! The agent follows a constrained loop:
//! 1. **Intent Extraction** (`conversation`) - Parse NL → structured `QuoteIntent`
//! 2. **Guardrail Enforcement** (`guardrails`) - Validate actions against policies
//! 3. **Tool Execution** (`tools`) - Call CPQ/Slack/CRM adapters
//! 4. **Response Generation** - Format results for Slack
//!
//! # Key Types
//!
//! - `AgentRuntime` - Main orchestrator (see `runtime` module)
//! - `LlmProvider` - Pluggable trait for OpenAI/Anthropic/Ollama
//! - `GuardrailPolicy` - Safety constraints and permission checks
//!
//! # Safety Principle
//!
//! The LLM is strictly a translator. It NEVER decides prices, configurations,
//! or policy outcomes. Those are deterministic decisions made by the CPQ core.

pub mod conversation;
pub mod guardrails;
pub mod llm;
pub mod runtime;
pub mod tools;
