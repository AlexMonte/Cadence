//! Optional launch guidance. Music changes still go through the command owner.
use bevy::prelude::*;
use bevy_ui_widgets::Activate;
use tessera::prelude::NodeId;

use super::{theme::MusaicUiTheme, widgets::musaic_chrome_button};
use crate::{
    application::{
        command::{EditorCommand, EditorCommandBus},
        editor::{ActiveSurfaceChangeReason, ActiveSurfaceChanged, SelectionMode},
        pipeline::runtime::{RuntimePreviewSnapshot, feedback::Readiness},
        session::{FirstLoop, MusaicProject},
    },
    domain::document::{AtomValue, DocumentNodeKind, NoteName},
    infrastructure::app::{AppState, MusaicSet},
};

#[derive(Clone)]
struct Targets {
    pattern: NodeId,
    note: NodeId,
    octave: NodeId,
}

#[derive(Default, Clone, Copy, PartialEq, Eq)]
enum Stage {
    #[default]
    Listen,
    Edit,
    Save,
}
impl Stage {
    fn text(self) -> &'static str {
        match self {
            Self::Listen => "First loop · Press Play to hear C4, E4, G4 and a rest.",
            Self::Edit => {
                "Make it yours · Open the first note, then choose a pitch in the inspector."
            }
            Self::Save => "You changed the melody · Use File > Save to keep your loop.",
        }
    }
}
struct Progress {
    targets: Targets,
    stage: Stage,
}

#[derive(Resource, Default)]
pub(super) struct FirstLoopGuide {
    pending: Option<Targets>,
    active: Option<Progress>,
}
impl FirstLoopGuide {
    pub(super) fn request(&mut self, starter: &FirstLoop) {
        self.pending = Some(Targets {
            pattern: starter.pattern.clone(),
            note: starter.first_note.clone(),
            octave: starter.first_octave.clone(),
        });
    }
}

pub(super) fn plugin(app: &mut App) {
    app.init_resource::<FirstLoopGuide>()
        .add_systems(OnExit(AppState::Editor), clear)
        .add_systems(
            Update,
            (advance, paint)
                .chain()
                .after(super::shell::rebuild::rebuild_ui)
                .in_set(MusaicSet::RenderUi)
                .run_if(in_state(AppState::Editor)),
        );
}

fn clear(mut guide: ResMut<FirstLoopGuide>) {
    *guide = FirstLoopGuide::default();
}

fn advance(
    mut guide: ResMut<FirstLoopGuide>,
    mut surfaces: MessageReader<ActiveSurfaceChanged>,
    project: Res<MusaicProject>,
    snapshot: Res<RuntimePreviewSnapshot>,
) {
    // The owner emits OpenDocument only after accepting a replacement. Node IDs
    // alone cannot identify a project: opening another file may reuse every ID.
    // A launch request expires this frame even if adoption was rejected.
    let mut pending = guide.pending.take();
    for event in surfaces.read() {
        if event.reason == ActiveSurfaceChangeReason::OpenDocument {
            guide.active = pending.take().map(|targets| Progress {
                targets,
                stage: Stage::Listen,
            });
        }
    }
    let Some(active) = guide.active.as_mut() else {
        return;
    };
    let targets = &active.targets;
    let graph = &project.document.graph;
    let (Some(note), Some(octave)) = (graph.node(&targets.note), graph.node(&targets.octave))
    else {
        guide.active = None;
        return;
    };
    if !graph.contains_node(&targets.pattern) {
        guide.active = None;
        return;
    }
    if snapshot.feedback.readiness != Readiness::Accepted {
        return;
    }
    if snapshot.feedback.playing && active.stage == Stage::Listen {
        active.stage = Stage::Edit;
    }
    let pitch_changed = matches!(&note.kind, DocumentNodeKind::Atom(atom)
        if matches!(atom.atom, AtomValue::NoteName(name) if name != NoteName::C))
        || matches!(&octave.kind, DocumentNodeKind::Atom(atom)
            if matches!(atom.atom, AtomValue::Octave(value) if value != 4));
    if active.stage == Stage::Edit && pitch_changed {
        active.stage = Stage::Save;
    }
}

#[derive(Component)]
struct GuideRow;
#[derive(Component)]
struct GuideText;
#[derive(Component, Clone, Copy)]
enum Action {
    OpenNote,
    Dismiss,
}

