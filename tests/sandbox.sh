#!/bin/sh
# Build throwaway repositories under sandbox/ for trying `forgeguard init` by
# hand. Not a CI test: nothing here asserts, it just sets up scenarios and runs
# the non-interactive ones so you can read the output and poke at the results.
#
#   sh tests/sandbox.sh            build scenarios and show what each one does
#   sh tests/sandbox.sh --clean    delete sandbox/ and stop
#
# sandbox/ is gitignored, and `forgeguard gate` skips gitignored paths, so
# nothing in here affects the repository.
set -eu

repository_root="$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)"
sandbox="${repository_root}/sandbox"
binary="${FORGEGUARD_BINARY:-${repository_root}/target/debug/forgeguard}"

if [ "${1:-}" = "--clean" ]; then
    rm -rf "$sandbox"
    echo "removed $sandbox"
    exit 0
fi

test -x "$binary" || {
    echo "error: no binary at $binary — run 'cargo build' first" >&2
    exit 1
}

rm -rf "$sandbox"
mkdir -p "$sandbox"

# Each scenario is a git repository seeded with the marker directories of the
# agents it is meant to look like, so detection has something real to find.
scenario() {
    name="$1"
    shift
    directory="${sandbox}/${name}"
    mkdir -p "$directory"
    git -C "$directory" init -q
    for marker in "$@"; do
        case "$marker" in
            */) mkdir -p "${directory}/${marker}" ;;
            *) mkdir -p "$(dirname "${directory}/${marker}")"; : > "${directory}/${marker}" ;;
        esac
    done
    printf '%s' "$directory"
}

count_files() {
    find "$1" -not -path '*/.git/*' -type f | wc -l | tr -d ' '
}

rule() {
    printf '\n== %s ==\n' "$1"
}

empty="$(scenario empty)"
rule "empty — no agent configured"
echo "\$ forgeguard init"
set +e
(cd "$empty" && "$binary" init < /dev/null)
echo "exit=$? files=$(count_files "$empty")"
set -e

claude_only="$(scenario claude .claude/)"
rule "claude — one agent configured"
echo "\$ forgeguard init"
(cd "$claude_only" && "$binary" init < /dev/null) | grep -v '^Optional:'
echo "files=$(count_files "$claude_only")"

mixed="$(scenario mixed .claude/ .cursor/ .clinerules)"
rule "mixed — three agents configured"
echo "\$ forgeguard init"
(cd "$mixed" && "$binary" init < /dev/null) | grep -v '^Optional:'
echo "files=$(count_files "$mixed")"

everything="$(scenario everything)"
rule "everything — the old behaviour, for comparison"
echo "\$ forgeguard init --agent all"
(cd "$everything" && "$binary" init --agent all > /dev/null 2>&1)
echo "files=$(count_files "$everything")"

lean="$(scenario lean)"
rule "lean — an AGENTS.md-only agent"
echo "\$ forgeguard init --agent copilot"
(cd "$lean" && "$binary" init --agent copilot < /dev/null) | grep -v '^Optional:'

cat <<EOF

Scenarios live in sandbox/ and are safe to edit or delete.

Try the interactive picker, which needs a real terminal:

  cd sandbox/mixed && ${binary} init

Re-run init on a scenario to check the selection stays put:

  cd sandbox/claude && ${binary} init

Inspect what landed:

  find sandbox/claude -not -path '*/.git/*' -type f | sort

Start over:

  sh tests/sandbox.sh          rebuild every scenario
  sh tests/sandbox.sh --clean  delete sandbox/ entirely
EOF
