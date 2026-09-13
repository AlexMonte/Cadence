//! Dialogs own a modal tab group and restore the initiating focus on dismissal.
use crate::application::editor::interaction::keyboard_navigation::KeyboardModal;
use bevy::{
    input_focus::{
        InputFocus,
        tab_navigation::{NavAction, TabIndex, TabNavigation},
    },
    prelude::*,
};

pub(super) fn plugin(app: &mut App) {
    app.add_systems(
        PostUpdate,
        (update_tab_visibility, manage_modal_focus, show_focus_ring)
            .chain()
            .before(bevy::ui::UiSystems::Layout),
    )
    .add_systems(
        PreUpdate,
        return_to_board.after(bevy::input_focus::InputFocusSystems::Dispatch),
    );
}

#[derive(Component)]
struct HiddenTabIndex(i32);

/// Bevy 0.18 tab navigation enumerates TabIndex without testing Display::None.
/// Update only newly focusable controls and subtrees whose display toggled.
fn update_tab_visibility(
    changed: Query<(Entity, &Node), Changed<Node>>,
    children: Query<&Children>,
    parents: Query<&ChildOf>,
    nodes: Query<&Node>,
    mut indices: ParamSet<(
        Query<Entity, Added<TabIndex>>,
        Query<(&mut TabIndex, Option<&HiddenTabIndex>)>,
    )>,
    mut removed: RemovedComponents<Node>,
    mut hidden_roots: Local<std::collections::HashSet<Entity>>,
    mut focus: ResMut<InputFocus>,
    mut commands: Commands,
) {
    for entity in removed.read() {
        hidden_roots.remove(&entity);
    }
    let mut candidates: Vec<Entity> = indices.p0().iter().collect();
    for (entity, node) in &changed {
        let changed = if node.display == Display::None {
            hidden_roots.insert(entity)
        } else {
            hidden_roots.remove(&entity)
        };
        if changed {
            candidates.push(entity);
            candidates.extend(children.iter_descendants(entity));
        }
    }
    candidates.sort_unstable();
    candidates.dedup();
    let mut indices = indices.p1();
    for entity in candidates {
        let Ok((mut index, original)) = indices.get_mut(entity) else {
            continue;
        };
        let hidden = std::iter::once(entity)
            .chain(parents.iter_ancestors(entity))
            .any(|entity| {
                nodes
                    .get(entity)
                    .is_ok_and(|node| node.display == Display::None)
            });
        if hidden {
            if original.is_none() {
                commands.entity(entity).insert(HiddenTabIndex(index.0));
            }
            index.0 = -1;
            if focus.0 == Some(entity) {
                focus.clear();
            }
        } else if let Some(original) = original {
            index.0 = original.0;
            commands.entity(entity).remove::<HiddenTabIndex>();
        }
    }
}

#[derive(Component)]
struct FocusRing;

fn ring_node(visible: bool) -> Node {
    Node {
        position_type: PositionType::Absolute,
        left: px(0),
        top: px(0),
        width: percent(100),
        height: percent(100),
        border: UiRect::all(px(2)),
        border_radius: BorderRadius::all(px(3)),
        display: if visible {
            Display::Flex
        } else {
            Display::None
        },
        ..default()
    }
}

fn show_focus_ring(
    focus: Res<InputFocus>,
    visible: Res<bevy::input_focus::InputFocusVisible>,
    theme: Res<crate::infrastructure::ui::theme::MusaicUiTheme>,
    nodes: Query<(), With<Node>>,
    mut commands: Commands,
    mut previous: Local<Option<Entity>>,
    mut ring: Local<Option<Entity>>,
) {
    let current = focus
        .0
        .filter(|entity| visible.0 && nodes.contains(*entity));
    let existing = ring.filter(|entity| nodes.contains(*entity));
    if current == *previous && !theme.is_changed() && (current.is_none() || existing.is_some()) {
        return;
    }
    // Bevy 0.18 clamps negative Outline offsets to zero. A retained inner border
    // remains visible at scroll boundaries and never changes the control layout.
    if let Some(parent) = current {
        if let Some(entity) = existing {
            commands.entity(entity).insert((
                ChildOf(parent),
                ring_node(true),
                BorderColor::all(theme.chrome.accent),
            ));
        } else {
            *ring = Some(
                commands
                    .spawn((
                        FocusRing,
                        ChildOf(parent),
                        ring_node(true),
                        BorderColor::all(theme.chrome.accent),
                        ZIndex(100),
                        Pickable::IGNORE,
                    ))
                    .id(),
            );
        }
    } else if let Some(entity) = existing {
        commands.entity(entity).insert(ring_node(false));
    }
    *previous = current;
}

fn return_to_board(
    mut keys: ResMut<ButtonInput<KeyCode>>,
    mut focus: ResMut<InputFocus>,
    modals: Query<(), With<KeyboardModal>>,
    windows: Query<(), With<bevy::window::PrimaryWindow>>,
    state: Res<State<crate::infrastructure::app::AppState>>,
) {
    if *state.get() == crate::infrastructure::app::AppState::Editor
        && modals.is_empty()
        && keys.just_pressed(KeyCode::Escape)
        && focus.0.is_some_and(|entity| !windows.contains(entity))
    {
        focus.clear();
        // Consume cancellation at this level; the next Escape may cancel a tool.
        keys.clear_just_pressed(KeyCode::Escape);
    }
}

