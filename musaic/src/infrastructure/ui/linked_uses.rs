//! Review stable linked tile identities without changing the song.
use super::{
    theme::MusaicUiTheme,
    widgets::{ButtonLabel, musaic_button, musaic_chrome_button, spawn_dialog_overlay},
};
use crate::{
    application::{
        command::{EditorCommand, EditorCommandBus},
        editor::{ActiveSurfaceChangeReason, ActiveSurfaceChanged, SelectionMode},
        session::MusaicProject,
        trick_uses::{LinkedUses, linked_uses},
    },
    infrastructure::app::{AppState, MusaicSet},
};
use bevy::{
    input_focus::{InputFocus, tab_navigation::TabIndex},
    prelude::*,
    ui::InteractionDisabled,
};
use bevy_ui_widgets::Activate;
use tessera::prelude::NodeId;
const PAGE: usize = 5;
#[derive(Message)]
pub(crate) struct OpenUses(pub u64);
#[derive(Message)]
pub(crate) struct VisitCode(pub NodeId);
#[derive(Component)]
pub(crate) struct UsesButton(pub u64);
#[derive(Component)]
pub(crate) struct SourceButton(pub NodeId);
pub(crate) fn open_button(
    event: On<Activate>,
    buttons: Query<&UsesButton>,
    mut messages: MessageWriter<OpenUses>,
) {
    if let Ok(button) = buttons.get(event.entity) {
        messages.write(OpenUses(button.0));
    }
}
pub(crate) fn source_button(
    event: On<Activate>,
    buttons: Query<&SourceButton>,
    mut messages: MessageWriter<VisitCode>,
) {
    if let Ok(button) = buttons.get(event.entity) {
        messages.write(VisitCode(button.0.clone()));
    }
}
#[derive(Resource, Default)]
struct Form {
    report: Option<LinkedUses>,
    page: usize,
    revision: u64,
}
#[derive(Component)]
struct Dialog;
#[derive(Component)]
struct Heading;
#[derive(Component)]
struct Detail;
#[derive(Component)]
struct PageLabel;
#[derive(Component)]
enum Action {
    Source,
    Tile(usize),
    Previous,
    Next,
    Close,
}
pub(super) fn plugin(app: &mut App) {
    app.init_resource::<Form>()
        .add_message::<OpenUses>()
        .add_message::<VisitCode>()
        .add_systems(OnExit(AppState::Editor), |mut form: ResMut<Form>| {
            *form = default()
        })
        .add_systems(
            PreUpdate,
            escape
                .after(bevy::input_focus::InputFocusSystems::Dispatch)
                .run_if(in_state(AppState::Editor)),
        )
        .add_systems(
            Update,
            // Preserve widget ownership until the board input stage has finished.
            visit
                .after(MusaicSet::Input)
                .before(MusaicSet::Commands)
                .run_if(in_state(AppState::Editor)),
        )
        .add_systems(
            Update,
            (refresh, sync)
                .chain()
                .in_set(MusaicSet::RenderUi)
                .run_if(in_state(AppState::Editor)),
        );
}
fn visit(
    mut messages: MessageReader<VisitCode>,
    project: Res<MusaicProject>,
    mut bus: MessageWriter<EditorCommandBus>,
    mut view: MessageWriter<super::camera_rig::BoardViewAction>,
    mut focus: ResMut<InputFocus>,
    mut form: ResMut<Form>,
) {
    let Some(request) = messages.read().last() else {
        return;
    };
    let Some(site) = project.document.graph.location_of(&request.0) else {
        return;
    };
    bus.write(EditorCommandBus(EditorCommand::NavigateToSurface {
        surface: site.surface,
    }));
    bus.write(EditorCommandBus(EditorCommand::SelectNode {
        node: request.0.clone(),
        mode: SelectionMode::Replace,
    }));
    view.write(super::camera_rig::BoardViewAction::FitSelection);
    focus.clear();
    form.report = None;
}
fn refresh(
    mut requests: MessageReader<OpenUses>,
    mut surfaces: MessageReader<ActiveSurfaceChanged>,
    project: Res<MusaicProject>,
    mut form: ResMut<Form>,
) {
    if surfaces
        .read()
        .any(|s| s.reason == ActiveSurfaceChangeReason::OpenDocument)
    {
        *form = default();
        requests.clear();
        return;
    }
    if let Some(request) = requests.read().last() {
        form.report = linked_uses(&project.document, request.0);
        form.page = 0;
        form.revision = project.document.revision.0;
    } else if form.report.is_some() && form.revision != project.document.revision.0 {
        form.report = linked_uses(&project.document, form.report.as_ref().unwrap().id);
        form.revision = project.document.revision.0;
        form.page = form.page.min(pages(&form) - 1);
    }
}
fn pages(form: &Form) -> usize {
    form.report
        .as_ref()
        .map_or(1, |r| r.tiles.len().div_ceil(PAGE).max(1))
}
fn escape(mut keys: ResMut<ButtonInput<KeyCode>>, mut form: ResMut<Form>) {
    if form.report.is_some() && keys.just_pressed(KeyCode::Escape) {
        keys.clear_just_pressed(KeyCode::Escape);
        form.report = None;
    }
}
fn activate(
    event: On<Activate>,
    actions: Query<&Action>,
    mut form: ResMut<Form>,
    mut visits: MessageWriter<VisitCode>,
) {
    let Some(report) = &form.report else {
        return;
    };
    match actions.get(event.entity) {
        Ok(Action::Close) => form.report = None,
        Ok(Action::Previous) => form.page = form.page.saturating_sub(1),
        Ok(Action::Next) => form.page = (form.page + 1).min(pages(&form) - 1),
        Ok(Action::Source) => {
            visits.write(VisitCode(report.source.clone()));
        }
        Ok(Action::Tile(slot)) => {
            if let Some(tile) = report.tiles.get(form.page * PAGE + slot) {
                visits.write(VisitCode(tile.node.clone()));
            }
        }
        _ => {}
    }
}
fn disabled(action: &Action, form: &Form) -> bool {
    match action {
        Action::Previous => form.page == 0,
        Action::Next => form.page + 1 >= pages(form),
        Action::Tile(slot) => form
            .report
            .as_ref()
            .is_none_or(|r| r.tiles.get(form.page * PAGE + slot).is_none()),
        _ => false,
    }
}
fn text(
    parent: &mut ChildSpawnerCommands<'_>,
    value: impl Into<String>,
    size: f32,
    marker: impl Bundle,
    theme: &MusaicUiTheme,
) {
    parent.spawn((
        marker,
        Text::new(value),
        TextFont {
            font_size: size,
            ..default()
        },
        TextColor(theme.chrome.text_main),
        Node {
            max_width: px(620),
            ..default()
        },
    ));
}
fn sync(
    mut commands: Commands,
    theme: Res<MusaicUiTheme>,
    form: Res<Form>,
    roots: Query<Entity, With<Dialog>>,
    mut texts: Query<
        (&mut Text, Has<Heading>, Has<PageLabel>),
        Or<(With<Heading>, With<Detail>, With<PageLabel>)>,
    >,
    mut buttons: Query<(Entity, &Action, &mut ButtonLabel, Has<InteractionDisabled>)>,
) {
    let Some(report) = &form.report else {
        for root in &roots {
            commands.entity(root).despawn();
        }
        return;
    };
    if !form.is_changed() && !roots.is_empty() {
        return;
    }
    let title = format!("Linked uses · {}", report.name);
    let detail = format!(
        "{} linked tiles in this project. Every listed tile follows this source. Duplicating a linked tile keeps that link.\n\nSource: {}\nThese are authored references, including nested patterns; repeats do not create extra tiles.",
        report.tiles.len(),
        report.source_location
    );
    let page = format!(
        "Page {} of {} · choose a tile to locate it",
        form.page + 1,
        pages(&form)
    );
    if roots.is_empty() {
        spawn_dialog_overlay(
            &mut commands,
            &theme,
            (
                Dialog,
                Name::new("Linked uses"),
                DespawnOnExit(AppState::Editor),
            ),
            |card| {
                text(card, &title, 22., Heading, &theme);
                text(card, &detail, 14., Detail, &theme);
                text(card, &page, 13., PageLabel, &theme);
                for slot in 0..PAGE {
                    let action = Action::Tile(slot);
                    let label = report
                        .tiles
                        .get(form.page * PAGE + slot)
                        .map_or("No linked tiles".into(), |t| format!("Show {}", t.location));
                    let mut button = card.spawn((
                        musaic_button(
                            Node {
                                width: px(620),
                                max_width: percent(100),
                                min_height: px(34),
                                padding: UiRect::all(px(7)),
                                ..default()
                            },
                            (),
                            label,
                        ),
                        action,
                    ));
                    if slot >= report.tiles.len() {
                        button.insert((InteractionDisabled, TabIndex(-1)));
                        if slot > 0 {
                            button.insert(Node {
                                display: Display::None,
                                ..default()
                            });
                        }
                    }
                    button.observe(activate);
                }
                card.spawn(Node {
                    column_gap: px(8),
                    row_gap: px(8),
                    flex_wrap: FlexWrap::Wrap,
                    max_width: px(620),
                    ..default()
                })
                .with_children(|row| {
                    for (label, action) in [
                        ("Edit source tiles", Action::Source),
                        ("Previous", Action::Previous),
                        ("Next", Action::Next),
                        ("Close", Action::Close),
                    ] {
                        let off = disabled(&action, &form);
                        let mut button =
                            row.spawn((musaic_chrome_button(&theme, label, ()), action));
                        if off {
                            button.insert((
                                InteractionDisabled,
                                TabIndex(-1),
                                bevy_feathers::theme::ThemeFontColor(
                                    bevy_feathers::tokens::BUTTON_TEXT_DISABLED,
                                ),
                            ));
                        }
                        button.observe(activate);
                    }
                });
            },
        );
        return;
    }
    for (mut text, heading, paging) in &mut texts {
        let value = if heading {
            &title
        } else if paging {
            &page
        } else {
            &detail
        };
        if text.0 != *value {
            text.0.clone_from(value);
        }
    }
    for (entity, action, mut label, was_disabled) in &mut buttons {
        if let Action::Tile(slot) = action {
            let value = report
                .tiles
                .get(form.page * PAGE + slot)
                .map_or("No linked tiles".into(), |t| format!("Show {}", t.location));
            if label.0 != value {
                label.0 = value;
            }
            commands.entity(entity).insert(Node {
                width: px(620),
                max_width: percent(100),
                min_height: px(34),
                padding: UiRect::all(px(7)),
                display: if disabled(action, &form) && *slot > 0 {
                    Display::None
                } else {
                    Display::Flex
                },
                ..default()
            });
        }
        let off = disabled(action, &form);
        if off != was_disabled {
            commands
                .entity(entity)
                .insert(bevy_feathers::theme::ThemeFontColor(if off {
                    bevy_feathers::tokens::BUTTON_TEXT_DISABLED
                } else {
                    bevy_feathers::tokens::TEXT_MAIN
                }));
            if off {
                commands
                    .entity(entity)
                    .insert((InteractionDisabled, TabIndex(-1)));
            } else {
                commands
                    .entity(entity)
                    .remove::<InteractionDisabled>()
                    .insert(TabIndex(0));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[derive(Resource, Default)]
    struct ObservedWidgetFocus(bool);
    #[test]
    fn navigation_releases_widget_focus_only_after_board_input_has_finished() {
        let mut app = App::new();
        let starter = crate::application::session::first_loop();
        let target = starter.first_note;
        app.add_plugins((MinimalPlugins, bevy::state::app::StatesPlugin))
            .insert_state(AppState::Editor)
            .insert_resource(starter.project)
            .init_resource::<InputFocus>()
            .init_resource::<ButtonInput<KeyCode>>()
            .init_resource::<MusaicUiTheme>()
            .init_resource::<ObservedWidgetFocus>()
            .add_message::<EditorCommandBus>()
            .add_message::<super::super::camera_rig::BoardViewAction>()
            .add_message::<ActiveSurfaceChanged>()
            .configure_sets(
                Update,
                (MusaicSet::Input, MusaicSet::Commands, MusaicSet::RenderUi).chain(),
            )
            .add_plugins(plugin)
            .add_systems(
                Update,
                (|focus: Res<InputFocus>, mut seen: ResMut<ObservedWidgetFocus>| {
                    seen.0 = focus.0.is_some()
                })
                .in_set(MusaicSet::Input),
            );
        let widget = app.world_mut().spawn_empty().id();
        app.world_mut().resource_mut::<InputFocus>().set(widget);
        app.world_mut().write_message(VisitCode(target));
        app.update();
        assert!(
            app.world().resource::<ObservedWidgetFocus>().0,
            "board input sees the widget owner"
        );
        assert!(app.world().resource::<InputFocus>().0.is_none());
        assert_eq!(
            app.world().resource::<Messages<EditorCommandBus>>().len(),
            2
        );
    }
    #[test]
    fn identical_project_replacement_discards_open_reference_snapshot() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .init_resource::<Form>()
            .insert_resource(MusaicProject::new_empty())
            .add_message::<ActiveSurfaceChanged>()
            .add_message::<OpenUses>()
            .add_systems(Update, refresh);
        app.world_mut().resource_mut::<Form>().report = Some(LinkedUses {
            id: 1000,
            name: "Old".into(),
            source: NodeId::new("same-id"),
            source_location: "old".into(),
            tiles: Vec::new(),
        });
        let surface = app
            .world()
            .resource::<MusaicProject>()
            .document
            .root_surface;
        app.world_mut().write_message(ActiveSurfaceChanged {
            previous: surface,
            current: surface,
            reason: ActiveSurfaceChangeReason::OpenDocument,
        });
        app.update();
        assert!(app.world().resource::<Form>().report.is_none());
    }
    #[test]
    fn source_button_uses_keyboard_activation_and_keeps_document_untouched() {
        let mut app = App::new();
        app.add_message::<VisitCode>();
        let entity = app
            .world_mut()
            .spawn(SourceButton(NodeId::new("source")))
            .observe(source_button)
            .id();
        app.world_mut().trigger(Activate { entity });
        let messages = app.world().resource::<Messages<VisitCode>>();
        let mut cursor = messages.get_cursor();
        assert_eq!(
            cursor
                .read(messages)
                .map(|m| m.0.clone())
                .collect::<Vec<_>>(),
            [NodeId::new("source")]
        );
    }
}
