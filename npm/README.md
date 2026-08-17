# @suiflex/forgeguard

Token-efficient quality gates for AI coding agents.

This package installs the ForgeGuard native CLI and exposes it as the `forgeguard` command. The matching binary is downloaded from the corresponding GitHub Release during installation and verified with its SHA-256 checksum.

## Requirements

- Node.js 18 or newer
- An operating system and CPU supported by the ForgeGuard release assets:
  - macOS: x64 and arm64
  - Linux: x64 and arm64
  - Windows: x64 and arm64

## Install

```bash
npm install -g @suiflex/forgeguard
```

Verify the installation:

```bash
forgeguard --version
```

Initialize ForgeGuard in a repository:

```bash
cd your-project
forgeguard init
forgeguard doctor
```

## Usage

Run the quality gate in the current repository:

```bash
forgeguard gate --changed --output compact
```

See all commands and options:

```bash
forgeguard --help
```

## Security

The installer downloads only the release archive for the package version from GitHub Releases. It downloads the accompanying `.sha256` file and refuses to install an archive when the checksum does not match.

Review the repository's [security policy](https://github.com/suiflex/ForgeGuard/blob/main/SECURITY.md) before using ForgeGuard in an untrusted repository. ForgeGuard can execute commands configured by `.forgeguard/config.toml`.

## Links

- [Repository](https://github.com/suiflex/ForgeGuard)
- [Full documentation](https://github.com/suiflex/ForgeGuard#readme)
- [Issue tracker](https://github.com/suiflex/ForgeGuard/issues)
- [Security policy](https://github.com/suiflex/ForgeGuard/blob/main/SECURITY.md)

## License

MIT
