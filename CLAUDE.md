# ForgeGuard

For code changes, use `/forgeguard-engineering`.

## Working in this repository

- Do not run `forgeguard init` against the repository root. ForgeGuard installs
  into its own checkout like any other project, writing `.claude/settings.json`,
  `.claude/skills/`, and `.forgeguard/`. Use `sh tests/sandbox.sh` to build
  throwaway repositories under the gitignored `sandbox/` instead.
- `AGENTS.md` is a symlink to `CLAUDE.md`; edit `CLAUDE.md`.

## Checks

`.github/workflows/ci.yml` runs, in order:

```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --locked --workspace
cargo build --locked --workspace --release
sh tests/install_test.sh
sh tests/wizard_test.sh
```

`tests/wizard_test.sh` drives the interactive `init` wizard through a
pseudo-terminal, so it needs a built binary and takes the first of
`target/debug` or `target/release`. `sh tests/logo.sh --check` verifies the
banner grid still matches `assets/brand/logo-mark.svg`; it is not part of CI
because it needs macOS `qlmanage` and Pillow.

## Things that have bitten before

- The Windows release job only builds, so test code that fails to compile there
  is never caught by CI. Gate unix-only tests with `#[cfg(unix)]`.
- A `// forgeguard: allow FG-XXX -- reason` marker suppresses the rule for only
  the two lines that follow it, so keep the marker on the line above the finding.
- Terminal output uses the stdlib-only `theme` module in
  `crates/forgeguard-cli/src/main.rs` — no color crates. Anything it prints must
  collapse to plain text when stdout is not a terminal or `NO_COLOR` is set.
