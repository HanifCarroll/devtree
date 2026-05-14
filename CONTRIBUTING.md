# Contributing

Thanks for considering a contribution.

## Local Checks

Run:

```sh
cargo fmt --check
cargo check
cargo clippy -- -D warnings
cargo test
```

## Design Boundary

`devtree` should stay a small local runtime tool. It should not absorb issue tracking, pull requests, deployments, or project-planning workflows.

Good additions:

- better config validation
- safer process management
- URL adapters
- env materialization adapters
- examples for common stacks

Avoid:

- hidden env-file discovery
- printing secrets
- overwriting local env files by default
- requiring a specific issue tracker
- turning setup commands into a full workflow engine

## Documentation

User-visible behavior should be documented in `README.md` or `docs/`. Config changes should update `docs/config.md` and at least one example when useful.
