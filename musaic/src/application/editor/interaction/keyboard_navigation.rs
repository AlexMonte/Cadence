//! Keyboard authoring intents. The command bus remains the sole editing path.

use super::navigation::{self, Motion};
use crate::application::editor::preferences::{EditorPreferences, keymap::Action};
use crate::{
    application::{
        command::{EditorCommand, EditorCommandBus},
        editor::{EditorAttention, EditorSession, FocusTarget, SelectionMode},
        pipeline::scene_sync::VisibleBoardState,
        session::MusaicProject,
    },
    domain::document::PlacementAddress,
    infrastructure::app::{AppState, MusaicSet},
};
use bevy::{input_focus::InputFocus, prelude::*, window::PrimaryWindow};

/// A modal owns keyboard input; background authoring must not react.
#[derive(Component)]
pub struct KeyboardModal;

#[derive(Resource, Default)]
pub struct KeyboardNavigation {
    pub help_open: bool,
    pub count: u16,
    pub selection_anchor: Option<FocusTarget>,
    pub status: String,
    prefix_g: bool,
}

#[derive(Message)]
pub struct KeyboardFocusMoved(pub PlacementAddress);

pub(super) fn plugin(app: &mut App) {
    app.init_resource::<KeyboardNavigation>()
        .init_resource::<crate::application::editor::preferences::EditorPreferences>()
        .add_message::<KeyboardFocusMoved>()
        .add_systems(OnExit(AppState::Editor), reset)
        .add_systems(
            Update,
            (reset_when_unavailable, dispatch)
                .chain()
                .in_set(MusaicSet::Input)
                .run_if(in_state(AppState::Editor)),
        );
}

fn reset(mut state: ResMut<KeyboardNavigation>) {
    *state = KeyboardNavigation::default();
}

pub fn shortcuts_available(
    focus: Option<Res<InputFocus>>,
    modals: Query<(), With<KeyboardModal>>,
    windows: Query<&Window, With<PrimaryWindow>>,
) -> bool {
    modals.is_empty()
        && focus.is_none_or(|focus| focus.0.is_none_or(|entity| windows.contains(entity)))
        && windows.iter().all(|window| window.focused)
}

fn reset_when_unavailable(
    focus: Option<Res<InputFocus>>,
    modals: Query<(), With<KeyboardModal>>,
    windows: Query<&Window, With<PrimaryWindow>>,
    mut state: ResMut<KeyboardNavigation>,
    mut changes: MessageReader<crate::application::editor::ActiveSurfaceChanged>,
    preferences: Res<EditorPreferences>,
) {
    let changed = changes.read().next().is_some();
    if changed || preferences.is_changed() || !shortcuts_available(focus, modals, windows) {
        state.count = 0;
        state.prefix_g = false;
        state.selection_anchor = None;
    }
}

#[derive(bevy::ecs::system::SystemParam)]
struct NavigationInput<'w, 's> {
    keys: Res<'w, ButtonInput<KeyCode>>,
    state: ResMut<'w, KeyboardNavigation>,
    preferences: Res<'w, crate::application::editor::preferences::EditorPreferences>,
    attention: Res<'w, EditorAttention>,
    session: Res<'w, EditorSession>,
    project: Res<'w, MusaicProject>,
    visible: Res<'w, VisibleBoardState>,
    focus: Option<Res<'w, InputFocus>>,
    modals: Query<'w, 's, (), With<KeyboardModal>>,
    windows: Query<'w, 's, &'static Window, With<PrimaryWindow>>,
    bus: MessageWriter<'w, EditorCommandBus>,
    camera: MessageWriter<'w, KeyboardFocusMoved>,
}

fn node_at_focus(focus: &FocusTarget) -> Option<tessera::prelude::NodeId> {
    match focus {
        FocusTarget::Tile { node }
        | FocusTarget::Atom { node }
        | FocusTarget::Port { node, .. } => Some(node.clone()),
        _ => None,
    }
}

