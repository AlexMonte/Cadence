use std::time::Duration;

use bevy::prelude::*;
use bevy::state::app::StatesPlugin;
use load_up::prelude::*;

#[derive(States, Clone, Eq, PartialEq, Hash, Debug, Default)]
enum GameState {
    #[default]
    Loading,
    Menu,
    Game,
}

impl LoadingStateFor for GameState {
    fn loading_state() -> Self {
        Self::Loading
    }
}

#[derive(Asset, TypePath, Clone)]
struct DataAsset;

#[derive(Resource, Asset, LoadAssetResource, TypePath, Clone)]
struct GameAssets {
    #[dependency(path = "primary.data", fallback = "fallback.data", timeout = 10.0)]
    data: Handle<DataAsset>,
}

#[derive(Resource, Asset, LoadAssetResource, TypePath, Clone)]
struct MenuAssets {
    #[dependency(path = "menu.data", retry = 2, retry_cooldown = 0.5)]
    data: Handle<DataAsset>,
}

fn make_data_asset() -> DataAsset {
    DataAsset
}

#[derive(Resource, Asset, LoadAssetResource, TypePath, Clone)]
struct ImmediateAssets {
    #[dependency(init = make_data_asset)]
    data: Handle<DataAsset>,
}

#[derive(Resource, Default)]
struct CancellationCount(usize);

fn app() -> App {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .add_plugins(AssetPlugin::default())
        .add_plugins(StatesPlugin);
    app
}

#[test]
fn state_intercept_prepares_resources_and_holds_loading_state() {
    let mut app = app();
    app.add_plugins(LoadUpPlugin::<GameState>::default())
        .require_resource_for_state::<GameAssets>(GameState::Game, RetentionPolicy::Keep)
        .init_state::<GameState>()
        .add_systems(Startup, |mut next: ResMut<NextState<GameState>>| {
            next.set(GameState::Game);
        });

    app.update();

    assert_eq!(
        *app.world().resource::<State<GameState>>().get(),
        GameState::Loading
    );
    assert!(app
        .world()
        .contains_resource::<ActiveStateLoad<GameState>>());
    assert_eq!(app.world().resource::<PreparedResources>().ids().len(), 1);
}

#[test]
fn plugin_configures_bounded_journal_capacity() {
    let mut app = app();
    app.add_plugins(LoadUpPlugin::<GameState>::default().with_journal_capacity(2))
        .init_state::<GameState>();
    assert_eq!(app.world().resource::<TransitionJournal>().capacity(), 2);
}

#[test]
fn typed_replacement_updates_prepared_asset_and_live_resource() {
    let mut app = app();
    app.register_asset_resource::<GameAssets>()
        .prepare_resource_now::<GameAssets>();

    let entry_handle = app
        .world()
        .resource::<PreparedResources>()
        .get(GameAssets::id())
        .unwrap()
        .handle
        .clone();
    let pack = app
        .world()
        .resource::<Assets<GameAssets>>()
        .get(entry_handle.id().typed::<GameAssets>())
        .unwrap()
        .clone();
    app.world_mut().insert_resource(pack.clone());

    let mut dependency = None;
    pack.visit_load_up_dependencies(&mut |metadata| dependency = Some(metadata));
    let dependency = dependency.unwrap();
    let replacement = (dependency.acquire_fallback)(app.world_mut(), "fallback.data");
    let replacement_id = replacement.id();
    (dependency.replace)(app.world_mut(), &entry_handle, replacement);

    assert_eq!(
        app.world().resource::<GameAssets>().data.id().untyped(),
        replacement_id
    );
    assert_eq!(
        app.world()
            .resource::<Assets<GameAssets>>()
            .get(entry_handle.id().typed::<GameAssets>())
            .unwrap()
            .data
            .id()
            .untyped(),
        replacement_id
    );
}

#[test]
fn retry_is_idempotent_and_reacquires_primary() {
    let mut app = app();
    app.register_asset_resource::<GameAssets>()
        .prepare_resource_now::<GameAssets>();
    let id = app
        .world()
        .resource::<PreparedResources>()
        .get(GameAssets::id())
        .unwrap()
        .dependencies[0];
    {
        let mut registry = app.world_mut().resource_mut::<DependencyRegistry>();
        let dependency = registry.get_mut(id).unwrap();
        dependency.state.status = DependencyStatus::Failed(DependencyFailure::LoadFailed);
        dependency.state.acquisition = AcquisitionState::Exhausted;
        dependency.state.retry_after = Some(Duration::from_secs(99));
        dependency.state.last_failure = Some(DependencyFailure::LoadFailed);
    }

    assert!(retry_dependency(app.world_mut(), id).unwrap());
    assert!(!retry_dependency(app.world_mut(), id).unwrap());
    let dependency = app
        .world()
        .resource::<DependencyRegistry>()
        .get(id)
        .unwrap();
    assert_eq!(dependency.state.status, DependencyStatus::Loading);
    assert_eq!(dependency.state.acquisition, AcquisitionState::Primary);
    assert_eq!(dependency.state.retry_after, None);
    assert_eq!(dependency.state.last_failure, None);
    assert_eq!(dependency.resolved_path.as_deref(), Some("primary.data"));
}