fn manage_modal_focus(
    modals: Query<(Entity, Option<&GlobalZIndex>), With<KeyboardModal>>,
    entities: Query<Entity>,
    hidden: Query<(), With<HiddenTabIndex>>,
    navigation: TabNavigation,
    mut focus: ResMut<InputFocus>,
    mut stack: Local<Vec<(Entity, Option<Entity>)>>,
) {
    // Unwind in reverse opening order, preserving focus through nested dialogs.
    while let Some((root, previous)) = stack.last().copied() {
        if modals.contains(root) {
            break;
        }
        stack.pop();
        focus.0 = previous.filter(|entity| entities.contains(*entity) && !hidden.contains(*entity));
    }
    let mut opened: Vec<_> = modals
        .iter()
        .filter(|(entity, _)| !stack.iter().any(|(root, _)| root == entity))
        .collect();
    opened.sort_by_key(|(_, z)| z.map_or(0, |z| z.0));
    for (root, _) in opened {
        if let Ok(first) = navigation.initialize(root, NavAction::First) {
            stack.push((root, focus.0));
            focus.set(first);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::input_focus::tab_navigation::{TabGroup, TabIndex};

    #[test]
    fn focus_border_is_retained_inside_controls_and_recovers_after_rebuild() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .init_resource::<InputFocus>()
            .insert_resource(bevy::input_focus::InputFocusVisible(true))
            .init_resource::<crate::infrastructure::ui::theme::MusaicUiTheme>()
            .add_systems(Update, show_focus_ring);
        let first = app.world_mut().spawn(Node::default()).id();
        let second = app.world_mut().spawn(Node::default()).id();
        app.world_mut().resource_mut::<InputFocus>().set(first);
        app.update();
        let ring = app
            .world_mut()
            .query_filtered::<Entity, With<FocusRing>>()
            .single(app.world())
            .unwrap();
        assert_eq!(app.world().get::<ChildOf>(ring).unwrap().parent(), first);
        app.world_mut().resource_mut::<InputFocus>().set(second);
        app.update();
        assert_eq!(app.world().get::<ChildOf>(ring).unwrap().parent(), second);
        let node = app.world().get::<Node>(ring).unwrap();
        assert_eq!(node.position_type, PositionType::Absolute);
        assert_eq!(node.width, percent(100));
        assert_eq!(node.left, px(0));
        // A control can rebuild children without changing focus.
        app.world_mut().despawn(ring);
        app.update();
        assert_eq!(
            app.world_mut()
                .query_filtered::<Entity, With<FocusRing>>()
                .iter(app.world())
                .count(),
            1
        );
        app.world_mut().resource_mut::<InputFocus>().clear();
        app.update();
        let node = app
            .world_mut()
            .query_filtered::<&Node, With<FocusRing>>()
            .single(app.world())
            .unwrap();
        assert_eq!(node.display, Display::None);
    }

    #[test]
    fn collapsed_panels_leave_tab_order_and_restore_their_original_indices() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .init_resource::<InputFocus>();
        app.add_systems(Update, update_tab_visibility);
        let root = app.world_mut().spawn(Node::default()).id();
        let field = app.world_mut().spawn((TabIndex(3), ChildOf(root))).id();
        app.world_mut().resource_mut::<InputFocus>().set(field);
        app.update();
        app.world_mut().get_mut::<Node>(root).unwrap().display = Display::None;
        app.update();
        assert_eq!(app.world().get::<TabIndex>(field).unwrap().0, -1);
        assert_eq!(app.world().resource::<InputFocus>().0, None);
        // A field added while its panel is closed is also excluded.
        let added = app.world_mut().spawn((TabIndex(1), ChildOf(root))).id();
        app.update();
        assert_eq!(app.world().get::<TabIndex>(added).unwrap().0, -1);
        app.world_mut().get_mut::<Node>(root).unwrap().display = Display::Flex;
        app.update();
        assert_eq!(app.world().get::<TabIndex>(field).unwrap().0, 3);
        assert_eq!(app.world().get::<TabIndex>(added).unwrap().0, 1);
    }

    #[test]
    fn modal_focus_stays_inside_and_returns_to_the_initiator() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .init_resource::<InputFocus>();
        app.add_systems(Update, manage_modal_focus);
        let origin = app.world_mut().spawn(TabIndex(0)).id();
        app.world_mut().resource_mut::<InputFocus>().set(origin);
        let root = app
            .world_mut()
            .spawn((KeyboardModal, TabGroup::modal()))
            .id();
        let first = app.world_mut().spawn((TabIndex(0), ChildOf(root))).id();
        let last = app.world_mut().spawn((TabIndex(0), ChildOf(root))).id();
        app.update();
        assert_eq!(app.world().resource::<InputFocus>().0, Some(first));
        app.world_mut().resource_mut::<InputFocus>().set(last);
        use bevy::ecs::system::RunSystemOnce;
        let next = app
            .world_mut()
            .run_system_once(|nav: TabNavigation, focus: Res<InputFocus>| {
                nav.navigate(&focus, NavAction::Next).unwrap()
            })
            .unwrap();
        assert_eq!(next, first);
        app.world_mut().despawn(root);
        app.update();
        assert_eq!(app.world().resource::<InputFocus>().0, Some(origin));
    }
}
