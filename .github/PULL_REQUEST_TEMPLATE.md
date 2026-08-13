## Summary

<!-- What problem does this change solve? Keep this focused. -->

## Changes

<!-- List the important changes by area. -->

-

## Test plan

<!-- Check only verification that was actually run. -->

- [ ] `cargo fmt --all -- --check`
- [ ] `cargo clippy --workspace --all-targets --all-features -- -D warnings`
- [ ] `cargo test --locked --workspace`
- [ ] `forgeguard gate --changed --output compact`
- [ ] Other: <!-- describe -->

## Checklist

- [ ] The change is limited to the stated problem.
- [ ] Tests or verification cover behavioral changes.
- [ ] Documentation is updated where user-facing behavior changed.
- [ ] No secrets, tokens, or personal information are included.
- [ ] Commit messages use Conventional Commits.
