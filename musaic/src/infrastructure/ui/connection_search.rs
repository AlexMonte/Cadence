//! Keyboard and pointer destination choice share one inspected connection plan.
use super::{
    theme::MusaicUiTheme,
    widgets::{musaic_chrome_button, spawn_dialog_overlay},
};
use crate::{
    application::{
        command::{
            EditorCommand, EditorCommandBus,
            connection::{self, ConnectionPlan, ConnectionReceipt},
            editing::TileEdit,
        },
        editor::{EditorAttention, FocusTarget, SelectionState, tile_inspect_title},
        session::MusaicProject,
    },
    domain::document::{DocumentQueries, PlacementAddress},
    infrastructure::app::{AppState, MusaicSet},
};
use bevy::{
    input::{
        ButtonState,
        keyboard::{Key, KeyboardInput},
    },
    input_focus::{FocusedInput, InputFocus, tab_navigation::TabIndex},
    prelude::*,
};
use bevy_ui_widgets::Activate;
use tessera::prelude::{InputEndpoint, NodeId, OutputEndpoint, SpatialSide};

#[derive(Message)]
pub(super) struct OpenConnectionSearch(pub Option<NodeId>);
#[derive(Component)]
pub(super) struct ConnectionSource(pub NodeId);
#[derive(Clone)]
struct Choice {
    plan: ConnectionPlan,
    label: String,
    route: String,
}
#[derive(Resource, Default)]
struct ConnectionSearch {
    open: bool,
    source: Option<NodeId>,
    source_label: String,
    query: String,
    selected: usize,
    choices: Vec<Choice>,
    error: String,
    pending: Option<u64>,
    next_request: u64,
}
impl ConnectionSearch {
    fn results(&self) -> Vec<usize> {
        self.choices
            .iter()
            .enumerate()
            .filter(|(_, c)| {
                let text = format!("{} {}", c.label, c.route).to_lowercase();
                self.query
                    .split_whitespace()
                    .all(|word| text.contains(&word.to_lowercase()))
            })
            .map(|(index, _)| index)
            .collect()
    }
    fn submit(&mut self, bus: &mut MessageWriter<EditorCommandBus>) {
        if self.pending.is_some() {
            return;
        }
        let Some(index) = self.results().get(self.selected).copied() else {
            return;
        };
        let Some(request) = self.next_request.checked_add(1) else {
            self.error = "Close and restart the editor before connecting more tiles.".into();
            return;
        };
        self.next_request = request;
        self.pending = Some(request);
        self.error.clear();
        bus.write(EditorCommandBus(EditorCommand::EditTiles(
            TileEdit::Connect {
                request,
                plan: self.choices[index].plan.clone(),
            },
        )));
    }
}
#[derive(Component)]
struct Dialog;
#[derive(Component)]
struct Field;
#[derive(Component)]
struct FieldText;
#[derive(Component)]
struct Results;
#[derive(Component)]
struct ResultChoice(usize);
#[derive(Component)]
struct SourceLabel;

pub(super) fn plugin(app: &mut App) {
    app.init_resource::<ConnectionSearch>()
        .add_message::<OpenConnectionSearch>()
        .add_systems(
            OnExit(AppState::Editor),
            |mut form: ResMut<ConnectionSearch>| {
                let next = form.next_request;
                *form = ConnectionSearch {
                    next_request: next,
                    ..default()
                };
            },
        )
        .add_systems(
            Update,
            (receive_open, refresh, receive_receipt, sync)
                .chain()
                .in_set(MusaicSet::RenderUi)
                .after(MusaicSet::Commands)
                .run_if(in_state(AppState::Editor)),
        );
}
pub(super) fn open(
    event: On<Activate>,
    sources: Query<&ConnectionSource>,
    mut requests: MessageWriter<OpenConnectionSearch>,
) {
    requests.write(OpenConnectionSearch(
        sources.get(event.entity).ok().map(|s| s.0.clone()),
    ));
}
pub(super) fn point(
    event: On<Activate>,
    sources: Query<&ConnectionSource>,
    project: Res<MusaicProject>,
    mut focus: ResMut<InputFocus>,
    mut keys: ResMut<ButtonInput<KeyCode>>,
    mut navigation: ResMut<
        crate::application::editor::interaction::keyboard_navigation::KeyboardNavigation,
    >,
    mut bus: MessageWriter<EditorCommandBus>,
) {
    let Ok(source) = sources.get(event.entity) else {
        return;
    };
    if !project
        .document
        .graph
        .location_of(&source.0)
        .is_some_and(|place| place.surface == project.document.root_surface)
        || crate::domain::document::export_document_program(&project.document)
            .ok()
            .map(|program| {
                crate::domain::document::connection_policy::effective_bindings(&program, &source.0)
                    .outputs
                    .is_empty()
            })
            .unwrap_or(true)
    {
        navigation.status = "Choose a pattern-board tile with an output to draw a cable.".into();
        return;
    }
    keys.clear_just_pressed(KeyCode::Enter);
    keys.clear_just_pressed(KeyCode::Space);
    focus.clear();
    navigation.status = "Click a compatible destination · Escape cancels the cable".into();
    bus.write(EditorCommandBus(EditorCommand::StartConnection {
        source: source.0.clone(),
    }));
}

