#!/bin/sh
set -eu

repository_root="$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)"
temporary_directory="$(mktemp -d)"
trap 'rm -rf "$temporary_directory"' EXIT HUP INT TERM

case "$(uname -s)" in
    Linux) platform="linux" ;;
    Darwin) platform="macos" ;;
    *) exit 0 ;;
esac
case "$(uname -m)" in
    x86_64 | amd64) architecture="x86_64" ;;
    arm64 | aarch64) architecture="aarch64" ;;
    *) exit 0 ;;
esac

release_directory="${temporary_directory}/release"
home_directory="${temporary_directory}/home"
install_directory="${home_directory}/.local/bin"
mkdir -p "$release_directory" "$home_directory"

cat > "${release_directory}/forgeguard" <<'EOF'
#!/bin/sh
test "$*" = "init --global --agent all"
: > "${HOME}/global-setup-ran"
EOF
chmod +x "${release_directory}/forgeguard"

archive="forgeguard-${platform}-${architecture}.tar.gz"
tar -C "$release_directory" -czf "${release_directory}/${archive}" forgeguard
if command -v sha256sum >/dev/null 2>&1; then
    sha256sum "${release_directory}/${archive}" | awk '{print $1}' \
        > "${release_directory}/${archive}.sha256"
else
    shasum -a 256 "${release_directory}/${archive}" | awk '{print $1}' \
        > "${release_directory}/${archive}.sha256"
fi

HOME="$home_directory" \
SHELL="/bin/zsh" \
FORGEGUARD_DOWNLOAD_BASE="file://${release_directory}" \
sh "${repository_root}/install.sh" > "${temporary_directory}/output"

test -x "${install_directory}/forgeguard"
test -f "${home_directory}/global-setup-ran"
grep -F 'export PATH="$HOME/.local/bin:$PATH"' "${home_directory}/.zshrc" >/dev/null
grep -F "ForgeGuard installed:" "${temporary_directory}/output" >/dev/null

printf '%064d\n' 0 > "${release_directory}/${archive}.sha256"
if HOME="$home_directory" \
    SHELL="/bin/zsh" \
    FORGEGUARD_DOWNLOAD_BASE="file://${release_directory}" \
    sh "${repository_root}/install.sh" >/dev/null 2>&1; then
    echo "error: installer accepted an invalid checksum" >&2
    exit 1
fi
