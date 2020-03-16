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