fn receive_open(
    mut requests: MessageReader<OpenConnectionSearch>,
    attention: Res<EditorAttention>,
    selection: Res<SelectionState>,
    mut form: ResMut<ConnectionSearch>,
    mut bus: MessageWriter<EditorCommandBus>,
) {
    let Some(request) = requests.read().last() else {
        return;
    };
    if form.pending.is_some() {
        return;
    }
    // Choosing a different tool ends any unfinished placement/connection through
    // its existing owner; no hidden armed tile survives dismissal of this form.
    bus.write(EditorCommandBus(EditorCommand::CancelPlacement));
    let source = request.0.clone().or_else(|| match &attention.focus {
        FocusTarget::Tile { node } | FocusTarget::Atom { node } => Some(node.clone()),
        _ => selection.anchor.clone(),
    });
    *form = ConnectionSearch {
        open: true,
        source,
        next_request: form.next_request,
        ..default()
    };
}
fn tile_label(project: &MusaicProject, node: &NodeId) -> String {
    let title = tile_inspect_title(&DocumentQueries::new(&project.document), node);
    let location = project
        .document
        .graph
        .location_of(node)
        .map(|l| match l.address {
            PlacementAddress::BoardSlot(slot) => format!("({}, {})", slot.x, slot.y),
            PlacementAddress::StackIndex(index) => format!("inside pattern · {}", index.0 + 1),
        })
        .unwrap_or_else(|| "removed".into());
    format!("{title} · {location}")
}
fn side(side: SpatialSide) -> &'static str {
    match side {
        SpatialSide::North => "north",
        SpatialSide::South => "south",
        SpatialSide::East => "east",
        SpatialSide::West => "west",
        _ => "off",
    }
}
fn input(port: &InputEndpoint) -> String {
    match port {
        InputEndpoint::Socket(p) => p.0.clone(),
        InputEndpoint::GroupMember { group, member } => format!("{} / {}", group.0, member.0),
    }
}
fn output(port: &OutputEndpoint) -> String {
    match port {
        OutputEndpoint::Socket(p) => p.0.clone(),
        OutputEndpoint::GroupMember { group, member } => format!("{} / {}", group.0, member.0),
    }
}
fn refresh(
    project: Res<MusaicProject>,
    mut form: ResMut<ConnectionSearch>,
    mut cache: Local<Option<(NodeId, u64)>>,
) {
    if !form.open {
        *cache = None;
        return;
    }
    let Some(source) = form.source.clone() else {
        if form.error.is_empty() {
            form.error = "Select a tile on the pattern board, then choose Connect to tile.".into();
        }
        return;
    };
    let key = (source.clone(), project.document.revision.0);
    if cache.as_ref() == Some(&key) && !project.is_changed() {
        return;
    }
    let previous = form
        .results()
        .get(form.selected)
        .map(|i| form.choices[*i].plan.to.clone());
    form.source_label = tile_label(&project, &source);
    form.choices = connection::destinations(&project.document, &source)
        .into_iter()
        .map(|plan| {
            let route = format!(
                "{} · {} output · {} → {} input · {} · {}",
                if plan.edge.explicit {
                    "Cable"
                } else {
                    "Adjacent"
                },
                output(&plan.edge.output),
                side(plan.edge.side),
                input(&plan.edge.input),
                side(plan.edge.side.opposite()),
                match crate::domain::document::export_document_program(&project.document)
                    .ok()
                    .and_then(|program| tessera::prelude::TesseraCompiler::new()
                        .authored_output_shape(&program, &plan.from, &plan.edge.output))
                {
                    Some(tessera::prelude::StreamShape::NotePattern) => "Notes",
                    Some(tessera::prelude::StreamShape::ScalarPattern) => "Numbers",
                    Some(tessera::prelude::StreamShape::ControlPattern) => "Control values",
                    Some(tessera::prelude::StreamShape::EventPattern) => "Events",
                    _ => "Pattern",
                }
            );
            Choice {
                label: tile_label(&project, &plan.to),
                route,
                plan,
            }
        })
        .collect();
    form.choices.sort_by(|a, b| a.label.cmp(&b.label));
    form.selected = form
        .results()
        .iter()
        .position(|i| Some(&form.choices[*i].plan.to) == previous.as_ref())
        .unwrap_or(0);
    *cache = Some(key);
}
fn receive_receipt(
    mut receipts: MessageReader<ConnectionReceipt>,
    mut form: ResMut<ConnectionSearch>,
    mut navigation: ResMut<
        crate::application::editor::interaction::keyboard_navigation::KeyboardNavigation,
    >,
) {
    for receipt in receipts.read() {
        if form.pending != Some(receipt.request) {
            continue;
        }
        form.pending = None;
        match &receipt.result {
            Ok(()) => {
                form.open = false;
                navigation.status = "Connected tiles · Undo restores the previous ports".into();
            }
            Err(error) => form.error.clone_from(error),
        }
    }
}
fn close(_: On<Activate>, mut form: ResMut<ConnectionSearch>) {
    if form.pending.is_none() {
        form.open = false;
    }
}
fn submit(
    _: On<Activate>,
    mut form: ResMut<ConnectionSearch>,
    mut bus: MessageWriter<EditorCommandBus>,
) {
    form.submit(&mut bus);
}
fn choose(
    event: On<Activate>,
    choices: Query<&ResultChoice>,
    fields: Query<Entity, With<Field>>,
    mut form: ResMut<ConnectionSearch>,
    mut focus: ResMut<InputFocus>,
) {
    if form.pending.is_some() {
        return;
    }
    if let Ok(choice) = choices.get(event.entity) {
        form.selected = choice.0;
        form.error.clear();
        if let Ok(field) = fields.single() {
            focus.set(field);
        }
    }
}
fn focus_field(mut event: On<Pointer<Press>>, mut focus: ResMut<InputFocus>) {
    focus.set(event.entity);
    event.propagate(false);
}
fn type_query(
    mut event: On<FocusedInput<KeyboardInput>>,
    mut keys: ResMut<ButtonInput<KeyCode>>,
    mut form: ResMut<ConnectionSearch>,
    mut bus: MessageWriter<EditorCommandBus>,
) {
    if event.input.state != ButtonState::Pressed || event.input.key_code == KeyCode::Tab {
        return;
    }
    let key = event.input.key_code;
    event.propagate(false);
    keys.clear_just_pressed(key);
    if form.pending.is_some() {
        return;
    }
    let control = [
        KeyCode::SuperLeft,
        KeyCode::SuperRight,
        KeyCode::ControlLeft,
        KeyCode::ControlRight,
    ]
    .iter()
    .any(|k| keys.pressed(*k));
    match key {
        KeyCode::Escape => form.open = false,
        KeyCode::Enter if !event.input.repeat => form.submit(&mut bus),
        KeyCode::ArrowDown => {
            form.selected = (form.selected + 1).min(form.results().len().saturating_sub(1));
            form.error.clear();
        }
        KeyCode::ArrowUp => {
            form.selected = form.selected.saturating_sub(1);
            form.error.clear();
        }
        KeyCode::Backspace | KeyCode::Delete => {
            if control {
                form.query.clear();
            } else {
                form.query.pop();
            }
            form.selected = 0;
            form.error.clear();
        }
        _ if !control => {
            let text = event
                .input
                .text
                .as_deref()
                .or_else(|| match &event.input.logical_key {
                    Key::Character(text) => Some(text.as_str()),
                    _ => None,
                });
            if let Some(text) = text {
                for c in text.chars().filter(|c| !c.is_control()) {
                    if form.query.len() + c.len_utf8() <= 96 {
                        form.query.push(c);
                    }
                }
                form.selected = 0;
                form.error.clear();
            }
        }
        _ => {}
    }
}
fn spawn_results(
    parent: &mut ChildSpawnerCommands<'_>,
    form: &ConnectionSearch,
    theme: &MusaicUiTheme,
) {
    let results = form.results();
    let status = if form.pending.is_some() {
        "Connecting…"
    } else if !form.error.is_empty() {
        &form.error
    } else if form.choices.is_empty() {
        "No free compatible destination. Add a compatible tile on the pattern board or free a port. Connected inputs and feedback loops are excluded."
    } else if results.is_empty() {
        "No matching destination · try a shorter search"
    } else {
        "Choose a destination · preview below · Enter connects · Escape cancels"
    };
    parent.spawn((
        Text::new(status),
        TextColor(theme.chrome.text_dim),
        TextFont {
            font_size: 13.0,
            ..default()
        },
    ));
    for (position, index) in results
        .iter()
        .enumerate()
        .skip(form.selected.saturating_sub(4))
        .take(5)
    {
        let choice = &form.choices[*index];
        parent
            .spawn((
                musaic_chrome_button(theme, &choice.label, ResultChoice(position)),
                TabIndex(-1),
            ))
            .insert(BackgroundColor(if position == form.selected {
                theme.chrome.crumb_selected_bg
            } else {
                theme.chrome.panel_bg
            }))
            .observe(choose);
    }
    if let Some(index) = results.get(form.selected) {
        let choice = &form.choices[*index];
        parent
            .spawn((
                Node {
                    width: percent(100),
                    padding: UiRect::all(px(12)),
                    border: UiRect::all(px(1)),
                    ..default()
                },
                BorderColor::all(theme.chrome.accent),
                BackgroundColor(theme.chrome.window_bg),
            ))
            .with_children(|card| {
                card.spawn((
                    Text::new(format!(
                        "Route preview\n{}\n↓\n{}\n{}\nNothing changes until you connect.",
                        form.source_label, choice.label, choice.route
                    )),
                    TextColor(theme.chrome.text_main),
                    TextFont {
                        font_size: 14.0,
                        ..default()
                    },
                ));
            });
    }
}
fn sync(
    mut commands: Commands,
    theme: Res<MusaicUiTheme>,
    form: Res<ConnectionSearch>,
    roots: Query<Entity, With<Dialog>>,
    results: Query<Entity, With<Results>>,
    mut text: ParamSet<(
        Query<&mut Text, With<FieldText>>,
        Query<&mut Text, With<SourceLabel>>,
        Query<&mut bevy::a11y::AccessibilityNode, With<Field>>,
    )>,
) {
    if !form.open {
        for root in &roots {
            commands.entity(root).despawn();
        }
        return;
    }
    if roots.is_empty() {
        let root = spawn_dialog_overlay(
            &mut commands,
            &theme,
            (Dialog, DespawnOnExit(AppState::Editor)),
            |card| {
                card.spawn((
                    Text::new("Connect to tile"),
                    TextColor(theme.chrome.text_main),
                    TextFont {
                        font_size: 22.0,
                        ..default()
                    },
                ));
                card.spawn((
                    SourceLabel,
                    Text::new(format!("From {}", form.source_label)),
                    TextColor(theme.chrome.text_dim),
                ));
                let mut accessible = accesskit::Node::new(accesskit::Role::TextInput);
                accessible.set_label("Find a compatible destination");
                accessible.set_description("Search tile names, positions or ports. Arrows choose; Enter connects; Escape cancels.");
                accessible.set_value(form.query.clone());
                card.spawn((
                    Field,
                    TabIndex(0),
                    bevy::a11y::AccessibilityNode::from(accessible),
                    Node {
                        width: px(470),
                        max_width: percent(100),
                        min_height: px(40),
                        padding: UiRect::all(px(8)),
                        border: UiRect::all(px(1)),
                        ..default()
                    },
                    BorderColor::all(theme.chrome.button_border),
                    Pickable::default(),
                ))
                .observe(focus_field)
                .with_children(|field| {
                    field.spawn((
                        FieldText,
                        Text::new("Search destinations… |"),
                        TextColor(theme.chrome.text_main),
                        TextFont {
                            font_size: 16.0,
                            ..default()
                        },
                        Pickable::IGNORE,
                    ));
                });
                card.spawn((
                    Results,
                    Node {
                        width: percent(100),
                        flex_direction: FlexDirection::Column,
                        row_gap: px(8),
                        ..default()
                    },
                ))
                .with_children(|parent| spawn_results(parent, &form, &theme));
                card.spawn((Node {
                    column_gap: px(8),
                    ..default()
                },))
                    .with_children(|row| {
                        row.spawn(musaic_chrome_button(&theme, "Connect", ()))
                            .observe(submit);
                        row.spawn(musaic_chrome_button(&theme, "Cancel", ()))
                            .observe(close);
                    });
            },
        );
        commands.entity(root).observe(type_query);
    } else if form.is_changed() || theme.is_changed() {
        for mut field in &mut text.p0() {
            field.0 = if form.query.is_empty() {
                "Search destinations… |".into()
            } else {
                format!("{} |", form.query)
            };
        }
        for mut accessible in &mut text.p2() {
            accessible.set_value(form.query.clone());
        }
        for mut source in &mut text.p1() {
            source.0 = format!("From {}", form.source_label);
        }
        for entity in &results {
            commands
                .entity(entity)
                .despawn_children()
                .with_children(|parent| spawn_results(parent, &form, &theme));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn fixture() -> (App, Entity) {
        use crate::domain::board::BoardSlot;
        use crate::domain::document::{
            AtomValue, GraphTilePrototypeId, PlacementAddress, TileSpawnKind,
        };
        use tessera::prelude::{
            NodeId, NodeSpatialBindings, OutputEndpoint, OutputPort, Rational, SpatialSide,
        };
        let mut project = MusaicProject::new_empty();
        let root = project.document.root_surface;
        project
            .document
            .graph
            .insert_tile_at_id(
                &mut project.document.surfaces,
                root,
                PlacementAddress::BoardSlot(BoardSlot::new(0, 0)),
                NodeId::new("gain"),
                TileSpawnKind::TrickInstance {
                    prototype: GraphTilePrototypeId(3),
                },
            )
            .unwrap();
        project
            .document
            .graph
            .insert_tile_at_id(
                &mut project.document.surfaces,
                root,
                PlacementAddress::BoardSlot(BoardSlot::new(-1, 0)),
                NodeId::new("value"),
                TileSpawnKind::Atom {
                    atom: AtomValue::Ratio(Rational::one()),
                },
            )
            .unwrap();
        let mut value_bindings = NodeSpatialBindings::default();
        value_bindings.outputs.insert(
            OutputEndpoint::Socket(OutputPort::new("out")),
            SpatialSide::South,
        );
        project
            .document
            .connections
            .bindings
            .insert(NodeId::new("value"), value_bindings);
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, bevy::input::InputPlugin, bevy::input_focus::InputDispatchPlugin))
            .insert_resource(EditorAttention::new(project.document.root_surface)).insert_resource(project)
            .init_resource::<SelectionState>().init_resource::<ConnectionSearch>()
            .init_resource::<crate::application::editor::interaction::keyboard_navigation::KeyboardNavigation>()
            .add_message::<OpenConnectionSearch>().add_message::<EditorCommandBus>().add_message::<ConnectionReceipt>()
            .add_systems(Update, (receive_open, refresh, receive_receipt).chain());
        app.configure_sets(
            PreUpdate,
            bevy::input_focus::InputFocusSystems::Dispatch.after(bevy::input::InputSystems),
        );
        let window = app.world_mut().spawn(bevy::window::PrimaryWindow).id();
        let field = app.world_mut().spawn_empty().observe(type_query).id();
        app.world_mut().resource_mut::<InputFocus>().set(field);
        (app, window)
    }
    fn key(app: &mut App, window: Entity, code: KeyCode, text: Option<&str>, repeat: bool) {
        app.world_mut().write_message(KeyboardInput {
            key_code: code,
            logical_key: text.map(|s| Key::Character(s.into())).unwrap_or(Key::Enter),
            text: text.map(Into::into),
            repeat,
            state: ButtonState::Pressed,
            window,
        });
        app.update();
        assert!(
            !app.world()
                .resource::<ButtonInput<KeyCode>>()
                .just_pressed(code)
        );
    }
    #[test]
    fn dialog_builds_refreshes_and_cleans_up_its_preview() {
        let (mut app, _) = fixture();
        app.init_resource::<MusaicUiTheme>()
            .add_systems(Update, sync.after(receive_receipt));
        app.world_mut()
            .write_message(OpenConnectionSearch(Some(NodeId::new("value"))));
        app.update();
        assert_eq!(
            app.world_mut()
                .query_filtered::<Entity, With<Dialog>>()
                .iter(app.world())
                .count(),
            1
        );
        assert!(
            app.world_mut()
                .query::<&Text>()
                .iter(app.world())
                .any(|t| t.0.contains("Route preview") && t.0.contains("amount"))
        );
        app.world_mut().resource_mut::<ConnectionSearch>().query = "gain".into();
        app.update();
        assert_eq!(
            app.world_mut()
                .query_filtered::<Entity, With<ResultChoice>>()
                .iter(app.world())
                .count(),
            1
        );
        app.world_mut().resource_mut::<ConnectionSearch>().open = false;
        app.update();
        assert_eq!(
            app.world_mut()
                .query_filtered::<Entity, With<Dialog>>()
                .iter(app.world())
                .count(),
            0
        );
        assert_eq!(
            app.world_mut()
                .query_filtered::<Entity, With<ResultChoice>>()
                .iter(app.world())
                .count(),
            0
        );
    }
    #[test]
    fn keyboard_submission_waits_for_its_receipt_and_keeps_rejected_search() {
        let (mut app, window) = fixture();
        app.world_mut()
            .write_message(OpenConnectionSearch(Some(NodeId::new("value"))));
        app.update();
        assert!(
            app.world().resource::<ConnectionSearch>().choices[0]
                .route
                .contains("Numbers")
        );
        app.world_mut()
            .resource_mut::<Messages<EditorCommandBus>>()
            .clear();
        key(&mut app, window, KeyCode::KeyG, Some("gain"), false);
        key(&mut app, window, KeyCode::ArrowDown, None, false);
        key(&mut app, window, KeyCode::Enter, None, false);
        key(&mut app, window, KeyCode::Enter, None, true);
        let commands: Vec<_> = app
            .world_mut()
            .resource_mut::<Messages<EditorCommandBus>>()
            .drain()
            .collect();
        assert_eq!(commands.len(), 1);
        assert!(
            matches!(&commands[0].0, EditorCommand::EditTiles(TileEdit::Connect {request: 1, plan}) if plan.to == NodeId::new("gain"))
        );
        app.world_mut().write_message(ConnectionReceipt {
            request: 99,
            result: Ok(()),
        });
        app.update();
        assert_eq!(app.world().resource::<ConnectionSearch>().pending, Some(1));
        app.world_mut().write_message(ConnectionReceipt {
            request: 1,
            result: Err("Port is occupied".into()),
        });
        app.update();
        let form = app.world().resource::<ConnectionSearch>();
        assert!(form.open);
        assert_eq!(form.query, "gain");
        assert_eq!(form.error, "Port is occupied");
        key(&mut app, window, KeyCode::Enter, None, false);
        assert_eq!(app.world().resource::<ConnectionSearch>().pending, Some(2));
        app.world_mut().write_message(ConnectionReceipt {
            request: 2,
            result: Ok(()),
        });
        app.update();
        assert!(!app.world().resource::<ConnectionSearch>().open);
    }
    #[test]
    fn filtering_and_cancel_never_emit_an_edit() {
        let (mut app, window) = fixture();
        app.world_mut()
            .write_message(OpenConnectionSearch(Some(NodeId::new("value"))));
        app.update();
        app.world_mut()
            .resource_mut::<Messages<EditorCommandBus>>()
            .clear();
        let before = app.world().resource::<MusaicProject>().document.clone();
        key(&mut app, window, KeyCode::KeyX, Some("no such tile"), false);
        key(&mut app, window, KeyCode::Enter, None, false);
        assert!(
            app.world()
                .resource::<ConnectionSearch>()
                .results()
                .is_empty()
        );
        key(&mut app, window, KeyCode::Escape, None, false);
        assert!(!app.world().resource::<ConnectionSearch>().open);
        assert!(
            app.world()
                .resource::<Messages<EditorCommandBus>>()
                .is_empty()
        );
        assert_eq!(app.world().resource::<MusaicProject>().document, before);
    }
}