#[test]
fn reset_rebuilds_and_release_drops_runtime_records() {
    let mut app = app();
    app.register_asset_resource::<GameAssets>()
        .prepare_resource_now::<GameAssets>();
    let first_handle = app
        .world()
        .resource::<PreparedResources>()
        .get(GameAssets::id())
        .unwrap()
        .handle
        .id();

    reset_prepared_resource::<GameAssets>(app.world_mut()).unwrap();
    let entry = app
        .world()
        .resource::<PreparedResources>()
        .get(GameAssets::id())
        .unwrap();
    assert_ne!(entry.handle.id(), first_handle);
    assert_eq!(entry.ordinal, 0);

    assert!(release_prepared_resource::<GameAssets>(app.world_mut()));
    assert!(!app
        .world()
        .resource::<PreparedResources>()
        .is_started(GameAssets::id()));
    assert!(app
        .world()
        .resource::<DependencyRegistry>()
        .ids()
        .is_empty());
}

#[test]
fn registration_reports_duplicate_and_conflicting_roles() {
    let mut app = app();
    app.add_plugins(LoadUpPlugin::<GameState>::default())
        .init_state::<GameState>();
    app.try_require_resource_for_state::<GameAssets>(GameState::Game, RetentionPolicy::Keep)
        .unwrap();
    assert!(matches!(
        app.try_require_resource_for_state::<GameAssets>(GameState::Game, RetentionPolicy::Keep),
        Err(LoadUpSetupError::DuplicateRegistration { .. })
    ));
    assert!(matches!(
        app.try_stream_resource_for_state::<GameAssets>(GameState::Game, RetentionPolicy::Keep),
        Err(LoadUpSetupError::ConflictingRegistration { .. })
    ));
}

#[test]
fn snapshots_follow_registration_order_and_include_configuration() {
    let mut app = app();
    app.register_asset_resource::<MenuAssets>()
        .register_asset_resource::<GameAssets>()
        .prepare_resource_now::<GameAssets>()
        .prepare_resource_now::<MenuAssets>();
    let snapshot = load_up_snapshot::<GameState>(app.world());
    assert_eq!(snapshot.resources[0].id, MenuAssets::id());
    assert_eq!(snapshot.resources[1].id, GameAssets::id());
    assert_eq!(
        snapshot.resources[1].dependencies[0].path.as_deref(),
        Some("primary.data")
    );
}

#[test]
fn timeout_and_retry_options_use_duration() {
    let mut app = app();
    app.register_asset_resource::<GameAssets>()
        .register_asset_resource::<MenuAssets>()
        .prepare_resource_now::<GameAssets>()
        .prepare_resource_now::<MenuAssets>();
    let registry = app.world().resource::<DependencyRegistry>();
    let game = registry
        .get(
            app.world()
                .resource::<PreparedResources>()
                .get(GameAssets::id())
                .unwrap()
                .dependencies[0],
        )
        .unwrap();
    assert_eq!(game.timeout, Some(Duration::from_secs(10)));
    let menu = registry
        .get(
            app.world()
                .resource::<PreparedResources>()
                .get(MenuAssets::id())
                .unwrap()
                .dependencies[0],
        )
        .unwrap();
    assert_eq!(
        menu.policy,
        FailurePolicy::Retry {
            attempts: 2,
            cooldown: Duration::from_millis(500)
        }
    );
}

#[test]
fn timeout_flows_through_fallback_and_terminal_state_leaves_polling() {
    let mut app = app();
    app.register_asset_resource::<GameAssets>()
        .prepare_resource_now::<GameAssets>();
    assert_eq!(
        app.world().resource::<DependencyRegistry>().pending_len(),
        1
    );

    app.world_mut()
        .resource_mut::<Time>()
        .advance_by(Duration::from_secs(11));
    poll_dependencies(app.world_mut());
    let id = app
        .world()
        .resource::<PreparedResources>()
        .get(GameAssets::id())
        .unwrap()
        .dependencies[0];
    let dependency = app
        .world()
        .resource::<DependencyRegistry>()
        .get(id)
        .unwrap();
    assert_eq!(dependency.state.acquisition, AcquisitionState::Fallback);
    assert_eq!(
        dependency.state.last_failure,
        Some(DependencyFailure::TimedOut)
    );
    assert_eq!(dependency.resolved_path.as_deref(), Some("fallback.data"));

    app.world_mut()
        .resource_mut::<Time>()
        .advance_by(Duration::from_secs(11));
    poll_dependencies(app.world_mut());
    let dependency = app
        .world()
        .resource::<DependencyRegistry>()
        .get(id)
        .unwrap();
    assert_eq!(
        dependency.state.status,
        DependencyStatus::Failed(DependencyFailure::FallbackFailed)
    );
    assert_eq!(
        app.world().resource::<DependencyRegistry>().pending_len(),
        0
    );
    let journal = app.world().resource::<TransitionJournal>();
    assert!(journal
        .entries()
        .any(|entry| entry.failure == Some(DependencyFailure::TimedOut) && !entry.terminal));
    assert!(journal.entries().any(|entry| entry.terminal));
}

