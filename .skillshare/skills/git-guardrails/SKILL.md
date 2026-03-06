---
name: git-guardrails
description: >-
  Prevent large files and compiled binaries from entering git history.
  Use when setting up a repo, reviewing .gitignore, or after a history cleanup.
targets: [claude, codex]
---

# Git Guardrails

Prevent repo bloat by blocking large files and binaries at commit time.

## What it does

1. **Large-file guard**: Blocks staged files over 2 MB (configurable via `GIT_GUARDRAILS_MAX_BYTES`)
2. **Binary extension guard**: Blocks compiled artifacts (.so, .dylib, .dll, .exe, .a, .rlib, .rmeta, .o, .dSYM)
3. **Integrates with existing hooks**: Chains with cargo fmt, clippy, and UBS if present

## Install to a repo

```bash
# Copy the hook into the repo's tracked hooks directory
mkdir -p .githooks
cp "$SKILLSHARE_SOURCE/pre-commit" .githooks/pre-commit
chmod +x .githooks/pre-commit

# Point git at the tracked hooks
git config core.hooksPath .githooks
```

Or just activate hooks if `.githooks/pre-commit` already exists:

```bash
git config core.hooksPath .githooks
```

## Verify

```bash
# Should block:
dd if=/dev/zero of=test-big bs=1M count=3
git add test-big    # hook will reject on commit
git reset HEAD test-big && rm test-big

# Should also block:
touch fake.rlib
git add fake.rlib   # hook will reject on commit
git reset HEAD fake.rlib && rm fake.rlib
```

## .gitignore patterns to pair with this

Add these to `.gitignore` as a first line of defense:

```gitignore
# Build artifacts
target/
.tmp-target/

# SQLite databases
*.db
*.db-shm
*.db-wal
*.sqlite
*.sqlite3

# Compiled binaries
*.so
*.dylib
*.dll
*.exe
*.rlib
*.rmeta
```

## Customize

- `GIT_GUARDRAILS_MAX_BYTES=5242880` — change the size limit (default 2 MB)
- `QUOTEY_PRE_COMMIT_CLIPPY=0` — skip clippy in the hook
- `git commit --no-verify` — bypass all hooks (emergency only)