pub(super) fn spawn(parent: &mut ChildSpawnerCommands<'_>, theme: &MusaicUiTheme) {
    parent
        .spawn((
            GuideRow,
            Node {
                display: Display::None,
                flex_shrink: 0.0,
                align_items: AlignItems::Center,
                flex_wrap: FlexWrap::Wrap,
                column_gap: px(12),
                row_gap: px(8),
                padding: UiRect::axes(px(22), px(10)),
                border: UiRect::bottom(px(1)),
                ..default()
            },
            BackgroundColor(theme.chrome.window_bg),
            BorderColor::all(theme.chrome.border),
        ))
        .with_children(|row| {
            let mut accessible = accesskit::Node::new(accesskit::Role::Note);
            accessible.set_live(accesskit::Live::Polite);
            accessible.set_live_atomic();
            row.spawn((
                GuideText,
                Text::new(""),
                TextFont {
                    font_size: 13.0,
                    ..default()
                },
                TextColor(theme.chrome.text_main),
                bevy::a11y::AccessibilityNode(accessible),
                Node {
                    flex_grow: 1.0,
                    ..default()
                },
            ));
            for (label, action) in [
                ("Open first note", Action::OpenNote),
                ("Dismiss guide", Action::Dismiss),
            ] {
                row.spawn(musaic_chrome_button(theme, label, action))
                    .observe(activate);
            }
        });
}

fn paint(
    guide: Res<FirstLoopGuide>,
    mut rows: Query<&mut Node, With<GuideRow>>,
    mut labels: Query<(&mut Text, &mut bevy::a11y::AccessibilityNode), With<GuideText>>,
) {
    let display = if guide.active.is_some() {
        Display::Flex
    } else {
        Display::None
    };
    for mut row in &mut rows {
        if row.display != display {
            row.display = display;
        }
    }
    let label = guide
        .active
        .as_ref()
        .map(|active| active.stage.text())
        .unwrap_or_default();
    for (mut text, mut accessible) in &mut labels {
        if text.0 != label {
            text.0 = label.into();
            accessible.set_label(label);
        }
    }
}

fn activate(
    event: On<Activate>,
    actions: Query<&Action>,
    mut guide: ResMut<FirstLoopGuide>,
    mut bus: MessageWriter<EditorCommandBus>,
) {
    let Ok(action) = actions.get(event.entity) else {
        return;
    };
    let Some(active) = &guide.active else {
        return;
    };
    match action {
        Action::OpenNote => {
            bus.write(EditorCommandBus(EditorCommand::EnterContainer {
                container: active.targets.pattern.clone(),
            }));
            bus.write(EditorCommandBus(EditorCommand::SelectNode {
                node: active.targets.note.clone(),
                mode: SelectionMode::Replace,
            }));
        }
        Action::Dismiss => guide.active = None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture() -> App {
        let starter = crate::application::session::first_loop();
        let mut guide = FirstLoopGuide::default();
        guide.request(&starter);
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .insert_resource(guide)
            .insert_resource(starter.project)
            .init_resource::<RuntimePreviewSnapshot>()
            .add_message::<ActiveSurfaceChanged>()
            .add_systems(Update, advance);
        app
    }
    fn replace(app: &mut App) {
        let root = app
            .world()
            .resource::<MusaicProject>()
            .document
            .root_surface;
        app.world_mut().write_message(ActiveSurfaceChanged {
            previous: root,
            current: root,
            reason: ActiveSurfaceChangeReason::OpenDocument,
        });
    }
    fn stage(app: &App) -> Option<Stage> {
        app.world()
            .resource::<FirstLoopGuide>()
            .active
            .as_ref()
            .map(|active| active.stage)
    }
    #[test]
    fn guide_waits_for_accepted_playback_and_edit_then_clears_on_same_id_replacement() {
        let mut app = fixture();
        replace(&mut app);
        app.update();
        assert!(stage(&app) == Some(Stage::Listen));
        app.world_mut()
            .resource_mut::<RuntimePreviewSnapshot>()
            .feedback
            .playing = true;
        app.update();
        assert!(
            stage(&app) == Some(Stage::Listen),
            "checking is not successful playback"
        );
        app.world_mut()
            .resource_mut::<RuntimePreviewSnapshot>()
            .feedback
            .readiness = Readiness::Accepted;
        app.update();
        assert!(stage(&app) == Some(Stage::Edit));
        let note = app
            .world()
            .resource::<FirstLoopGuide>()
            .active
            .as_ref()
            .unwrap()
            .targets
            .note
            .clone();
        app.world_mut()
            .resource_mut::<MusaicProject>()
            .document
            .graph
            .set_atom_value(&note, AtomValue::NoteName(NoteName::D))
            .unwrap();
        app.world_mut()
            .resource_mut::<RuntimePreviewSnapshot>()
            .feedback
            .readiness = Readiness::Rejected;
        app.update();
        assert!(
            stage(&app) == Some(Stage::Edit),
            "rejected edits are not success"
        );
        app.world_mut()
            .resource_mut::<RuntimePreviewSnapshot>()
            .feedback
            .readiness = Readiness::Accepted;
        app.update();
        assert!(stage(&app) == Some(Stage::Save));
        replace(&mut app);
        app.update();
        assert!(
            stage(&app).is_none(),
            "even identical node IDs belong to a new session"
        );
    }
    #[test]
    fn unaccepted_launch_cannot_attach_to_a_later_project() {
        let mut app = fixture();
        app.update();
        replace(&mut app);
        app.update();
        assert!(stage(&app).is_none());
    }
}
