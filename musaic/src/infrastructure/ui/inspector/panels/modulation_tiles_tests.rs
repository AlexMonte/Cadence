//! Numeric, symbolic, and seed controls use the real editor command/history path.
use super::*;
use crate::{
    adapter::persistence::{export_project_bytes, import_project_bytes},
    application::{editor::EditorPlugin, history::CommandHistory, pipeline::PlaybackPlugin},
    domain::{
        board::BoardSlot,
        document::{ContainerKind, NoteName, PlacementAddress, StackIndex, TileSpawnKind},
    },
    infrastructure::app::{AppState, TransportMode},
};
use bevy::input::{
    ButtonState,
    keyboard::{Key, KeyboardInput},
};

fn fixture() -> (App, NodeId, NodeId) {
    let mut app = App::new();
    app.add_plugins(bevy::state::app::StatesPlugin)
        .init_state::<AppState>()
        .init_state::<TransportMode>()
        .add_plugins((
            MinimalPlugins,
            bevy::input::InputPlugin,
            bevy::input_focus::InputDispatchPlugin,
        ))
        .add_plugins((EditorPlugin, PlaybackPlugin));
    app.configure_sets(
        PreUpdate,
        bevy::input_focus::InputFocusSystems::Dispatch.after(bevy::input::InputSystems),
    );
    app.insert_state(AppState::Editor).add_systems(Last, sync);
    let mut project = MusaicProject::new_empty();
    let root = project.document.root_surface;
    let container = project
        .document
        .graph
        .insert_tile(
            &mut project.document.surfaces,
            root,
            PlacementAddress::BoardSlot(BoardSlot::new(0, 0)),
            TileSpawnKind::Container {
                kind: ContainerKind::Sequence,
            },
        )
        .unwrap();
    let surface = project
        .document
        .graph
        .container_surface(&container)
        .unwrap();
    let values = [
        AtomValue::NoteName(NoteName::C),
        AtomValue::Modifier(AtomModifier::Gain(Rational::new(1, 2))),
        AtomValue::Modifier(AtomModifier::Modulation {
            parameter: ParameterKey::Gain,
            value: ModulationParameters::default(),
        }),
    ];
    let nodes: Vec<_> = values
        .into_iter()
        .enumerate()
        .map(|(index, atom)| {
            project
                .document
                .graph
                .insert_tile(
                    &mut project.document.surfaces,
                    surface,
                    PlacementAddress::StackIndex(StackIndex(index)),
                    TileSpawnKind::Atom { atom },
                )
                .unwrap()
        })
        .collect();
    let node = nodes[2].clone();
    let (_, value) = current(&project, &node).unwrap();
    app.insert_resource(project);
    app.world_mut()
        .commands()
        .spawn(Node::default())
        .with_children(|parent| {
            spawn(
                parent,
                &node,
                &AtomModifier::Modulation {
                    parameter: ParameterKey::Gain,
                    value,
                },
            )
        });
    app.world_mut().flush();
    app.update();
    (app, node, nodes[1].clone())
}
fn read(app: &App, node: &NodeId) -> (ParameterKey, ModulationParameters) {
    current(app.world().resource::<MusaicProject>(), node).unwrap()
}
fn send(app: &mut App, command: EditorCommand) {
    app.world_mut().write_message(EditorCommandBus(command));
    app.update();
}
fn operand(app: &mut App, wanted: Operand) -> Entity {
    app.world_mut()
        .query::<(Entity, &ModulationOperand)>()
        .iter(app.world())
        .find(|(_, field)| field.operand == wanted)
        .unwrap()
        .0
}
fn exact(app: &mut App, wanted: Operand, value: Rational) {
    let source = operand(app, wanted);
    app.world_mut().trigger(ValueChange { source, value });
    app.update();
}
fn choice(app: &mut App, wanted: Choice) {
    let entity = app
        .world_mut()
        .query::<(Entity, &ModulationChoice)>()
        .iter(app.world())
        .find(|(_, button)| button.choice == wanted)
        .unwrap()
        .0;
    app.world_mut().trigger(Activate { entity });
    app.update();
}
fn key(app: &mut App, window: Entity, key_code: KeyCode, text: Option<&str>) {
    app.world_mut().write_message(KeyboardInput {
        key_code,
        logical_key: text.map(|v| Key::Character(v.into())).unwrap_or(Key::Enter),
        state: ButtonState::Pressed,
        text: text.map(Into::into),
        repeat: false,
        window,
    });
    app.update();
}
fn select_text(app: &mut App, window: Entity, field: Entity, text: &str) {
    app.world_mut().resource_mut::<InputFocus>().set(field);
    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .press(KeyCode::ControlLeft);
    key(app, window, KeyCode::KeyA, None);
    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .release(KeyCode::ControlLeft);
    key(app, window, KeyCode::KeyD, Some(text));
    assert!(
        !app.world()
            .resource::<ButtonInput<KeyCode>>()
            .just_pressed(KeyCode::KeyD)
    );
}

