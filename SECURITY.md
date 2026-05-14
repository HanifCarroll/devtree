# Security

`devtree` is a local development tool that may copy env files and run configured commands. Treat `.devtree.yml` as executable local configuration.

## Reporting

Please report security issues privately to the maintainer instead of opening a public issue.

## Secret Handling

`devtree` should not print, parse, upload, or persist secret values. It only copies files explicitly listed in `.devtree.yml`.

If you add a new env or secret strategy, preserve these rules:

- do not print secret values
- do not copy files unless explicitly configured
- do not overwrite destination env files by default
- keep generated or local state out of Git
