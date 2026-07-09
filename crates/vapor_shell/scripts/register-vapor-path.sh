#!/usr/bin/env bash
set -euo pipefail

usage() {
    cat <<'USAGE'
Usage:
  register-vapor-path.sh [--app-root PATH] [--dry-run]

Registers <app-root>/bin in shell startup files so `vapor` works from a normal
Linux shell. The script writes only a marked Vapor PATH block and preserves all
other profile content.

When --app-root is omitted, the script assumes it lives in <app-root>/scripts.
USAGE
}

script_dir="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
app_root="$(realpath -m "$script_dir/..")"
dry_run=false

while [ "$#" -gt 0 ]; do
    case "$1" in
        --app-root)
            app_root="${2:-}"
            shift 2
            ;;
        --dry-run)
            dry_run=true
            shift
            ;;
        -h|--help)
            usage
            exit 0
            ;;
        *)
            echo "unknown argument: $1" >&2
            usage >&2
            exit 2
            ;;
    esac
done

if [ -z "$app_root" ]; then
    echo "app root must not be empty" >&2
    exit 2
fi

app_root="$(realpath -m "$app_root")"
bin_dir="$app_root/bin"
command_path="$bin_dir/vapor"

if [ ! -x "$command_path" ]; then
    echo "app-owned Vapor command is missing or not executable: $command_path" >&2
    exit 1
fi

block_start="# >>> Vapor managed PATH >>>"
block_end="# <<< Vapor managed PATH <<<"
block="$(cat <<BLOCK
$block_start
VAPOR_BIN='$bin_dir'
case ":\$PATH:" in
    *":\$VAPOR_BIN:"*) ;;
    *) export PATH="\$VAPOR_BIN\${PATH:+:\$PATH}" ;;
esac
unset VAPOR_BIN
$block_end
BLOCK
)"

profiles=("$HOME/.profile" "$HOME/.bashrc")

install_block() {
    local profile="$1"
    local current filtered
    current=""
    if [ -f "$profile" ]; then
        current="$(cat "$profile")"
    fi
    filtered="$(printf '%s\n' "$current" | sed "/^$block_start\$/,/^$block_end\$/d")"
    if [ -n "$filtered" ]; then
        filtered="${filtered%$'\n'}"
        filtered="$filtered"$'\n'
    fi
    updated="$filtered$block"$'\n'
    if [ "$dry_run" = true ]; then
        echo "would register PATH in $profile"
        return
    fi
    mkdir -p "$(dirname "$profile")"
    printf '%s' "$updated" > "$profile"
    echo "registered PATH in $profile"
}

for profile in "${profiles[@]}"; do
    install_block "$profile"
done

echo "PATH directory: $bin_dir"
echo "PATH command: $command_path"
echo "hint: open a new terminal, then run: vapor setup status"
