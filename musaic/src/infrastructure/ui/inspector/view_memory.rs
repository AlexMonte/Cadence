//! Session-only inspector presentation, keyed by authored identities rather than UI entities.
use super::*;
use crate::application::editor::{ActiveSurfaceChangeReason, ActiveSurfaceChanged};
use crate::infrastructure::{
    app::{AppState, MusaicSet},
    ui::widgets::{ButtonLabel, musaic_chrome_button},
};
use bevy::{a11y::AccessibilityNode, input_focus::InputFocus};
use std::collections::VecDeque;
use tessera::prelude::NodeId;
const CAPACITY: usize = 64;
#[derive(Clone, PartialEq, Eq)]
struct ViewKey(Vec<InspectorPanelKind>);
impl ViewKey {
    fn new(layout: &InspectorLayout) -> Self {
        // Library position belongs to its context, not each empty insertion cell.
        Self(
            layout
                .panels
                .iter()
                .map(|panel| match panel {
                    InspectorPanelKind::DrawerPanel { context, .. } => {
                        InspectorPanelKind::DrawerPanel {
                            context: *context,
                            target: None,
                        }
                    }
                    InspectorPanelKind::PlacementPromptPanel { context, .. } => {
                        InspectorPanelKind::PlacementPromptPanel {
                            context: *context,
                            target: None,
                        }
                    }
                    other => other.clone(),
                })
                .collect(),
        )
    }
}
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum SectionKind {
    SavedSounds,
    SoundShaping,
    Reuse,
}
impl SectionKind {
    fn default_expanded(self) -> bool {
        matches!(self, Self::SoundShaping)
    }
    fn title(self) -> &'static str {
        match self {
            Self::SavedSounds => "Saved sounds",
            Self::SoundShaping => "Sound shaping",
            Self::Reuse => "Save reusable tile",
        }
    }
}
#[derive(Clone, PartialEq, Eq)]
struct SectionKey {
    node: NodeId,
    kind: SectionKind,
}
#[derive(Resource, Default)]
struct Memory {
    scrolls: VecDeque<(ViewKey, f32)>,
    sections: VecDeque<(SectionKey, bool)>,
}
fn remember<K: PartialEq, V>(entries: &mut VecDeque<(K, V)>, key: K, value: V) {
    if let Some(index) = entries.iter().position(|(old, _)| old == &key) {
        entries.remove(index);
    }
    if entries.len() == CAPACITY {
        entries.pop_front();
    }
    entries.push_back((key, value));
}
impl Memory {
    fn position(&self, key: &ViewKey) -> f32 {
        self.scrolls
            .iter()
            .find(|(old, _)| old == key)
            .map_or(0.0, |(_, y)| *y)
    }
    fn expanded(&self, key: &SectionKey) -> bool {
        self.sections
            .iter()
            .find(|(old, _)| old == key)
            .map_or(key.kind.default_expanded(), |(_, open)| *open)
    }
}
#[derive(Component)]
pub(super) struct PanelView {
    key: ViewKey,
    initialized: bool,
}
pub(super) fn panel(layout: &InspectorLayout) -> PanelView {
    PanelView {
        key: ViewKey::new(layout),
        initialized: false,
    }
}
#[derive(Component)]
struct SectionHeader {
    key: SectionKey,
    body: Entity,
    expanded: bool,
    initialized: bool,
}

pub(in crate::infrastructure::ui) fn plugin(app: &mut App) {
    app.init_resource::<Memory>()
        .add_systems(OnExit(AppState::Editor), |mut memory: ResMut<Memory>| {
            *memory = Memory::default()
        })
        .add_systems(
            Update,
            (reset, capture)
                .chain()
                .after(MusaicSet::Commands)
                .before(MusaicSet::RenderUi)
                .run_if(in_state(AppState::Editor)),
        )
        .add_systems(
            PostUpdate,
            (initialize, initialize_sections)
                .before(bevy::ui::UiSystems::Layout)
                .run_if(in_state(AppState::Editor)),
        )
        .add_systems(
            PostUpdate,
            clamp_scroll
                .after(bevy::ui::UiSystems::Layout)
                .before(crate::infrastructure::ui::keyboard_regions::reveal_focus)
                .run_if(in_state(AppState::Editor)),
        );
}
fn reset(
    mut changes: MessageReader<ActiveSurfaceChanged>,
    mut memory: ResMut<Memory>,
    mut panels: Query<&mut PanelView>,
    mut sections: Query<&mut SectionHeader>,
) {
    if changes
        .read()
        .any(|event| event.reason == ActiveSurfaceChangeReason::OpenDocument)
    {
        *memory = Memory::default();
        // Outgoing controls can survive until the end of this update's deferred work.
        for mut panel in &mut panels {
            panel.initialized = false;
        }
        for mut section in &mut sections {
            section.initialized = false;
        }
    }
}
fn capture(
    panels: Query<
        (&PanelView, &ScrollPosition, Option<&InspectorSlideLayer>),
        Changed<ScrollPosition>,
    >,
    mut memory: ResMut<Memory>,
) {
    for (panel, position, slide) in &panels {
        if !panel.initialized || slide.is_some_and(|s| s.direction == SlideDirection::Exit) {
            continue;
        }
        if position.y.is_finite() {
            remember(&mut memory.scrolls, panel.key.clone(), position.y.max(0.0));
        }
    }
}
fn initialize(mut panels: Query<(&mut PanelView, &mut ScrollPosition)>, memory: Res<Memory>) {
    for (mut panel, mut position) in &mut panels {
        if !panel.initialized {
            position.y = memory.position(&panel.key);
            panel.initialized = true;
        }
    }
}
fn clamp_scroll(mut panels: Query<(&ComputedNode, &mut ScrollPosition), With<PanelView>>) {
    for (node, mut position) in &mut panels {
        if node.size().y <= 0.0 {
            continue;
        }
        let max = ((node.content_size().y - node.size().y) * node.inverse_scale_factor()).max(0.0);
        let next = if position.y.is_finite() {
            position.y.clamp(0.0, max)
        } else {
            0.0
        };
        if position.y != next {
            position.y = next;
        }
    }
}

