#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

DIST_INDEX="$ROOT_DIR/ui/dist/index.html"
if [[ -f "$DIST_INDEX" ]]; then
  for input in "$ROOT_DIR/ui/index.html" "$ROOT_DIR/ui/app.js" "$ROOT_DIR/ui/app.css"; do
    if [[ -f "$input" && "$input" -nt "$DIST_INDEX" ]]; then
      echo "UI dist is stale: $input is newer than $DIST_INDEX" >&2
      echo "Run: npm --prefix ui run build" >&2
      echo "For live source edits use: ./scripts/dev.sh" >&2
      exit 1
    fi
  done
fi

exec cargo run -p src-tauri "$@"