#[test]
fn waveform_and_target_choices_preserve_the_latest_clock_settings() {
    let (mut app, node, _) = fixture();
    exact(&mut app, Operand::Rate, Rational::zero());
    assert_eq!(read(&app, &node).1.rate, Rational::zero());
    exact(&mut app, Operand::Rate, Rational::new(-2, 3));
    assert_eq!(read(&app, &node).1.rate, Rational::new(-2, 3));
    exact(&mut app, Operand::Rate, Rational::new(2, 3));
    exact(&mut app, Operand::Phase, Rational::new(-5, 7));
    for (waveform, _) in WAVES {
        choice(&mut app, Choice::Waveform(waveform));
        assert_eq!(read(&app, &node).1.waveform, waveform);
        assert_eq!(read(&app, &node).1.rate, Rational::new(2, 3));
        assert_eq!(read(&app, &node).1.phase, Rational::new(-5, 7));
    }
    for (parameter, _) in TARGETS {
        choice(&mut app, Choice::Target(parameter));
        let (actual, value) = read(&app, &node);
        assert_eq!(actual, parameter);
        assert_eq!(
            value.minimum,
            ModulationParameters::for_parameter(parameter).minimum
        );
        assert_eq!(
            value.maximum,
            ModulationParameters::for_parameter(parameter).maximum
        );
        assert_eq!(value.waveform, ModulationWaveform::Ramp);
        assert_eq!(value.rate, Rational::new(2, 3));
        assert_eq!(value.phase, Rational::new(-5, 7));
    }
    choice(&mut app, Choice::Reset);
    assert_eq!(
        read(&app, &node).1,
        ModulationParameters::for_parameter(ParameterKey::Transpose)
    );
    send(&mut app, EditorCommand::Undo);
    assert_eq!(read(&app, &node).1.rate, Rational::new(2, 3));
}

#[test]
fn exact_operands_follow_the_owned_group_after_reorder_and_survive_history_and_save() {
    let (mut app, node, other) = fixture();
    let original_location = app
        .world()
        .resource::<MusaicProject>()
        .document
        .graph
        .location_of(&node)
        .unwrap();
    send(
        &mut app,
        EditorCommand::MoveModifierGroup {
            owner: node.clone(),
            step: -1,
        },
    );
    let moved_location = app
        .world()
        .resource::<MusaicProject>()
        .document
        .graph
        .location_of(&node)
        .unwrap();
    assert_ne!(original_location, moved_location);
    for (operand, value) in [
        (Operand::Rate, Rational::new(1, 3)),
        (Operand::Phase, Rational::new(-7, 4)),
        (Operand::Minimum, Rational::new(1, 9)),
        (Operand::Maximum, Rational::new(7, 9)),
    ] {
        exact(&mut app, operand, value);
    }
    let edited = read(&app, &node);
    assert_eq!(
        (
            edited.1.rate,
            edited.1.phase,
            edited.1.minimum,
            edited.1.maximum
        ),
        (
            Rational::new(1, 3),
            Rational::new(-7, 4),
            Rational::new(1, 9),
            Rational::new(7, 9)
        )
    );
    assert_eq!(app.world().resource::<CommandHistory>().undo_len(), 5);
    let project = app.world().resource::<MusaicProject>();
    let reopened = import_project_bytes(&export_project_bytes(project).unwrap(), None).unwrap();
    assert_eq!(current(&reopened, &node), Some(edited));
    assert_eq!(
        project.document.graph.node(&other),
        reopened.document.graph.node(&other)
    );
    for _ in 0..5 {
        send(&mut app, EditorCommand::Undo);
    }
    assert_eq!(read(&app, &node).1, ModulationParameters::default());
    assert_eq!(
        app.world()
            .resource::<MusaicProject>()
            .document
            .graph
            .location_of(&node)
            .unwrap(),
        original_location
    );
    for _ in 0..5 {
        send(&mut app, EditorCommand::Redo);
    }
    assert_eq!(read(&app, &node), edited);
    assert_eq!(
        app.world()
            .resource::<MusaicProject>()
            .document
            .graph
            .location_of(&node)
            .unwrap(),
        moved_location
    );
}

#[test]
fn invalid_ranges_remain_focused_without_mutating_the_owned_group_or_history() {
    let (mut app, node, _) = fixture();
    let before = read(&app, &node);
    for (operand, value) in [
        (Operand::Minimum, Rational::from_integer(2)),
        (Operand::Minimum, Rational::from_integer(-1)),
        (
            Operand::Rate,
            Rational {
                numerator: 1,
                denominator: 0,
            },
        ),
    ] {
        exact(&mut app, operand, value);
        assert_eq!(read(&app, &node), before);
        assert_eq!(app.world().resource::<CommandHistory>().undo_len(), 0);
        let field = operand_entity(&mut app, operand);
        assert_eq!(app.world().resource::<InputFocus>().0, Some(field));
        assert!(
            app.world()
                .get::<Text>(field)
                .unwrap()
                .0
                .contains("Enter a valid value")
        );
    }
}
fn operand_entity(app: &mut App, wanted: Operand) -> Entity {
    operand(app, wanted)
}