pub(super) fn section(
    parent: &mut ChildSpawnerCommands<'_>,
    node: &NodeId,
    kind: SectionKind,
    build: impl FnOnce(&mut ChildSpawnerCommands<'_>),
) {
    let theme = MusaicUiTheme::default();
    let header = parent
        .spawn(musaic_chrome_button(
            &theme,
            format!(
                "{} · {}",
                kind.title(),
                if kind.default_expanded() {
                    "Hide"
                } else {
                    "Show"
                }
            ),
            (),
        ))
        .id();
    let body = parent
        .spawn(Node {
            display: if kind.default_expanded() {
                Display::Flex
            } else {
                Display::None
            },
            flex_direction: FlexDirection::Column,
            row_gap: px(8),
            width: percent(100),
            flex_shrink: 0.0,
            ..default()
        })
        .with_children(build)
        .id();
    parent
        .commands()
        .entity(header)
        .insert(SectionHeader {
            key: SectionKey {
                node: node.clone(),
                kind,
            },
            body,
            expanded: false,
            initialized: false,
        })
        .observe(toggle);
}
fn apply_section(
    header: &mut SectionHeader,
    label: &mut ButtonLabel,
    accessible: &mut AccessibilityNode,
    body: &mut Node,
    expanded: bool,
) {
    header.expanded = expanded;
    body.display = if expanded {
        Display::Flex
    } else {
        Display::None
    };
    label.0 = format!(
        "{} · {}",
        header.key.kind.title(),
        if expanded { "Hide" } else { "Show" }
    );
    accessible.set_expanded(expanded);
}
fn initialize_sections(
    mut headers: Query<(&mut SectionHeader, &mut ButtonLabel, &mut AccessibilityNode)>,
    mut bodies: Query<&mut Node>,
    memory: Res<Memory>,
) {
    for (mut header, mut label, mut accessible) in &mut headers {
        if header.initialized {
            continue;
        }
        header.initialized = true;
        let expanded = memory.expanded(&header.key);
        if let Ok(mut body) = bodies.get_mut(header.body) {
            apply_section(
                &mut header,
                &mut label,
                &mut accessible,
                &mut body,
                expanded,
            );
        }
    }
}
fn toggle(
    event: On<Activate>,
    mut headers: Query<(&mut SectionHeader, &mut ButtonLabel, &mut AccessibilityNode)>,
    mut bodies: Query<&mut Node>,
    mut memory: ResMut<Memory>,
    mut focus: ResMut<InputFocus>,
) {
    let Ok((mut header, mut label, mut accessible)) = headers.get_mut(event.entity) else {
        return;
    };
    let Ok(mut body) = bodies.get_mut(header.body) else {
        return;
    };
    let expanded = !header.expanded;
    apply_section(
        &mut header,
        &mut label,
        &mut accessible,
        &mut body,
        expanded,
    );
    remember(&mut memory.sections, header.key.clone(), expanded);
    // Keep the initiating button visible when collapsing a long section.
    focus.set(event.entity);
}

#[cfg(test)]
mod tests {
    use super::*;
    fn layout(name: &str) -> InspectorLayout {
        InspectorLayout::single(InspectorPanelKind::TileInspectPanel {
            node: NodeId::new(name),
        })
    }
    fn app() -> App {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .init_resource::<Memory>()
            .init_resource::<InputFocus>()
            .add_message::<ActiveSurfaceChanged>()
            .add_systems(
                Update,
                (
                    reset,
                    capture,
                    initialize,
                    initialize_sections,
                    clamp_scroll,
                )
                    .chain(),
            );
        app
    }
    fn body(app: &mut App, name: &str, height: f32) -> Entity {
        app.world_mut()
            .spawn((
                panel(&layout(name)),
                ScrollPosition::default(),
                ComputedNode {
                    size: Vec2::new(290.0, 200.0),
                    content_size: Vec2::new(290.0, height),
                    inverse_scale_factor: 1.0,
                    ..default()
                },
            ))
            .id()
    }
    #[test]
    fn returning_to_a_panel_restores_its_scroll_and_shorter_content_clamps() {
        let mut app = app();
        let a = body(&mut app, "a", 800.0);
        app.update();
        app.world_mut().get_mut::<ScrollPosition>(a).unwrap().y = 380.0;
        app.update();
        app.world_mut().despawn(a);
        let b = body(&mut app, "b", 800.0);
        app.update();
        assert_eq!(app.world().get::<ScrollPosition>(b).unwrap().y, 0.0);
        app.world_mut().get_mut::<ScrollPosition>(b).unwrap().y = 100.0;
        app.update();
        app.world_mut().despawn(b);
        let a = body(&mut app, "a", 800.0);
        app.update();
        assert_eq!(app.world().get::<ScrollPosition>(a).unwrap().y, 380.0);
        app.world_mut()
            .get_mut::<ComputedNode>(a)
            .unwrap()
            .content_size
            .y = 300.0;
        app.update();
        assert_eq!(app.world().get::<ScrollPosition>(a).unwrap().y, 100.0);
    }
    #[test]
    fn same_id_project_replacement_clears_existing_and_future_panel_memory() {
        let mut app = app();
        let a = body(&mut app, "a", 800.0);
        app.update();
        app.world_mut().get_mut::<ScrollPosition>(a).unwrap().y = 380.0;
        app.update();
        let root = crate::application::session::MusaicProject::new_empty()
            .document
            .root_surface;
        app.world_mut().write_message(ActiveSurfaceChanged {
            previous: root,
            current: root,
            reason: ActiveSurfaceChangeReason::OpenDocument,
        });
        app.update();
        assert_eq!(app.world().get::<ScrollPosition>(a).unwrap().y, 0.0);
        app.world_mut().despawn(a);
        let a = body(&mut app, "a", 800.0);
        app.update();
        assert_eq!(app.world().get::<ScrollPosition>(a).unwrap().y, 0.0);
    }
    fn section_fixture(app: &mut App, name: &str) -> Entity {
        let parent = app.world_mut().spawn_empty().id();
        let mut commands = app.world_mut().commands();
        commands.entity(parent).with_children(|p| {
            section(p, &NodeId::new(name), SectionKind::SavedSounds, |body| {
                body.spawn(crate::infrastructure::ui::widgets::musaic_button(
                    Node::default(),
                    (),
                    "Save sound",
                ));
            })
        });
        app.world_mut().flush();
        parent
    }
    #[test]
    fn section_toggle_retains_controls_and_restores_state_after_respawn() {
        let mut app = app();
        let parent = section_fixture(&mut app, "instrument");
        app.update();
        let (button, body) = app
            .world_mut()
            .query::<(Entity, &SectionHeader)>()
            .iter(app.world())
            .map(|(e, h)| (e, h.body))
            .next()
            .unwrap();
        assert_eq!(
            app.world().get::<Node>(body).unwrap().display,
            Display::None
        );
        app.world_mut().trigger(Activate { entity: button });
        app.update();
        assert_eq!(
            app.world().get::<Node>(body).unwrap().display,
            Display::Flex
        );
        assert_eq!(
            app.world()
                .get::<AccessibilityNode>(button)
                .unwrap()
                .is_expanded(),
            Some(true)
        );
        assert_eq!(app.world().resource::<InputFocus>().0, Some(button));
        app.world_mut().despawn(parent);
        section_fixture(&mut app, "instrument");
        app.update();
        let header = app
            .world_mut()
            .query::<&SectionHeader>()
            .single(app.world())
            .unwrap();
        assert!(header.expanded);
        assert_eq!(
            app.world().get::<Node>(header.body).unwrap().display,
            Display::Flex
        );
    }
    #[test]
    fn presentation_cache_is_bounded_and_refreshes_an_existing_key() {
        let mut memory = Memory::default();
        for i in 0..100 {
            remember(
                &mut memory.scrolls,
                ViewKey::new(&layout(&i.to_string())),
                i as f32,
            );
        }
        assert_eq!(memory.scrolls.len(), CAPACITY);
        assert_eq!(memory.position(&ViewKey::new(&layout("0"))), 0.0);
        remember(&mut memory.scrolls, ViewKey::new(&layout("99")), 12.0);
        assert_eq!(memory.scrolls.len(), CAPACITY);
        assert_eq!(memory.position(&ViewKey::new(&layout("99"))), 12.0);
    }
}
