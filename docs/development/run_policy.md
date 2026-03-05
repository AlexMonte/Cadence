# GrooveAtlas Run Policy

This document is the authoritative run-command policy for GrooveAtlas.

## Command Matrix

1. `cargo run -p src-tauri`
- Primary local run command.
- Uses bundled frontend assets from `ui/dist`.
- If `ui/dist/index.html` is missing, `src-tauri/build.rs` runs:
`npm --prefix ../ui run build`
- If build cannot run (for example `npm` is missing), build fails fast with an actionable error.

2. `cargo tauri dev`
- Frontend/HMR workflow.
- Uses `src-tauri/tauri.dev.conf.json`.
- Starts Vite dev server and loads app from `http://127.0.0.1:5173`.
- Shortcut:
`./scripts/dev.sh`

3. `cargo tauri build`
- Production packaging workflow.
- Uses `beforeBuildCommand` to build frontend and package with `ui/dist`.

4. `npm --prefix ui run build`
- Optional explicit frontend rebuild.
- Useful when you want to force-refresh `ui/dist`.

5. `./scripts/run.sh`
- Shortcut wrapper for:
`cargo run -p src-tauri`

## Required Tooling

1. Node.js + npm installed and available on `PATH`.
2. Rust + Cargo.
3. Tauri CLI (`cargo-tauri`) for `cargo tauri dev` and `cargo tauri build`.

## Startup Guarantees

1. Missing frontend artifacts must never produce a blank window.
2. Startup either:
- auto-builds missing frontend artifacts, or
- fails with explicit remediation steps.

## Troubleshooting

1. Blank window after startup
- Run:
`npm --prefix ui run build`
- Then:
`cargo run -p src-tauri`

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
