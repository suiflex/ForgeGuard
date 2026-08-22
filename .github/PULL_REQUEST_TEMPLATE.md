## Summary

<!-- What problem does this change solve? Keep this focused. -->

Closes #<!-- issue number, or explain why no issue was needed -->

## Changes

<!-- List the important changes by area. -->

-

## User-visible result

<!-- Show concise before/after output, a sanitized artifact, or the behavior a reviewer can reproduce. -->

## Test plan

<!-- Check only verification that was actually run. -->

- [ ] `cargo fmt --all -- --check`
- [ ] `cargo clippy --workspace --all-targets --all-features -- -D warnings`
- [ ] `cargo test --locked --workspace`
- [ ] `cargo build --locked --workspace --release`
- [ ] `sh tests/install_test.sh`
- [ ] `sh tests/wizard_test.sh`
- [ ] `forgeguard gate --changed --output compact`
- [ ] Other: <!-- describe -->

## Checklist

- [ ] The change is limited to the stated problem.
- [ ] The pull request explains what deliberately remains out of scope.
- [ ] Tests or verification cover behavioral changes.
- [ ] Documentation is updated where user-facing behavior changed.
- [ ] No secrets, tokens, or personal information are included.
- [ ] Commit messages use Conventional Commits.