#[test]
fn unload_policy_releases_pack_when_leaving_state() {
    let mut app = app();
    app.add_plugins(LoadUpPlugin::<GameState>::default())
        .require_resource_for_state::<ImmediateAssets>(GameState::Game, RetentionPolicy::Unload)
        .init_state::<GameState>();

    app.world_mut()
        .resource_mut::<NextState<GameState>>()
        .set(GameState::Game);
    for _ in 0..3 {
        app.update();
    }
    assert_eq!(
        *app.world().resource::<State<GameState>>().get(),
        GameState::Game
    );
    assert!(app.world().contains_resource::<ImmediateAssets>());

    app.world_mut()
        .resource_mut::<NextState<GameState>>()
        .set(GameState::Menu);
    app.update();
    assert!(!app.world().contains_resource::<ImmediateAssets>());
    assert!(!app
        .world()
        .resource::<PreparedResources>()
        .is_started(ImmediateAssets::id()));
}

#[test]
fn remove_resource_keeps_prepared_pack_and_reinserts_on_revisit() {
    let mut app = app();
    app.add_plugins(LoadUpPlugin::<GameState>::default())
        .require_resource_for_state::<ImmediateAssets>(
            GameState::Game,
            RetentionPolicy::RemoveResource,
        )
        .init_state::<GameState>();

    app.world_mut()
        .resource_mut::<NextState<GameState>>()
        .set(GameState::Game);
    for _ in 0..3 {
        app.update();
    }
    app.world_mut()
        .resource_mut::<NextState<GameState>>()
        .set(GameState::Menu);
    app.update();
    assert!(!app.world().contains_resource::<ImmediateAssets>());
    assert!(app
        .world()
        .resource::<PreparedResources>()
        .is_started(ImmediateAssets::id()));

    app.world_mut()
        .resource_mut::<NextState<GameState>>()
        .set(GameState::Game);
    app.update();
    app.update();
    assert!(app.world().contains_resource::<ImmediateAssets>());
}

#[test]
fn unload_does_not_release_pack_shared_with_destination() {
    let mut app = app();
    app.add_plugins(LoadUpPlugin::<GameState>::default())
        .require_resource_for_state::<ImmediateAssets>(GameState::Game, RetentionPolicy::Unload)
        .require_resource_for_state::<ImmediateAssets>(GameState::Menu, RetentionPolicy::Keep)
        .init_state::<GameState>();

    app.world_mut()
        .resource_mut::<NextState<GameState>>()
        .set(GameState::Game);
    for _ in 0..3 {
        app.update();
    }
    app.world_mut()
        .resource_mut::<NextState<GameState>>()
        .set(GameState::Menu);
    app.update();
    assert!(app.world().contains_resource::<ImmediateAssets>());
    assert!(app
        .world()
        .resource::<PreparedResources>()
        .is_started(ImmediateAssets::id()));
}

#[test]
fn superseded_load_is_cancelled_and_exclusive_pack_is_released() {
    let mut app = app();
    app.add_plugins(
        LoadUpPlugin::<GameState>::default()
            .with_cancellation_policy(CancellationPolicy::ReleaseExclusive),
    )
    .require_resource_for_state::<GameAssets>(GameState::Game, RetentionPolicy::Keep)
    .require_resource_for_state::<MenuAssets>(GameState::Menu, RetentionPolicy::Keep)
    .init_state::<GameState>()
    .init_resource::<CancellationCount>()
    .add_observer(
        |_: On<StateLoadCancelled<GameState>>, mut count: ResMut<CancellationCount>| {
            count.0 += 1;
        },
    );

    app.world_mut()
        .resource_mut::<NextState<GameState>>()
        .set(GameState::Game);
    app.update();
    let first_generation = app
        .world()
        .resource::<ActiveStateLoad<GameState>>()
        .generation;
    assert!(app
        .world()
        .resource::<PreparedResources>()
        .is_started(GameAssets::id()));

    app.world_mut()
        .resource_mut::<NextState<GameState>>()
        .set(GameState::Menu);
    app.update();
    let active = app.world().resource::<ActiveStateLoad<GameState>>();
    assert_eq!(active.target, GameState::Menu);
    assert_ne!(active.generation, first_generation);
    assert_eq!(app.world().resource::<CancellationCount>().0, 1);
    assert!(!app
        .world()
        .resource::<PreparedResources>()
        .is_started(GameAssets::id()));
}
