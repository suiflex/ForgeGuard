#!/bin/sh
# Drive the interactive `forgeguard init` wizard through a pseudo-terminal.
#
# The wizard only runs when stdout is a TTY, so a plain pipe cannot reach it.
# `script` allocates a PTY, and keystrokes are paced with sleeps because the
# prompts read raw key events as they render rather than buffering a script.
set -eu

repository_root="$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)"
# Prefer a debug build, which is what a developer has to hand, but fall back to
# the release one: CI only ever runs `cargo build --release`, and `cargo test`
# does not leave a plain binary behind.
binary="${FORGEGUARD_BINARY:-}"
if [ -z "$binary" ]; then
    for candidate in debug release; do
        if [ -x "${repository_root}/target/${candidate}/forgeguard" ]; then
            binary="${repository_root}/target/${candidate}/forgeguard"
            break
        fi
    done
fi
temporary_directory="$(mktemp -d)"
trap 'rm -rf "$temporary_directory"' EXIT HUP INT TERM

if ! command -v script >/dev/null 2>&1; then
    echo "skip: script(1) is unavailable, cannot allocate a PTY" >&2
    exit 0
fi
test -n "$binary" && test -x "$binary" || {
    echo "error: no forgeguard binary under ${repository_root}/target; run 'cargo build' first" >&2
    exit 1
}

# `script` differs between BSD and GNU: BSD takes the command as trailing
# arguments, util-linux needs -c with the command as one string.
#
# Detection reads --help rather than probing with a real run: a probe that
# guesses wrong lands in an interactive shell and hangs CI instead of failing.
# BSD script rejects long options, so its usage text never mentions --command.
if script --help 2>&1 | grep -q -- '--command'; then
    script_flavour="util-linux"
else
    script_flavour="bsd"
fi

# Belt and braces: if the flavour is ever misdetected, fail after a minute
# rather than hanging the job. `timeout` is absent on a stock macOS.
if command -v timeout >/dev/null 2>&1; then
    bounded() { timeout 60 "$@"; }
else
    bounded() { "$@"; }
fi

run_on_pty() {
    if [ "$script_flavour" = "util-linux" ]; then
        command_string="$1"
        shift
        for argument in "$@"; do
            command_string="$command_string '$argument'"
        done
        bounded script -q -e -c "$command_string" /dev/null
    else
        bounded script -q /dev/null "$@"
    fi
}
strip_ansi() {
    sed 's/\x1b\[[0-9;?]*[a-zA-Z]//g'
}

project="${temporary_directory}/project"
mkdir -p "$project/.claude" "$project/.cursor"
git -C "$project" init -q

# Enter x4: accept "This repository", accept the pre-selected agents, accept the
# gitignore prompt, accept the mode prompt.
transcript="${temporary_directory}/transcript"
{
    sleep 0.6; printf '\r'
    sleep 0.6; printf '\r'
    sleep 0.6; printf '\r'
    sleep 0.6; printf '\r'
    sleep 1.2
} | run_on_pty "$binary" --root "$project" init > "$transcript" 2>&1 || true
strip_ansi < "$transcript" > "${transcript}.plain"

assert_contains() {
    if ! grep -qF "$2" "${transcript}.plain"; then
        echo "error: expected to find: $2" >&2
        cat "${transcript}.plain" >&2
        exit 1
    fi
}

# The wizard ran at all, and the banner drew the brand mark: half-blocks in the
# shield colour, not just the wordmark next to them.
assert_contains "wordmark" "ForgeGuard"
for expected in '▀' '38;5;114'; do
    if ! grep -qF "$expected" "$transcript"; then
        echo "error: banner did not render the logo mark: $expected" >&2
        exit 1
    fi
done
# Detection reported both agents before the picker.
assert_contains "detection step" "claude, cursor"
# Rows carry what each target installs, not just a bare name.
assert_contains "menu summary" "CLAUDE.md, own skill, Stop hook"
assert_contains "AGENTS.md-only row" "AGENTS.md only"
# Detected agents arrive pre-checked, everything else unchecked.
assert_contains "claude preselected" "◼ claude"
assert_contains "cursor preselected" "◼ cursor"
assert_contains "codex not preselected" "◻ codex"
# The write summary is grouped rather than one line per file.
assert_contains "result panel" "installed"
assert_contains "agents line" "agents   claude, cursor"
# Asserted as a prefix: the point is that the tree collapsed to one line, not
# how many skill assets ForgeGuard happens to ship this release.
assert_contains "grouped writes" ".claude/ ("

# Accepting the pre-selection must install exactly those two agents.
test -f "$project/CLAUDE.md" || { echo "error: CLAUDE.md missing" >&2; exit 1; }
test -f "$project/.cursor/rules/forgeguard.mdc" || { echo "error: cursor rules missing" >&2; exit 1; }
test ! -f "$project/AGENTS.md" || { echo "error: AGENTS.md written for an unselected agent" >&2; exit 1; }

# A drifted ForgeGuard-owned file must be offered, not silently replaced.
drift="${temporary_directory}/drift"
mkdir -p "$drift/.claude"
git -C "$drift" init -q
"$binary" --root "$drift" init --agent claude >/dev/null 2>&1
printf 'hand written\n' > "$drift/CLAUDE.md"
{
    sleep 0.6; printf '\r'
    sleep 0.6; printf '\r'
    sleep 1.0
} | run_on_pty "$binary" --root "$drift" init --agent claude > "${temporary_directory}/drift-out" 2>&1 || true
strip_ansi < "${temporary_directory}/drift-out" > "${temporary_directory}/drift-plain"

for expected in "ForgeGuard file" "CLAUDE.md" "Replace them with the bundled versions?"; do
    if ! grep -qF "$expected" "${temporary_directory}/drift-plain"; then
        echo "error: refresh prompt never showed: $expected" >&2
        cat "${temporary_directory}/drift-plain" >&2
        exit 1
    fi
done
# Enter accepts the default, which is to keep the file.
if [ "$(cat "$drift/CLAUDE.md")" != "hand written" ]; then
    echo "error: declining the prompt still replaced the file" >&2
    exit 1
fi

# An empty pick must install nothing. Left arrow clears the selection, and two
# empty confirmations cancel.
empty="${temporary_directory}/empty"
mkdir -p "$empty/.claude"
git -C "$empty" init -q
{
    sleep 0.6; printf '\r'
    sleep 0.6; printf '\033[D'
    sleep 0.3; printf '\r'
    sleep 0.6; printf '\033[D'
    sleep 0.3; printf '\r'
    sleep 1.0
} | run_on_pty "$binary" --root "$empty" init > "${temporary_directory}/empty-out" 2>&1 || true

strip_ansi < "${temporary_directory}/empty-out" > "${temporary_directory}/empty-plain"
if [ -f "$empty/CLAUDE.md" ]; then
    echo "error: an empty selection installed files" >&2
    cat "${temporary_directory}/empty-plain" >&2
    exit 1
fi
# Guard against passing for the wrong reason: the run must have reached the
# picker, been re-asked, and then cancelled.
for expected in "nothing selected" "pick at least one" "no agent selected"; do
    if ! grep -qF "$expected" "${temporary_directory}/empty-plain"; then
        echo "error: empty-pick run never reached: $expected" >&2
        cat "${temporary_directory}/empty-plain" >&2
        exit 1
    fi
done

echo "wizard test passed"
