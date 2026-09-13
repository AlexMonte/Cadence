//! Major UI regions share F6 navigation; board input retains its existing owner.
use crate::application::editor::preferences::{EditorPreferences, keymap::Action};
use crate::{
    application::editor::interaction::keyboard_navigation::KeyboardModal,
    infrastructure::app::AppState,
};
use bevy::{
    input_focus::{InputFocus, InputFocusSystems, InputFocusVisible, tab_navigation::TabIndex},
    prelude::*,
};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default)]
pub(crate) enum RegionKind {
    #[default]
    Board,
    Toolbar,
    Inspector,
    Library,
    Timing,
}
impl RegionKind {
    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::Board => "Board",
            Self::Toolbar => "Toolbar",
            Self::Inspector => "Inspector",
            Self::Library => "Tile library",
            Self::Timing => "Pattern timing",
        }
    }
}
#[derive(Component)]
pub(crate) struct KeyboardRegion(pub RegionKind);
#[derive(Resource, Default)]
pub(crate) struct RegionNavigation {
    pub current: RegionKind,
    remembered: BTreeMap<RegionKind, Entity>,
}

pub(super) fn plugin(app: &mut App) {
    app.init_resource::<RegionNavigation>()
        .add_systems(
            OnExit(AppState::Editor),
            |mut state: ResMut<RegionNavigation>| *state = default(),
        )
        .add_systems(
            PreUpdate,
            switch_region
                .after(bevy::input::InputSystems)
                .before(InputFocusSystems::Dispatch)
                .run_if(in_state(AppState::Editor)),
        )
        .add_systems(
            Update,
            report_region
                .before(crate::infrastructure::app::MusaicSet::RenderUi)
                .run_if(in_state(AppState::Editor)),
        )
        .add_systems(
            PostUpdate,
            reveal_focus
                .after(bevy::ui::UiSystems::Layout)
                .run_if(in_state(AppState::Editor)),
        );
}
fn region_of(
    entity: Entity,
    parents: &Query<&ChildOf>,
    regions: &Query<&KeyboardRegion>,
) -> Option<RegionKind> {
    std::iter::once(entity)
        .chain(parents.iter_ancestors(entity))
        .find_map(|e| regions.get(e).ok().map(|r| r.0))
}
fn visible(entity: Entity, parents: &Query<&ChildOf>, nodes: &Query<&Node>) -> bool {
    std::iter::once(entity)
        .chain(parents.iter_ancestors(entity))
        .all(|e| nodes.get(e).is_ok_and(|node| node.display != Display::None))
}
#[derive(bevy::ecs::system::SystemParam)]
struct Regions<'w, 's> {
    markers: Query<'w, 's, &'static KeyboardRegion>,
    roots: Query<'w, 's, (Entity, &'static KeyboardRegion)>,
    parents: Query<'w, 's, &'static ChildOf>,
    children: Query<'w, 's, &'static Children>,
    nodes: Query<'w, 's, &'static Node>,
    indices: Query<'w, 's, &'static TabIndex>,
}
impl Regions<'_, '_> {
    fn target(&self, root: Entity, kind: RegionKind, remembered: Option<Entity>) -> Entity {
        let eligible = |entity| {
            visible(entity, &self.parents, &self.nodes)
                && self.indices.get(entity).is_ok_and(|i| i.0 >= 0)
                && region_of(entity, &self.parents, &self.markers) == Some(kind)
        };
        if let Some(last) = remembered.filter(|&e| eligible(e)) {
            return last;
        }
        // Stable tree order breaks ties between controls with the same TabIndex.
        std::iter::once(root)
            .chain(self.children.iter_descendants(root))
            .filter(|&e| eligible(e))
            .min_by_key(|e| self.indices.get(*e).unwrap().0)
            .unwrap_or(root)
    }
}
fn switch_region(
    preferences: Res<EditorPreferences>,
    mut keys: ResMut<ButtonInput<KeyCode>>,
    mut focus: ResMut<InputFocus>,
    mut focus_visible: ResMut<InputFocusVisible>,
    mut state: ResMut<RegionNavigation>,
    regions: Regions,
    modals: Query<(), With<KeyboardModal>>,
    windows: Query<&Window, With<bevy::window::PrimaryWindow>>,
) {
    if !modals.is_empty() || windows.iter().any(|w| !w.focused) {
        return;
    }
    let previous = preferences.shortcut(Action::PreviousRegion, &keys);
    let Some(key) = previous.or_else(|| preferences.shortcut(Action::NextRegion, &keys)) else {
        return;
    };
    let reverse = previous.is_some();
    keys.clear_just_pressed(key);
    let current = focus
        .0
        .and_then(|e| region_of(e, &regions.parents, &regions.markers))
        .unwrap_or(RegionKind::Board);
    if let Some(entity) = focus.0 {
        state.remembered.insert(current, entity);
    }
    let mut available: Vec<_> = regions
        .roots
        .iter()
        .filter(|(e, _)| visible(*e, &regions.parents, &regions.nodes))
        .map(|(e, r)| (r.0, e))
        .collect();
    available.sort_by_key(|(kind, _)| *kind);
    available.dedup_by_key(|(kind, _)| *kind);
    if available.is_empty() {
        return;
    }
    let index = if reverse {
        available
            .iter()
            .rposition(|(kind, _)| *kind < current)
            .unwrap_or(available.len() - 1)
    } else {
        available
            .iter()
            .position(|(kind, _)| *kind > current)
            .unwrap_or(0)
    };
    let (kind, root) = available[index];
    if kind == RegionKind::Board {
        focus.clear();
    } else {
        focus.set(regions.target(root, kind, state.remembered.get(&kind).copied()));
    }
    focus_visible.0 = true;
    state.current = kind;
}
fn report_region(
    focus: Res<InputFocus>,
    parents: Query<&ChildOf>,
    regions: Query<&KeyboardRegion>,
    modals: Query<(), With<KeyboardModal>>,
    mut state: ResMut<RegionNavigation>,
) {
    if !modals.is_empty() {
        return;
    }
    let current = focus
        .0
        .and_then(|e| region_of(e, &parents, &regions))
        .unwrap_or(RegionKind::Board);
    if state.current != current {
        state.current = current;
    }
}

/// Scroll only the focused control's ancestor chain, including nested library
/// and inspector viewports. This never moves the music camera or document.
pub(crate) fn reveal_focus(
    focus: Res<InputFocus>,
    parents: Query<&ChildOf>,
    geometry: Query<(Ref<ComputedNode>, Ref<UiGlobalTransform>)>,
    mut scrolls: Query<(
        &Node,
        &ComputedNode,
        &UiGlobalTransform,
        &mut ScrollPosition,
    )>,
) {
    let Some(entity) = focus.0 else {
        return;
    };
    let Ok((computed, transform)) = geometry.get(entity) else {
        return;
    };
    if !focus.is_changed() && !computed.is_changed() && !transform.is_changed() {
        return;
    }
    let mut rect = Rect::from_center_size(transform.translation, computed.size());
    for ancestor in parents.iter_ancestors(entity) {
        let Ok((node, computed, transform, mut scroll)) = scrolls.get_mut(ancestor) else {
            continue;
        };
        if node.overflow.y != OverflowAxis::Scroll {
            continue;
        }
        let viewport = Rect::from_center_size(transform.translation, computed.size());
        let delta = if rect.height() >= viewport.height() || rect.min.y < viewport.min.y {
            rect.min.y - viewport.min.y
        } else if rect.max.y > viewport.max.y {
            rect.max.y - viewport.max.y
        } else {
            0.0
        };
        let scale = computed.inverse_scale_factor();
        if scale <= 0.0 {
            continue;
        }
        let maximum = ((computed.content_size().y - computed.size().y) * scale).max(0.0);
        let next = (scroll.y + delta * scale).clamp(0.0, maximum);
        let applied = (next - scroll.y) / scale;
        if next != scroll.y {
            scroll.y = next;
        }
        rect.min.y -= applied;
        rect.max.y -= applied;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::input::{
        ButtonState,
        keyboard::{Key, KeyboardInput},
    };
    fn app() -> (App, Entity) {
        let mut app = App::new();
        app.add_plugins((
            MinimalPlugins,
            bevy::state::app::StatesPlugin,
            bevy::input::InputPlugin,
            bevy::input_focus::InputDispatchPlugin,
        ))
        .init_state::<AppState>()
        .insert_state(AppState::Editor)
        .init_resource::<EditorPreferences>()
        .init_resource::<InputFocusVisible>()
        .add_plugins(plugin);
        let window = app
            .world_mut()
            .spawn((
                bevy::window::PrimaryWindow,
                Window {
                    focused: true,
                    ..default()
                },
            ))
            .id();
        (app, window)
    }
    fn region(app: &mut App, kind: RegionKind) -> Entity {
        app.world_mut()
            .spawn((Node::default(), KeyboardRegion(kind), TabIndex(-1)))
            .id()
    }
    fn control(app: &mut App, parent: Entity) -> Entity {
        app.world_mut()
            .spawn((Node::default(), TabIndex(0), ChildOf(parent)))
            .id()
    }
    fn f6(app: &mut App, window: Entity, reverse: bool) {
        let state = if reverse {
            ButtonState::Pressed
        } else {
            ButtonState::Released
        };
        for (key_code, state) in [
            (KeyCode::ShiftLeft, state),
            (KeyCode::F6, ButtonState::Pressed),
            (KeyCode::F6, ButtonState::Released),
        ] {
            app.world_mut().write_message(KeyboardInput {
                key_code,
                logical_key: Key::F6,
                state,
                text: None,
                repeat: false,
                window,
            });
        }
        app.update();
    }
    #[test]
    fn f6_visits_visible_regions_restores_controls_and_returns_board_ownership() {
        let (mut app, window) = app();
        region(&mut app, RegionKind::Board);
        let toolbar = region(&mut app, RegionKind::Toolbar);
        let file = control(&mut app, toolbar);
        let inspector = region(&mut app, RegionKind::Inspector);
        let first = control(&mut app, inspector);
        let second = control(&mut app, inspector);
        let library = region(&mut app, RegionKind::Library);
        app.world_mut()
            .entity_mut(library)
            .insert(ChildOf(inspector));
        let search = control(&mut app, library);
        let timing = region(&mut app, RegionKind::Timing);
        app.world_mut().get_mut::<Node>(timing).unwrap().display = Display::None;
        f6(&mut app, window, false);
        assert_eq!(app.world().resource::<InputFocus>().0, Some(file));
        f6(&mut app, window, false);
        assert_eq!(app.world().resource::<InputFocus>().0, Some(first));
        app.world_mut().resource_mut::<InputFocus>().set(second);
        f6(&mut app, window, false);
        assert_eq!(app.world().resource::<InputFocus>().0, Some(search));
        f6(&mut app, window, true);
        assert_eq!(app.world().resource::<InputFocus>().0, Some(second));
        app.world_mut().get_mut::<Node>(library).unwrap().display = Display::None;
        f6(&mut app, window, false);
        assert_eq!(app.world().resource::<InputFocus>().0, None);
        assert!(app.world().resource::<InputFocusVisible>().0);
        // A rebuilt inspector falls back to a surviving control.
        app.world_mut().despawn(second);
        f6(&mut app, window, true);
        assert_eq!(app.world().resource::<InputFocus>().0, Some(first));
    }
    #[test]
    fn dialogs_and_unfocused_windows_keep_keyboard_ownership() {
        let (mut app, window) = app();
        region(&mut app, RegionKind::Board);
        let toolbar = region(&mut app, RegionKind::Toolbar);
        let button = control(&mut app, toolbar);
        app.world_mut().resource_mut::<InputFocus>().set(button);
        let modal = app.world_mut().spawn((KeyboardModal, Node::default())).id();
        f6(&mut app, window, false);
        assert_eq!(app.world().resource::<InputFocus>().0, Some(button));
        app.world_mut().despawn(modal);
        app.world_mut().get_mut::<Window>(window).unwrap().focused = false;
        f6(&mut app, window, false);
        assert_eq!(app.world().resource::<InputFocus>().0, Some(button));
    }
    #[test]
    fn nested_scroll_panels_reveal_focus_without_scrolling_twice() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .init_resource::<InputFocus>()
            .add_systems(Update, reveal_focus);
        let geometry = |y| {
            (
                ComputedNode {
                    size: Vec2::splat(100.0),
                    content_size: Vec2::new(100.0, 300.0),
                    inverse_scale_factor: 1.0,
                    ..default()
                },
                UiGlobalTransform::from(bevy::math::Affine2::from_translation(Vec2::new(0.0, y))),
            )
        };
        let outer = app
            .world_mut()
            .spawn((
                Node {
                    overflow: Overflow::scroll_y(),
                    ..default()
                },
                geometry(0.0),
                ScrollPosition::default(),
            ))
            .id();
        let inner = app
            .world_mut()
            .spawn((
                Node {
                    overflow: Overflow::scroll_y(),
                    ..default()
                },
                geometry(100.0),
                ScrollPosition::default(),
                ChildOf(outer),
            ))
            .id();
        let field = app
            .world_mut()
            .spawn((
                Node::default(),
                ComputedNode {
                    size: Vec2::splat(20.0),
                    ..default()
                },
                UiGlobalTransform::from(bevy::math::Affine2::from_translation(Vec2::new(
                    0.0, 230.0,
                ))),
                ChildOf(inner),
            ))
            .id();
        app.world_mut().resource_mut::<InputFocus>().set(field);
        app.update();
        assert_eq!(app.world().get::<ScrollPosition>(inner).unwrap().y, 90.0);
        assert_eq!(app.world().get::<ScrollPosition>(outer).unwrap().y, 100.0);
        app.update();
        assert_eq!(app.world().get::<ScrollPosition>(inner).unwrap().y, 90.0);
        assert_eq!(app.world().get::<ScrollPosition>(outer).unwrap().y, 100.0);
    }
}
