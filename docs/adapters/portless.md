# Portless Adapter

Portless gives each worktree a stable local URL without manually choosing ports.

`devtree` runs Portless like this:

```sh
portless <name> <command>
```

For example:

```yaml
apps:
  web:
    command: pnpm dev
    url:
      provider: portless
      name: palabruno
```

Running:

```sh
devtree start issue-123 web
```

starts:

```sh
portless palabruno pnpm dev
```

Inside a linked Git worktree, Portless prefixes the branch name into the route, so the URL is:

```txt
https://issue-123.palabruno.localhost
```

## Requirements

Install Portless globally or make it available on `PATH`:

```sh
npm install -g portless
portless service install
portless service status
```

## Troubleshooting

Check active routes:

```sh
portless list
```

If HTTPS startup needs an administrator prompt, install the service from a normal terminal once:

```sh
portless service install
```
