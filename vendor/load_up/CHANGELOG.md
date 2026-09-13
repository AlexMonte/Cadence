# Changelog

All notable changes to this project will be documented in this file.

## [0.3.0] - 2026-08-26

### Added

- Bevy 0.19.1 support across the core and optional integrations.
- `retry_prepared_resource` and `RetryPreparedResourceCommand` for explicit recovery after exhausted failures.
- `release_prepared_resource` for dropping prepared packs, dependency records, and inserted resources.
- Typed fallback acquisition with replacement-handle propagation into prepared and inserted resources.
- Canonical dependency/resource processing order for deterministic notifications.

### Changed

- Fallback handles now replace the corresponding field in the prepared pack instead of only updating diagnostic registry state.
- Optional integrations now target `iyes_progress` 0.17, `bevy-inspector-egui` 0.37, and `bevy_common_assets` 0.17.

## [0.2.0] - 2026-05-23

### Added

- Granular Bevy crate dependencies (`bevy_app`, `bevy_asset`, `bevy_ecs`, `bevy_state`, `bevy_time`) for a leaner library surface.
- Cargo features: `progress`, `tracing`, `debug-inspector`, `config-assets`.
- `UntypedAssetLoadFailedEvent` handling for faster failure recovery with path and `AssetLoadError` detail on `DependencyFailed`.
- Handle-indexed `DependencyRegistry` for event-driven dependency lookup.
- `LoadUpSetupError` and enriched `DependencyFailed` fields (`path`, `load_error`).
- Optional `LoadUpProgressPlugin` bridge for [`iyes_progress`](https://docs.rs/iyes_progress/0.16.0/iyes_progress/) loading bars.
- Optional `tracing` instrumentation (`RUST_LOG=load_up=debug`).
- Examples: `progress`, `debug_inspector`, `config_and_assets`.
- `trybuild` compile-fail tests for the `LoadAssetResource` macro.
- README ecosystem comparison with `bevy_asset_loader`.

### Changed

- `LoadAssetResource` attribute parsing refactored to use `darling`.
- Prelude re-exports common Bevy types for adopters.

## [0.1.1] - 2026-05-21

### Fixed

- `LoadUpPlugin` now auto-inserts `LoadingState` so state intercept works out of the box.
- `LoadUpPlugin` panics at startup with a clear message when `StatesPlugin` / `DefaultPlugins` is missing.
- `#[dependency(retry = N)]` now compiles by emitting `cooldown_seconds` (defaults to `0.0`).
- Dependency asset types (`Handle<T>` inner types) are auto-registered via `register_dependency_assets`.

### Added

- `retry_cooldown = SECS` macro attribute with real delay logic using Bevy `Time`.
- `state_safe = loaded` macro attribute for `StateSafeThreshold::Loaded`.
- `LoadUpPlugin` export in the prelude.
- Integration tests, `basic` example, CI workflow, and LICENSE files.

### Changed

- Improved `#[dependency(skip)]` error message (still unsupported).

## [0.1.0] - 2026-05-10

Initial release.
