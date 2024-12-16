#!/usr/bin/env bash
set -euo pipefail

usage() {
    cat <<'EOF'
Usage: scripts/regenerate-static-media.sh [--skip-build] [--cli PATH]

Regenerates:
  static/screen.svg
  static/recording.gif

The controller and monitor demos are recorded manually and are not changed.
EOF
}

skip_build=0
cli_path=""
while (($#)); do
    case "$1" in
        --skip-build)
            skip_build=1
            shift
            ;;
        --cli)
            if (($# < 2)); then
                echo "--cli requires a path" >&2
                exit 2
            fi
            cli_path=$2
            shift 2
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

script_dir=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
repo_root=$(cd -- "$script_dir/.." && pwd)
static_dir="$repo_root/static"
temp_dir=$(mktemp -d "${TMPDIR:-/tmp}/termprobe-static-media.XXXXXX")
session_prefix="static-media-$$"
sessions=()

close_session() {
    local session=$1
    local exit_code

    if [[ -z ${cli_path:-} || ! -f $cli_path ]]; then
        return
    fi

    set +e
    "$cli_path" --session "$session" close >/dev/null 2>&1
    exit_code=$?
