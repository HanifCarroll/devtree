# Config Reference

`devtree` reads `.devtree.yml` from the Git repo root.

## Top-Level Fields

```yaml
name: myapp
worktreesRoot: ../myapp-worktrees
env: {}
setup: []
apps: {}
```

### `name`

The project name. This is used for default Portless route names and log paths.

### `worktreesRoot`

Where linked worktrees should be created. Relative paths are resolved from the repo root.

```yaml
worktreesRoot: ../myapp-worktrees
```

## Env Files

```yaml
env:
  copy:
    - apps/web/.env.local
  optionalCopy:
    - apps/mobile/.env.local
```

- `copy`: required local files. `devtree create` fails if one is missing.
- `optionalCopy`: copied when present, skipped when absent.

Existing destination files are left unchanged.

## Setup Commands

```yaml
setup:
  - name: install dependencies
    cwd: .
    run: pnpm install --frozen-lockfile
    timeoutSeconds: 900
```

Fields:

- `name`: required label shown in output.
- `run`: required shell command.
- `cwd`: relative path inside the worktree. Defaults to `.`.
- `env`: extra environment variables for the command.
- `ifFileExists`: skip unless this path exists in the worktree.
- `ifFileMissing`: skip unless this path is missing in the worktree.
- `timeoutSeconds`: kill the command if it runs too long.

If setup becomes complex, keep `devtree` simple and call a repo-owned script:

```yaml
setup:
  - name: repo bootstrap
    run: ./scripts/devtree-bootstrap.sh
```

## Apps

```yaml
apps:
  web:
    cwd: apps/web
    command: pnpm dev
    url:
      provider: portless
      name: myapp
    healthUrl: /api/health
    healthTimeoutSeconds: 45
```

Fields:

- `cwd`: relative path inside the worktree. Defaults to `.`.
- `command`: command to start the app.
- `env`: extra environment variables for the process.
- `url`: URL provider config.
- `healthUrl`: optional path or absolute URL checked after start.
- `healthTimeoutSeconds`: health wait timeout. Defaults to `45`.

If no app is passed to `devtree start`, `devtree` uses `default` if present, otherwise the first configured app.

## URL Providers

### Portless

```yaml
url:
  provider: portless
  name: myapp
```

Runs:

```sh
portless myapp <command>
```

### Raw Port

```yaml
url:
  provider: raw-port
  url: http://127.0.0.1:3000
```

Runs the command without Portless and records the provided URL.

### None

```yaml
url:
  provider: none
```

Runs the command and records `none` as the URL.
