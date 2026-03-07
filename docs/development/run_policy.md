# Cadence Run Policy

This document is the authoritative run-command policy for Cadence.

## Command Matrix

1. `./scripts/dev.sh`
- Canonical local development command.
- Runs `cargo tauri dev --config src-tauri/tauri.dev.conf.json`.
- Starts Vite dev server and always serves live `ui/` source.

2. `cargo run -p src-tauri`
- Dist-mode run command.
- Uses bundled frontend assets from `ui/dist`.
- Missing `ui/dist/index.html` triggers build preflight.
- Stale `ui/dist` is a hard error; rebuild first:
`npm --prefix ui run build`

3. `./scripts/run.sh`
- Dist-mode wrapper for `cargo run -p src-tauri`.
- Performs stale-dist precheck before launching.

4. `cargo tauri build`
- Production packaging workflow.
- Uses `beforeBuildCommand` to build frontend and package with `ui/dist`.

5. `npm --prefix ui run build`
- Explicit frontend rebuild for dist-mode workflows.

## Required Tooling

1. Node.js + npm installed and available on `PATH`.
2. Rust + Cargo.
3. Tauri CLI (`cargo-tauri`) for `cargo tauri dev` and `cargo tauri build`.

## Startup Guarantees

1. Missing frontend artifacts must never produce a blank window.
2. Dist-mode startup fails fast when `ui/dist` is stale, with explicit remediation steps.
3. Dev-mode startup never depends on `ui/dist` freshness.

## Troubleshooting

1. Dist-mode startup fails with stale UI error
- Run:
`npm --prefix ui run build`
- Then:
`./scripts/run.sh`

2. `cargo tauri dev` cannot start
- Confirm Tauri CLI is installed:
`cargo tauri --version`
- Confirm Vite can run:
`npm --prefix ui run dev -- --host`

3. `cargo tauri dev` opens with dist instead of Vite
- Run using the provided helper:
`./scripts/dev.sh`
- This command pins the dev config file:
`src-tauri/tauri.dev.conf.json`

4. Dist appears stale
- Rebuild:
`npm --prefix ui run build`
