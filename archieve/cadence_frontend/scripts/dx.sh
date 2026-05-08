#!/usr/bin/env bash
set -euo pipefail

if [[ -x "${HOME}/.cargo/bin/dx" ]]; then
  exec "${HOME}/.cargo/bin/dx" "$@"
fi

exec dx "$@"
