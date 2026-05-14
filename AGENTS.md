# AGENTS.md

## Project

`devtree` is a standalone Rust CLI for creating runnable Git worktrees. Keep it separate from SDLC, issue tracking, PR automation, and release workflows.

## Scope

The tool owns:

- worktree creation and cleanup
- explicit env-file copy
- setup command execution
- app process start/stop/log/status
- URL recording through adapters such as Portless

The tool does not own:

- GitHub issue lifecycle
- PR creation
- deployment
- project planning docs
- agent orchestration

## Development

Run:

```sh
cargo fmt --check
cargo check
cargo test
```

Before committing, run the `simplify` skill or do the equivalent local pass across the diff for reuse, quality, and efficiency.

## Design Rules

- Keep config explicit.
- Do not read or print secret values.
- Do not overwrite copied env files by default.
- Prefer small local behavior over a general workflow engine.
- Add adapters only when they keep the core simple.
