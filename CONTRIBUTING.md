# Contributing to ForgeGuard

You do not need to understand the whole scanner to make a useful first contribution. Start with one user problem, keep the diff focused, and leave exact verification behind.

## Choose a first contribution

Look for [`good first issue`](https://github.com/suiflex/ForgeGuard/labels/good%20first%20issue) and [`help wanted`](https://github.com/suiflex/ForgeGuard/labels/help%20wanted). If none matches your experience, open a [contribution proposal](https://github.com/suiflex/ForgeGuard/issues/new?template=contribution_proposal.yml) before investing in a large change.

| Lane | Good first outcome | Start here |
|---|---|---|
| Documentation | Clarify one workflow, agent limitation, error, or real example | `README.md`, `docs/`, `tests/install_test.sh` |
| Detection preset | Add or correct one ecosystem command without replacing user config | `crates/forgeguard-core/src/detector.rs`, detector tests |
| Rule fixture | Add a missed case or false-positive regression for an existing rule | `crates/forgeguard-core/src/rules.rs`, rule tests |
| Agent integration | Improve one documented policy, skill, path, or lifecycle adapter | `crates/forgeguard-core/src/init.rs`, `crates/forgeguard-core/assets/`, integration tests |
| General Guard | Improve a role review phase, evidence contract, or non-file resource workflow | `crates/forgeguard-core/src/hook.rs`, `docs/FOCUS.md`, focused tests |

Documentation-only fixes are valid contributions. A small false-positive fixture can be more valuable than a broad new rule.

## Before changing files

1. Search existing issues, discussions, rules, and tests for the same problem.
2. For behavior changes, agree on a bounded issue or contribution proposal. Small typo and link fixes can go straight to a pull request.
3. Create a focused branch from `main`.
4. Install the Rust toolchain and Git. Additional tools are needed only when the area under test uses them.

Do **not** run `forgeguard init` in the ForgeGuard checkout. It would install generated agent files into the product's own repository. Use the throwaway sandbox instead:

```bash
sh tests/sandbox.sh
```

## Implement one verifiable change

- Reuse the repository's existing rule, detector, output, and test patterns.
- Keep evidence specific and recommendations actionable.
- Preserve existing user configuration during initialization and upgrades.
- Add the smallest test that fails before the behavior change and passes after it.
- Update user-facing documentation when commands, output, configuration, integrations, or limitations change.

For a new or stricter rule, state which class it belongs to:

1. **Deterministic or semantic:** exact local evidence can block.
2. **Structural or heuristic:** uncertain evidence should guide review, not overclaim certainty.
3. **Evidence-based:** the conclusion requires a benchmark, query plan, profiler, evaluation, trace, or comparable artifact.

Do not add a hard-blocking rule based only on broad regular expressions. Rule tests must cover both a real detection and realistic false-positive resistance. A suppression marker applies only to the two lines after it:

```text
// forgeguard: allow FG-XXX -- concrete reason
```

## Run the checks

Run the checks that cover your change while iterating. Before requesting final review, run the repository sequence:

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --locked --workspace
cargo build --locked --workspace --release
sh tests/install_test.sh
sh tests/wizard_test.sh
```

Also run the changed-code gate when the binary is available:

```bash
target/release/forgeguard gate --changed --output compact
```

Unix-only tests must use `#[cfg(unix)]` so the Windows release build still compiles. Terminal output uses the stdlib-only `theme` module in `crates/forgeguard-cli/src/main.rs`; new output must remain plain when stdout is not a terminal or `NO_COLOR` is set.

## Open the pull request

The pull request should answer four questions:

1. What user problem does this solve?
2. What changed, and what deliberately did not change?
3. What exact command or artifact proves the result?
4. Which issue does it close, or why was no issue needed?

Check only commands you actually ran. Maintainers may ask for a smaller scope, a false-positive fixture, platform coverage, or clearer evidence before merging. Once merged, GitHub records the contribution in the repository's contributor history.

For questions, design exploration, and integrations that are not ready for an issue, use [GitHub Discussions](https://github.com/suiflex/ForgeGuard/discussions). Report vulnerabilities privately through the repository's [security advisory form](https://github.com/suiflex/ForgeGuard/security/advisories/new).
