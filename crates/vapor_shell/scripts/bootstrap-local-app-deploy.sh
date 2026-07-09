#!/usr/bin/env bash
set -euo pipefail

usage() {
    cat <<'USAGE'
Usage:
  bootstrap-local-app-deploy.sh --binary PATH --target PATH [--manifest PATH] [--yes]

Installs the minimum local Vapor shell bootstrap into a Steam app directory:

  <target>/Vapor.toml
  <target>/bin/vapor

This intentionally does not copy authored source, Cargo workspaces, staged
payloads, or full depot contents. After this, run the installed
<target>/bin/vapor and let that app-local shell run setup, open
external source repos, build, package, and eventually self-deploy.

Default mode is a dry run. Add --yes to write files.
USAGE
}

script_dir="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
default_manifest="$(realpath -m "$script_dir/../../../..")/Vapor.toml"

binary=
manifest="$default_manifest"
target=
confirmed=false

while [ "$#" -gt 0 ]; do
    case "$1" in
        --binary)
            binary="${2:-}"
            shift 2
            ;;
        --manifest)
            manifest="${2:-}"
            shift 2
            ;;
        --target)
            target="${2:-}"
            shift 2
            ;;
        --yes)
            confirmed=true
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

if [ -z "$binary" ] || [ -z "$target" ]; then
    usage >&2
    exit 2
fi

binary="$(realpath "$binary")"
manifest="$(realpath "$manifest")"
target="$(realpath -m "$target")"

if [ ! -f "$binary" ]; then
    echo "vapor binary is not a file: $binary" >&2
    exit 1
fi

if [ ! -f "$manifest" ]; then
    echo "root manifest is not a file: $manifest" >&2
    exit 1
fi

if ! grep -Eq '^[[:space:]]*\[root\][[:space:]]*$' "$manifest"; then
    echo "manifest does not declare [root]: $manifest" >&2
    exit 1
fi

case "$target" in
    /|"$HOME"|"$HOME"/)
        echo "refusing unsafe deployment target: $target" >&2
        exit 1
        ;;
esac

case "$binary" in
    "$target"/*)
        echo "refusing binary already inside target app dir:" >&2
        echo "  binary: $binary" >&2
        echo "  target: $target" >&2
        exit 1
        ;;
esac

case "$manifest" in
    "$target"/*)
        echo "refusing manifest already inside target app dir:" >&2
        echo "  manifest: $manifest" >&2
        echo "  target:   $target" >&2
        exit 1
        ;;
esac

dest_manifest="$target/Vapor.toml"
dest_binary="$target/bin/vapor"

if [ "$confirmed" = true ]; then
    mkdir -p "$target/bin"
    install -m 0644 "$manifest" "$dest_manifest"
    install -m 0755 "$binary" "$dest_binary"
    echo "installed minimal Vapor shell bootstrap:"
else
    echo "dry-run minimal Vapor shell bootstrap:"
    echo "add --yes to apply"
fi

echo "  manifest: $manifest -> $dest_manifest"
echo "  binary:   $binary -> $dest_binary"
echo
echo "next:"
echo "  \"$dest_binary\" setup self status"
echo "  \"$dest_binary\" setup self install"
echo "  \"$dest_binary\" open /path/to/external/source"
