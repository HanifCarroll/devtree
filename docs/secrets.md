# Secrets

`devtree` treats secrets as local files owned by the repo user. It does not parse, print, redact, sync, or manage secret values.

## Rules

- Env files are copied only when explicitly listed in `.devtree.yml`.
- Env file contents are never printed.
- Existing env files in a worktree are not overwritten.
- Missing files listed under `env.copy` fail the create step.
- Missing files listed under `env.optionalCopy` are skipped.

## Recommended Pattern

Commit examples and templates:

```txt
.env.example
apps/web/.env.example
```

Ignore real local env files:

```gitignore
.env
.env.local
apps/**/.env.local
```

Then configure `devtree`:

```yaml
env:
  copy:
    - apps/web/.env.local
```

## Future Strategies

The initial version supports explicit copy only. Good future additions:

- template creation from `.env.example`
- 1Password or `op inject`
- `direnv`
- custom command-based materialization

Those should be adapters, not the default.
