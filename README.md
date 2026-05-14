# devtree

`devtree` creates runnable Git worktrees: it creates the checkout, copies explicitly configured local env files, runs setup commands, starts dev servers, and records the local URL and logs.

It is intentionally separate from issue trackers, PR workflows, and SDLC tools. Those systems can call `devtree`, but `devtree` only manages local runnable worktrees.

## Why

Git worktrees are useful for parallel agent and human work, but a fresh worktree is often not runnable. Ignored files such as `.env.local` are missing, dependencies may not be installed, generated clients are stale, and the app is not reachable at a stable URL.

`devtree` makes that setup explicit and repeatable.

## Install

From a local clone:

```sh
cargo install --path .
```

From GitHub, after releases are published:

```sh
cargo install --git https://github.com/HanifCarroll/devtree
```

## Quick Start

In a Git repo:

```sh
devtree init
```

Edit `.devtree.yml`, then:

```sh
devtree create fix-navbar
devtree setup fix-navbar
devtree start fix-navbar web
```

Or run the common path:

```sh
devtree up fix-navbar web
```

Useful follow-up commands:

```sh
devtree status
devtree logs fix-navbar web
devtree stop fix-navbar web
devtree clean fix-navbar
```

## Example Config

```yaml
name: myapp
worktreesRoot: ../myapp-worktrees

env:
  copy: []
  optionalCopy:
    - .env.local

setup:
  - name: install dependencies
    run: pnpm install --frozen-lockfile
    timeoutSeconds: 900

  - name: generate prisma client
    cwd: apps/web
    run: pnpm prisma generate
    ifFileExists: prisma/schema.prisma

apps:
  web:
    cwd: apps/web
    command: pnpm dev
    url:
      provider: portless
      name: myapp
    healthUrl: /api/health
```

With Portless, `devtree start fix-navbar web` starts the app through `portless myapp ...`. In a linked worktree, Portless exposes a branch-scoped URL such as:

```txt
https://fix-navbar.myapp.localhost
```

## Config Reference

See [docs/config.md](docs/config.md).

## Secrets

`devtree` never reads or prints secret values. It only copies env files that are explicitly listed in `.devtree.yml`. See [docs/secrets.md](docs/secrets.md).

## Portless

Portless is the default URL provider because it gives stable `.localhost` URLs and automatically detects linked Git worktrees. See [docs/adapters/portless.md](docs/adapters/portless.md).

## State And Logs

Runtime state is stored outside your repo under the platform state directory for `devtree`. Logs are stored under:

```txt
<state-dir>/logs/<repo>/<branch>/<app>.log
```

Run `devtree doctor` to see the exact local paths.

## Safety

- `devtree clean` refuses to remove a dirty worktree unless `--force` is passed.
- `devtree clean` refuses to remove a worktree while tracked apps are still running.
- Env files are not overwritten after the first copy.
- Only files explicitly listed in `.devtree.yml` are copied.

## License

MIT
