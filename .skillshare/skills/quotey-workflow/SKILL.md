---
name: quotey-workflow
description: >-
  Follow Quotey repository rules for coding, issue tracking, and deterministic
  CPQ safety. Use when implementing features or fixes in this repo.
targets: [claude, codex]
---

# Quotey Workflow

## Purpose

Keep changes aligned with Quotey execution rules and deterministic CPQ constraints.

## Instructions

1. Read `AGENTS.md`, `.planning/config.json`, and `.planning/PROJECT.md` before non-trivial changes.
2. Track work in `br` issues instead of ad-hoc TODO lists.
3. Keep CPQ decisions deterministic; LLM outputs are translator/summarizer input only.
4. Update docs when behavior changes.
5. Run relevant validation before finishing changes.

## Safety

- Never delete files without explicit user approval in the current session.
- Never run destructive git/fs commands without explicit user approval.
