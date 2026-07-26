#!/bin/sh
set -eu

repository="suiflex/ForgeGuard"
version="${FORGEGUARD_VERSION:-latest}"

case "$version" in
    *[!A-Za-z0-9._-]*)
        echo "error: invalid FORGEGUARD_VERSION: $version" >&2
        exit 1
        ;;
esac

case "$(uname -s)" in
    Linux) platform="linux" ;;
    Darwin) platform="macos" ;;
    *)
        echo "error: unsupported operating system; use install.ps1 on Windows" >&2
        exit 1
        ;;
esac

case "$(uname -m)" in
    x86_64 | amd64) architecture="x86_64" ;;
    arm64 | aarch64) architecture="aarch64" ;;
    *)
        echo "error: unsupported architecture: $(uname -m)" >&2
        exit 1
        ;;
esac

archive="forgeguard-${platform}-${architecture}.tar.gz"
if [ -n "${FORGEGUARD_DOWNLOAD_BASE:-}" ]; then
    download_base="${FORGEGUARD_DOWNLOAD_BASE%/}"
elif [ "$version" = "latest" ]; then
    download_base="https://github.com/${repository}/releases/latest/download"
else
    download_base="https://github.com/${repository}/releases/download/${version}"
fi

temporary_directory="$(mktemp -d)"
trap 'rm -rf "$temporary_directory"' EXIT HUP INT TERM

download() {
    source_url="$1"
    destination="$2"
    case "$source_url" in
        file://*)
            cp "${source_url#file://}" "$destination"
            return
            ;;
    esac
    if command -v curl >/dev/null 2>&1; then
        curl -fsSL --retry 3 --proto '=https' --tlsv1.2 "$source_url" -o "$destination"
    elif command -v wget >/dev/null 2>&1; then
        wget -q --https-only "$source_url" -O "$destination"
    else
        echo "error: install curl or wget first" >&2
        exit 1
    fi
}

download "${download_base}/${archive}" "${temporary_directory}/${archive}"
download "${download_base}/${archive}.sha256" "${temporary_directory}/${archive}.sha256"

expected_checksum="$(tr -d '[:space:]' < "${temporary_directory}/${archive}.sha256")"
if command -v sha256sum >/dev/null 2>&1; then
    actual_checksum="$(sha256sum "${temporary_directory}/${archive}" | awk '{print $1}')"
elif command -v shasum >/dev/null 2>&1; then
    actual_checksum="$(shasum -a 256 "${temporary_directory}/${archive}" | awk '{print $1}')"
else
    echo "error: SHA-256 tool not found" >&2
    exit 1
fi
if [ "$expected_checksum" != "$actual_checksum" ]; then
    echo "error: checksum verification failed" >&2
    exit 1
fi

tar -xzf "${temporary_directory}/${archive}" -C "$temporary_directory"

: "${HOME:?HOME is not set}"
default_install_directory="${HOME}/.local/bin"
install_directory="${FORGEGUARD_INSTALL_DIR:-$default_install_directory}"
mkdir -p "$install_directory"
install -m 0755 "${temporary_directory}/forgeguard" "${install_directory}/forgeguard"
PATH="${install_directory}:${PATH}"
export PATH

case ":${PATH}:" in
    *":${install_directory}:"*) ;;
    *)
        echo "error: failed to add ForgeGuard to this process PATH" >&2
        exit 1
        ;;
esac

if [ "$install_directory" = "$default_install_directory" ]; then
    path_line='export PATH="$HOME/.local/bin:$PATH"'
    profile="${FORGEGUARD_PROFILE:-}"
    if [ -z "$profile" ]; then
        case "${SHELL:-}" in
            */zsh) profile="${HOME}/.zshrc" ;;
            */bash) profile="${HOME}/.bashrc" ;;
            *) profile="${HOME}/.profile" ;;
        esac
    fi
    if ! grep -F "$path_line" "$profile" >/dev/null 2>&1; then
        printf '\n# ForgeGuard\n%s\n' "$path_line" >> "$profile"
    fi
fi

"${install_directory}/forgeguard" init --global --agent all

echo
echo "ForgeGuard installed: ${install_directory}/forgeguard"
echo "Restart terminal, then run inside a repository:"
echo "  forgeguard init --agent all"