#[test]
fn seed_entry_accepts_the_full_u64_range_and_rejects_overflow_without_shortcuts() {
    let (mut app, node, _) = fixture();
    let field = app
        .world_mut()
        .query_filtered::<Entity, With<SeedInput>>()
        .single(app.world())
        .unwrap();
    let window = app.world_mut().spawn(bevy::window::PrimaryWindow).id();
    exact(&mut app, Operand::Rate, Rational::new(3, 7));
    let baseline_history = app.world().resource::<CommandHistory>().undo_len();
    for value in [u64::MAX, 1u64 << 63, 0] {
        select_text(&mut app, window, field, &value.to_string());
        key(&mut app, window, KeyCode::Enter, None);
        assert_eq!(read(&app, &node).1.seed, value);
        assert_eq!(read(&app, &node).1.rate, Rational::new(3, 7));
        assert_eq!(app.world().resource::<InputFocus>().0, None);
    }
    assert_eq!(
        app.world().resource::<CommandHistory>().undo_len(),
        baseline_history + 3
    );
    for invalid in ["18446744073709551616", "-1", "1/2", ""] {
        select_text(&mut app, window, field, invalid);
        key(&mut app, window, KeyCode::Enter, None);
        assert_eq!(read(&app, &node).1.seed, 0);
        assert_eq!(app.world().resource::<InputFocus>().0, Some(field));
        assert_eq!(
            app.world().resource::<CommandHistory>().undo_len(),
            baseline_history + 3
        );
        key(&mut app, window, KeyCode::Escape, None);
        assert_eq!(app.world().resource::<InputFocus>().0, None);
    }
    send(&mut app, EditorCommand::Undo);
    assert_eq!(read(&app, &node).1.seed, 1u64 << 63);
    send(&mut app, EditorCommand::Undo);
    assert_eq!(read(&app, &node).1.seed, u64::MAX);
    let reopened = import_project_bytes(
        &export_project_bytes(app.world().resource::<MusaicProject>()).unwrap(),
        None,
    )
    .unwrap();
    assert_eq!(current(&reopened, &node).unwrap().1.seed, u64::MAX);
}

#[test]
fn velocity_target_saves_saw_and_seeded_noise_and_rejects_out_of_range_levels() {
    let (mut app, node, _) = fixture();
    choice(&mut app, Choice::Target(ParameterKey::Velocity));
    exact(&mut app, Operand::Rate, Rational::from_integer(4));
    exact(&mut app, Operand::Minimum, Rational::new(7, 10));
    for waveform in [
        ModulationWaveform::Saw,
        ModulationWaveform::SteppedNoise,
        ModulationWaveform::Random,
    ] {
        choice(&mut app, Choice::Waveform(waveform));
        let before = read(&app, &node);
        let history = app.world().resource::<CommandHistory>().undo_len();
        exact(&mut app, Operand::Maximum, Rational::new(11, 10));
        assert_eq!(read(&app, &node), before);
        assert_eq!(app.world().resource::<CommandHistory>().undo_len(), history);
        let reopened = import_project_bytes(
            &export_project_bytes(app.world().resource::<MusaicProject>()).unwrap(),
            None,
        )
        .unwrap();
        assert_eq!(current(&reopened, &node), Some(before));
    }
}

#[test]
fn random_waveform_survives_target_changes_history_and_save_for_every_supported_target() {
    let (mut app, node, _) = fixture();
    choice(&mut app, Choice::Waveform(ModulationWaveform::Random));
    for (target, _) in TARGETS {
        choice(&mut app, Choice::Target(target));
        let value = read(&app, &node);
        assert_eq!(value.0, target);
        assert_eq!(value.1.waveform, ModulationWaveform::Random);
        value.1.validate_for(target).unwrap();
        let reopened = import_project_bytes(
            &export_project_bytes(app.world().resource::<MusaicProject>()).unwrap(),
            None,
        )
        .unwrap();
        assert_eq!(current(&reopened, &node), Some(value));
    }
    send(&mut app, EditorCommand::Undo);
    assert_eq!(read(&app, &node).1.waveform, ModulationWaveform::Random);
    send(&mut app, EditorCommand::Redo);
    assert_eq!(read(&app, &node).0, ParameterKey::Transpose);
    assert_eq!(read(&app, &node).1.waveform, ModulationWaveform::Random);
}