fn dispatch(mut input: NavigationInput) {
    if !input.modals.is_empty()
        || input.focus.as_ref().is_some_and(|focus| {
            focus
                .0
                .is_some_and(|entity| !input.windows.contains(entity))
        })
        || input.windows.iter().any(|window| !window.focused)
    {
        return;
    }
    let keys = &input.keys;
    let control = [
        KeyCode::SuperLeft,
        KeyCode::SuperRight,
        KeyCode::ControlLeft,
        KeyCode::ControlRight,
    ]
    .iter()
    .any(|key| keys.pressed(*key));
    let shift = keys.pressed(KeyCode::ShiftLeft) || keys.pressed(KeyCode::ShiftRight);
    if input.preferences.shortcut(Action::Help, keys).is_some() {
        input.state.help_open = !input.state.help_open;
        return;
    }
    if keys.just_pressed(KeyCode::Escape) {
        input.state.count = 0;
        input.state.prefix_g = false;
        input.state.selection_anchor = None;
        input.state.help_open = false;
        input.state.status = "Canceled · project stays open".into();
        return;
    }
    if input.state.help_open
        || input.session.is_placing_from_drawer()
        || input.session.is_connecting()
    {
        return;
    }
    let vim = input.preferences.vim_navigation && input.preferences.keymap.character_shortcuts;
    let plain_g = input
        .preferences
        .keymap
        .gg_enabled(input.preferences.vim_navigation)
        && keys.just_pressed(KeyCode::KeyG)
        && !control
        && !shift
        && !keys.any_pressed([KeyCode::AltLeft, KeyCode::AltRight]);
    let pressed = |action| input.preferences.shortcut(action, keys).is_some();
    let extend = [
        Action::SelectLeft,
        Action::SelectRight,
        Action::SelectUp,
        Action::SelectDown,
    ]
    .into_iter()
    .any(pressed);
    let motion = if pressed(Action::Left) || pressed(Action::SelectLeft) {
        Some(Motion::Step(-1, 0))
    } else if pressed(Action::Right) || pressed(Action::SelectRight) {
        Some(Motion::Step(1, 0))
    } else if pressed(Action::Up) || pressed(Action::SelectUp) {
        Some(Motion::Step(0, -1))
    } else if pressed(Action::Down) || pressed(Action::SelectDown) {
        Some(Motion::Step(0, 1))
    } else if pressed(Action::NextExpression) {
        Some(Motion::Expression(1))
    } else if pressed(Action::PreviousExpression) {
        Some(Motion::Expression(-1))
    } else if pressed(Action::LastExpression) {
        Some(Motion::Last)
    } else if pressed(Action::FirstExpression) || plain_g && input.state.prefix_g {
        Some(Motion::First)
    } else {
        None
    };
    if let Some(motion) = motion {
        let count = input.state.count.max(1);
        input.state.count = 0;
        input.state.prefix_g = false;
        let Some(target) = navigation::navigate(&input.attention, &input.visible, motion, count)
        else {
            return;
        };
        if extend || input.state.selection_anchor.is_some() {
            let anchor = input
                .state
                .selection_anchor
                .clone()
                .unwrap_or_else(|| input.attention.focus.clone());
            input.state.selection_anchor = Some(anchor.clone());
            input
                .bus
                .write(EditorCommandBus(EditorCommand::ClearSelection));
            for node in navigation::range_nodes(&input.visible, &anchor, &target) {
                input.bus.write(EditorCommandBus(EditorCommand::SelectNode {
                    node,
                    mode: SelectionMode::Add,
                }));
            }
        } else {
            input
                .bus
                .write(EditorCommandBus(EditorCommand::ClearSelection));
        }
        input.bus.write(EditorCommandBus(EditorCommand::Focus {
            target: target.clone(),
        }));
        let mut next_attention = input.attention.clone();
        next_attention.focus = target;
        let coordinate =
            if let Some(address) = navigation::focused_address(&next_attention, &input.visible) {
                input.camera.write(KeyboardFocusMoved(address));
                match address {
                    PlacementAddress::BoardSlot(slot) => {
                        crate::application::pipeline::selected_tile::grid_coordinate(slot.x, slot.y)
                    }
                    PlacementAddress::StackIndex(index) => {
                        format!("Tile {}", index.0.saturating_add(1))
                    }
                }
            } else {
                String::new()
            };
        let hint = if input.state.selection_anchor.is_some() {
            "Escape returns to navigation".into()
        } else {
            format!(
                "{} inspects or opens",
                input
                    .preferences
                    .keymap
                    .describe(Action::Inspect, input.preferences.vim_navigation)
            )
        };
        input.state.status = format!("{coordinate} · {hint}");
        return;
    }
    if plain_g {
        input.state.prefix_g = true;
        input.state.status = "g · press g for first expression, Escape to cancel".into();
        return;
    }
    if vim && !shift && !control && !keys.any_pressed([KeyCode::AltLeft, KeyCode::AltRight]) {
        for (digit, key) in [
            KeyCode::Digit0,
            KeyCode::Digit1,
            KeyCode::Digit2,
            KeyCode::Digit3,
            KeyCode::Digit4,
            KeyCode::Digit5,
            KeyCode::Digit6,
            KeyCode::Digit7,
            KeyCode::Digit8,
            KeyCode::Digit9,
        ]
        .into_iter()
        .enumerate()
        {
            if keys.just_pressed(key) {
                input.state.count = (input.state.count.saturating_mul(10) + digit as u16).min(999);
                input.state.prefix_g = false;
                return;
            }
        }
    }
    if input.keys.get_just_pressed().next().is_some() {
        input.state.count = 0;
        input.state.prefix_g = false;
    }
    if input
        .preferences
        .shortcut(Action::SelectMode, &input.keys)
        .is_some()
    {
        input.state.selection_anchor = if input.state.selection_anchor.is_some() {
            None
        } else {
            Some(input.attention.focus.clone())
        };
        return;
    }
    let focused = node_at_focus(&input.attention.focus);
    if input
        .preferences
        .shortcut(Action::Inspect, &input.keys)
        .is_some()
    {
        if let Some(tile) = input.session.armed_tile().cloned() {
            if let Some(address) = navigation::focused_address(&input.attention, &input.visible) {
                let surface = input.attention.active_board();
                let target = match address {
                    PlacementAddress::BoardSlot(slot) => {
                        crate::application::editor::transaction::PlacementTarget::BoardSlot {
                            surface,
                            slot,
                        }
                    }
                    PlacementAddress::StackIndex(index) => {
                        crate::application::editor::transaction::PlacementTarget::StackIndex {
                            surface,
                            index,
                        }
                    }
                };
                input
                    .bus
                    .write(EditorCommandBus(EditorCommand::PlaceTile { target, tile }));
            }
        } else if let Some(node) = focused {
            let command = if input
                .project
                .document
                .graph
                .container_surface(&node)
                .is_some()
            {
                EditorCommand::EnterContainer { container: node }
            } else {
                EditorCommand::SelectNode {
                    node,
                    mode: SelectionMode::Replace,
                }
            };
            input.bus.write(EditorCommandBus(command));
        }
    } else if input
        .preferences
        .shortcut(Action::Parent, &input.keys)
        .is_some()
    {
        let current = input.attention.active_board();
        let parent = input.project.document.graph.nodes().find_map(|node| {
            (input.project.document.graph.container_surface(&node.id) == Some(current))
                .then(|| input.project.document.graph.location_of(&node.id))
                .flatten()
                .map(|location| location.surface)
        });
        if let Some(surface) = parent {
            input
                .bus
                .write(EditorCommandBus(EditorCommand::NavigateToSurface {
                    surface,
                }));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        application::{
            editor::{EditorPlugin, SelectionState},
            pipeline::PlaybackPlugin,
        },
        infrastructure::app::TransportMode,
    };

    fn app() -> App {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, bevy::state::app::StatesPlugin))
            .init_state::<AppState>()
            .init_state::<TransportMode>()
            .init_resource::<InputFocus>()
            .add_plugins((EditorPlugin, PlaybackPlugin));
        crate::infrastructure::app::configure_pipeline_schedule(&mut app);
        app.insert_state(AppState::Editor);
        app.world_mut()
            .write_message(EditorCommandBus(EditorCommand::AdoptProject {
                project: MusaicProject::demo(),
                path: None,
            }));
        app.update();
        app.update();
        app.world_mut()
            .resource_mut::<crate::application::editor::preferences::EditorPreferences>()
            .vim_navigation = true;
        app
    }

    fn send(app: &mut App, command: EditorCommand) {
        app.world_mut().write_message(EditorCommandBus(command));
        app.update();
        app.update();
    }

    fn key(app: &mut App, key: KeyCode) {
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(key);
        app.update();
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .reset_all();
        app.update();
    }

    fn sequence(app: &App) -> tessera::prelude::NodeId {
        let project = app.world().resource::<MusaicProject>();
        project
            .document
            .graph
            .nodes_on_surface(project.document.root_surface)
            .into_iter()
            .find(|(_, node)| project.document.graph.container_surface(&node.id).is_some())
            .unwrap()
            .1
            .id
            .clone()
    }

    #[test]
    fn movement_skips_wide_faces_without_mutating_music_and_enters_containers() {
        let mut app = app();
        let sequence = sequence(&app);
        send(
            &mut app,
            EditorCommand::Focus {
                target: FocusTarget::Tile {
                    node: sequence.clone(),
                },
            },
        );
        let revision = app.world().resource::<MusaicProject>().document.revision;
        key(&mut app, KeyCode::KeyL);
        let at = app.world().resource::<EditorAttention>();
        assert_ne!(
            at.focus,
            FocusTarget::Tile {
                node: sequence.clone()
            }
        );
        assert_eq!(
            app.world().resource::<MusaicProject>().document.revision,
            revision
        );
        assert!(app.world().resource::<SelectionState>().nodes.is_empty());
        send(
            &mut app,
            EditorCommand::Focus {
                target: FocusTarget::Tile {
                    node: sequence.clone(),
                },
            },
        );
        key(&mut app, KeyCode::Enter);
        let surface = app
            .world()
            .resource::<MusaicProject>()
            .document
            .graph
            .container_surface(&sequence)
            .unwrap();
        assert_eq!(
            app.world().resource::<EditorAttention>().active_board(),
            surface
        );
        key(&mut app, KeyCode::KeyG);
        key(&mut app, KeyCode::KeyG);
        let first = app.world().resource::<EditorAttention>().focus.clone();
        key(&mut app, KeyCode::KeyW);
        let second = app.world().resource::<EditorAttention>().focus.clone();
        assert_ne!(first, second);
        key(&mut app, KeyCode::KeyB);
        assert_eq!(app.world().resource::<EditorAttention>().focus, first);
        key(&mut app, KeyCode::Minus);
        assert_eq!(
            app.world().resource::<EditorAttention>().active_board(),
            app.world()
                .resource::<MusaicProject>()
                .document
                .root_surface
        );
    }

    #[test]
    fn enter_places_the_armed_tile_at_a_negative_cursor_and_undo_restores_the_board() {
        use crate::domain::{
            board::BoardSlot,
            document::{AtomValue, TileSpawnKind},
        };
        let mut app = app();
        send(&mut app, EditorCommand::NewProject);
        let surface = app.world().resource::<EditorAttention>().active_board();
        let slot = BoardSlot::new(-4, -3);
        send(
            &mut app,
            EditorCommand::Focus {
                target: FocusTarget::EmptySlot { surface, slot },
            },
        );
        send(
            &mut app,
            EditorCommand::ArmPlacementTool {
                tile: TileSpawnKind::Atom {
                    atom: AtomValue::Number(7),
                },
            },
        );
        key(&mut app, KeyCode::Enter);
        let project = app.world().resource::<MusaicProject>();
        let nodes = project.document.graph.nodes_on_surface(surface);
        assert_eq!(nodes.len(), 1);
        assert_eq!(nodes[0].0.address, PlacementAddress::BoardSlot(slot));
        assert!(!app.world().resource::<EditorSession>().is_armed());
        key(&mut app, KeyCode::KeyU);
        assert!(
            app.world()
                .resource::<MusaicProject>()
                .document
                .graph
                .nodes_on_surface(surface)
                .is_empty()
        );
    }

    #[test]
    fn window_focus_accepts_motion_but_losing_window_focus_does_not() {
        let mut app = app();
        let window = app
            .world_mut()
            .spawn((
                PrimaryWindow,
                Window {
                    focused: true,
                    ..default()
                },
            ))
            .id();
        app.world_mut().resource_mut::<InputFocus>().set(window);
        let before = app.world().resource::<EditorAttention>().focus.clone();
        key(&mut app, KeyCode::ArrowRight);
        let after = app.world().resource::<EditorAttention>().focus.clone();
        assert_ne!(before, after);
        app.world_mut().get_mut::<Window>(window).unwrap().focused = false;
        key(&mut app, KeyCode::ArrowRight);
        assert_eq!(app.world().resource::<EditorAttention>().focus, after);
    }

    #[test]
    fn modals_and_focused_controls_block_background_commands_and_clear_counts() {
        let mut app = app();
        let node = sequence(&app);
        send(
            &mut app,
            EditorCommand::SelectNode {
                node: node.clone(),
                mode: SelectionMode::Replace,
            },
        );
        key(&mut app, KeyCode::Digit3);
        assert_eq!(app.world().resource::<KeyboardNavigation>().count, 3);
        let modal = app.world_mut().spawn(KeyboardModal).id();
        for code in [
            KeyCode::Delete,
            KeyCode::KeyX,
            KeyCode::Space,
            KeyCode::KeyD,
        ] {
            key(&mut app, code);
        }
        assert!(
            app.world()
                .resource::<MusaicProject>()
                .document
                .graph
                .node(&node)
                .is_some()
        );
        assert_eq!(
            *app.world().resource::<State<TransportMode>>().get(),
            TransportMode::Stopped
        );
        assert_eq!(app.world().resource::<KeyboardNavigation>().count, 0);
        app.world_mut().entity_mut(modal).despawn();
        let field = app.world_mut().spawn_empty().id();
        app.world_mut().resource_mut::<InputFocus>().set(field);
        let before = app.world().resource::<EditorAttention>().focus.clone();
        for code in [KeyCode::KeyL, KeyCode::Delete, KeyCode::Space] {
            key(&mut app, code);
        }
        assert_eq!(app.world().resource::<EditorAttention>().focus, before);
        assert!(
            app.world()
                .resource::<MusaicProject>()
                .document
                .graph
                .node(&node)
                .is_some()
        );
    }

    #[test]
    fn selection_can_shrink_and_escape_keeps_project_open() {
        let mut app = app();
        let node = sequence(&app);
        send(&mut app, EditorCommand::EnterContainer { container: node });
        key(&mut app, KeyCode::KeyG);
        key(&mut app, KeyCode::KeyG);
        key(&mut app, KeyCode::KeyV);
        key(&mut app, KeyCode::KeyW);
        assert_eq!(app.world().resource::<SelectionState>().nodes.len(), 2);
        key(&mut app, KeyCode::KeyB);
        assert_eq!(app.world().resource::<SelectionState>().nodes.len(), 1);
        key(&mut app, KeyCode::Escape);
        assert!(
            app.world()
                .resource::<KeyboardNavigation>()
                .selection_anchor
                .is_none()
        );
        assert_eq!(
            *app.world().resource::<State<AppState>>().get(),
            AppState::Editor
        );
    }
    #[test]
    fn remapped_motion_replaces_old_keys_and_obeys_field_and_modal_ownership() {
        use crate::application::editor::preferences::keymap::Chord;
        let mut app = app();
        send(&mut app, EditorCommand::NewProject);
        let surface = app.world().resource::<EditorAttention>().active_board();
        let origin = FocusTarget::EmptySlot {
            surface,
            slot: crate::domain::board::BoardSlot::new(0, 0),
        };
        send(
            &mut app,
            EditorCommand::Focus {
                target: origin.clone(),
            },
        );
        let mut raw = ButtonInput::default();
        raw.press(KeyCode::F8);
        app.world_mut()
            .resource_mut::<EditorPreferences>()
            .keymap
            .replace(Action::Right, vec![Chord::from_input(KeyCode::F8, &raw)])
            .unwrap();
        key(&mut app, KeyCode::ArrowRight);
        key(&mut app, KeyCode::KeyL);
        assert_eq!(app.world().resource::<EditorAttention>().focus, origin);
        let field = app.world_mut().spawn_empty().id();
        app.world_mut().resource_mut::<InputFocus>().set(field);
        key(&mut app, KeyCode::F8);
        assert_eq!(app.world().resource::<EditorAttention>().focus, origin);
        app.world_mut().resource_mut::<InputFocus>().clear();
        let modal = app.world_mut().spawn(KeyboardModal).id();
        key(&mut app, KeyCode::F8);
        assert_eq!(app.world().resource::<EditorAttention>().focus, origin);
        app.world_mut().despawn(modal);
        key(&mut app, KeyCode::F8);
        assert_eq!(
            app.world().resource::<EditorAttention>().focus,
            FocusTarget::EmptySlot {
                surface,
                slot: crate::domain::board::BoardSlot::new(1, 0)
            }
        );
    }
    #[test]
    fn vim_copy_delete_and_undo_keep_the_focused_ports_owning_tile() {
        let mut app = app();
        let node = sequence(&app);
        send(&mut app, EditorCommand::ClearSelection);
        app.world_mut().resource_mut::<EditorAttention>().focus = FocusTarget::Port {
            node: node.clone(),
            port: crate::domain::document::PortId(0),
        };
        key(&mut app, KeyCode::KeyY);
        let count = app
            .world()
            .resource::<MusaicProject>()
            .document
            .graph
            .nodes()
            .count();
        let surface = app.world().resource::<EditorAttention>().active_board();
        send(
            &mut app,
            EditorCommand::Focus {
                target: FocusTarget::EmptySlot {
                    surface,
                    slot: crate::domain::board::BoardSlot::new(50, 50),
                },
            },
        );
        key(&mut app, KeyCode::KeyP);
        assert!(
            app.world()
                .resource::<MusaicProject>()
                .document
                .graph
                .nodes()
                .count()
                > count
        );
        key(&mut app, KeyCode::KeyU);
        assert_eq!(
            app.world()
                .resource::<MusaicProject>()
                .document
                .graph
                .nodes()
                .count(),
            count
        );
        send(&mut app, EditorCommand::ClearSelection);
        app.world_mut().resource_mut::<EditorAttention>().focus = FocusTarget::Port {
            node: node.clone(),
            port: crate::domain::document::PortId(0),
        };
        key(&mut app, KeyCode::KeyX);
        assert!(
            app.world()
                .resource::<MusaicProject>()
                .document
                .graph
                .node(&node)
                .is_none()
        );
        key(&mut app, KeyCode::KeyU);
        assert!(
            app.world()
                .resource::<MusaicProject>()
                .document
                .graph
                .node(&node)
                .is_some()
        );
        assert_eq!(
            app.world()
                .resource::<MusaicProject>()
                .document
                .graph
                .nodes()
                .count(),
            count
        );
    }
}
