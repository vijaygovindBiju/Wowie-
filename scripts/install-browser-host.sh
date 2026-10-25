#!/usr/bin/env bash
set -euo pipefail

# Wowie Native Messaging Host Registration Script
# Installs or uninstalls native messaging manifests for Chromium, Chrome, Brave, and Firefox.

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"

HOST_NAME="org.wowie.browser_host"
UNINSTALL=false
TARGET_BIN=""

usage() {
    cat <<EOF
Usage: $(basename "$0") [OPTIONS]

Options:
  --bin <PATH>     Specify absolute path to wowie-browser-host binary.
                   Defaults to /usr/bin/wowie-browser-host if installed,
                   otherwise builds or locates target/release/wowie-browser-host.
  --uninstall      Remove native messaging host registrations.
  -h, --help       Show this help message.
EOF
}

while [[ $# -gt 0 ]]; do
    case "$1" in
        --bin)
            TARGET_BIN="$2"
            shift 2
            ;;
        --uninstall)
            UNINSTALL=true
            shift
            ;;
        -h|--help)
            usage
            exit 0
            ;;
        *)
            echo "Unknown option: $1" >&2
            usage
            exit 1
            ;;
    esac
done

# Browser native messaging configuration target directories
CHROMIUM_DIRS=(
    "${HOME}/.config/google-chrome/NativeMessagingHosts"
    "${HOME}/.config/chromium/NativeMessagingHosts"
    "${HOME}/.config/BraveSoftware/Brave-Browser/NativeMessagingHosts"
)
FIREFOX_DIRS=(
    "${HOME}/.mozilla/native-messaging-hosts"
)

if [[ "${UNINSTALL}" == "true" ]]; then
    echo "==> Uninstalling Wowie Native Messaging Host manifests..."
    for dir in "${CHROMIUM_DIRS[@]}"; do
        if [[ -f "${dir}/${HOST_NAME}.json" ]]; then
            rm -f "${dir}/${HOST_NAME}.json"
            echo "    Removed: ${dir}/${HOST_NAME}.json"
        fi
    done
    for dir in "${FIREFOX_DIRS[@]}"; do
        if [[ -f "${dir}/${HOST_NAME}.json" ]]; then
            rm -f "${dir}/${HOST_NAME}.json"
            echo "    Removed: ${dir}/${HOST_NAME}.json"
        fi
    done
    echo "==> Uninstallation complete."
    exit 0
fi

# Locate host binary
if [[ -z "${TARGET_BIN}" ]]; then
    if [[ -x "/usr/bin/wowie-browser-host" ]]; then
        TARGET_BIN="/usr/bin/wowie-browser-host"
    elif [[ -x "${REPO_ROOT}/target/release/wowie-browser-host" ]]; then
        TARGET_BIN="${REPO_ROOT}/target/release/wowie-browser-host"
    elif [[ -x "${REPO_ROOT}/target/debug/wowie-browser-host" ]]; then
        TARGET_BIN="${REPO_ROOT}/target/debug/wowie-browser-host"
    else
        echo "==> Building wowie-browser-host..."
        cargo build --release -p wowie-observer --bin wowie-browser-host --manifest-path "${REPO_ROOT}/Cargo.toml"
        TARGET_BIN="${REPO_ROOT}/target/release/wowie-browser-host"
    fi
fi

if [[ ! -x "${TARGET_BIN}" ]]; then
    echo "Error: Binary not executable: ${TARGET_BIN}" >&2
    exit 1
fi

echo "==> Registering Wowie Native Messaging Host"
echo "    Host binary: ${TARGET_BIN}"

# Generate Chromium manifest
CHROMIUM_MANIFEST=$(cat <<EOF
{
  "name": "${HOST_NAME}",
  "description": "Wowie Desktop AI Companion Native Messaging Host",
  "path": "${TARGET_BIN}",
  "type": "stdio",
  "allowed_origins": [
    "chrome-extension://*/*"
  ]
}
EOF
)

# Generate Firefox manifest
FIREFOX_MANIFEST=$(cat <<EOF
{
  "name": "${HOST_NAME}",
  "description": "Wowie Desktop AI Companion Native Messaging Host",
  "path": "${TARGET_BIN}",
  "type": "stdio",
  "allowed_extensions": [
    "wowie-companion@wowie.local"
  ]
}
EOF
)

# Install into Chromium-based browser paths
for dir in "${CHROMIUM_DIRS[@]}"; do
    mkdir -p "${dir}"
    echo "${CHROMIUM_MANIFEST}" > "${dir}/${HOST_NAME}.json"
    echo "    Installed: ${dir}/${HOST_NAME}.json"
done

# Install into Firefox paths
for dir in "${FIREFOX_DIRS[@]}"; do
    mkdir -p "${dir}"
    echo "${FIREFOX_MANIFEST}" > "${dir}/${HOST_NAME}.json"
    echo "    Installed: ${dir}/${HOST_NAME}.json"
done

echo "==> Native Messaging Host registered successfully!"
echo "    Load the extension unpacked from '${REPO_ROOT}/extensions/browser' in your browser."
