# Security Policy

## Supported versions

ForgeGuard is pre-1.0. Security fixes are applied to the latest release line.

## Reporting a vulnerability

Do not open a public issue for an exploitable vulnerability. Use GitHub's private vulnerability reporting for this repository.

## Command execution warning

`forgeguard gate` executes commands from `.forgeguard/config.toml` through the platform shell. Treat configuration from an untrusted repository as executable code and review it before running ForgeGuard.

ForgeGuard also installs lifecycle hooks. Keep host hook trust disabled until repository ownership, generated hook JSON, and `.forgeguard/config.toml` are verified.

ForgeGuard does not upload source code, prompts, or scan results by default.

Official installers download only HTTPS GitHub Release assets and reject binaries whose SHA-256 checksum does not match the release sidecar. Review remote scripts before piping them into a shell when your security policy requires it.
