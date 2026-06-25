#!/usr/bin/env bash
# Run becket from source when the binary is not installed (e.g. before a release).
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
exec cargo run --quiet --manifest-path "$ROOT/Cargo.toml" -p becket-cli -- "$@"
