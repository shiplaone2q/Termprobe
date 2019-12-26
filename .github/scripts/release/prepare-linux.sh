#!/usr/bin/env bash
# Shared toolchain setup for native engines and Python wheels.
set -euo pipefail

curl --proto '=https' --tlsv1.2 -fsSL https://sh.rustup.rs |
  sh -s -- -y --profile minimal --default-toolchain stable --no-modify-path

version=0.16.0
case "$(uname -m)" in
  x86_64|amd64)
    arch=x86_64
