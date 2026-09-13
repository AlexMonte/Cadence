//! Exercise exact controls through keyboard dispatch, editor history, and persistence.
use super::*;
use crate::{
    adapter::persistence::{export_project_bytes, import_project_bytes},
    application::{
        editor::EditorPlugin, history::CommandHistory, pipeline::PlaybackPlugin,
        session::MusaicProject,
    },
    domain::{
        document::DocumentNodeKind,
        instrument::{InstrumentCompressor, InstrumentDelay, InstrumentEffect, InstrumentReverb},
    },
    infrastructure::app::{AppState, TransportMode},
};
use bevy::input::{
    ButtonState,
    keyboard::{Key, KeyboardInput},
};

fn fixture() -> (App, NodeId) {
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
    app.insert_state(AppState::Editor).add_systems(
        Last,
        (super::tests::refresh_test_projection, sync_sound_controls).chain(),
    );
    let mut project = MusaicProject::demo();
    let output = project
        .document
        .graph
        .nodes_on_surface(project.document.root_surface)
        .into_iter()
        .find(|(_, node)| matches!(&node.kind, DocumentNodeKind::Sound(_)))
        .unwrap()
        .1
        .id
        .clone();
    let mut instrument = InstrumentDefinition::new(InstrumentSource::Drum("hh".into()));
    instrument.sound.delay = Some(InstrumentDelay::default());
    instrument.sound.reverb = Some(InstrumentReverb::default());
    instrument.sound.compressor = Some(InstrumentCompressor::default());
    instrument.sound.effects = vec![
        InstrumentEffect::LowPass {
            cutoff_hz: Rational::new(500, 1),
            resonance: Rational::zero(),
        },
        InstrumentEffect::HighPass {
            cutoff_hz: Rational::new(100, 1),
            resonance: Rational::zero(),
        },
        InstrumentEffect::Drive {
            amount: Rational::zero(),
            wet: Rational::one(),
            output_gain: Rational::one(),
        },
    ];
    project
        .document
        .graph
        .set_sound_definition(&output, instrument)
        .unwrap();
    app.insert_resource(project);
    app.update();
    let sound = projected_sound(app.world().resource::<EditorUiProjection>(), &output)
        .unwrap()
        .clone();
    app.world_mut()
        .commands()
        .spawn(Node::default())
        .with_children(|parent| spawn_sound_panel(parent, &output, &sound));
    app.world_mut().flush();
    (app, output)
}

fn field(app: &mut App, parameter: SoundParameter) -> Entity {
    app.world_mut()
        .query::<(Entity, &SoundParameterExact)>()
        .iter(app.world())
        .find(|(_, editor)| editor.0.parameter == parameter)
        .unwrap()
        .0
}
fn send(app: &mut App, command: EditorCommand) {
    app.world_mut().write_message(EditorCommandBus(command));
    app.update();
}
fn instrument(app: &App, output: &NodeId) -> InstrumentDefinition {
    app.world()
        .resource::<MusaicProject>()
        .document
        .graph
        .sound_definition(output)
        .unwrap()
        .clone()
}
fn edit(app: &mut App, parameter: SoundParameter, value: Rational) {
    let source = field(app, parameter);
    app.world_mut().trigger(ValueChange { source, value });
    app.update();
}
fn key(app: &mut App, window: Entity, key_code: KeyCode, text: Option<&str>) {
    app.world_mut().write_message(KeyboardInput {
        key_code,
        logical_key: text
            .map(|value| Key::Character(value.into()))
            .unwrap_or(Key::Enter),
        state: ButtonState::Pressed,
        text: text.map(Into::into),
        repeat: false,
        window,
    });
    app.update();
}

#[test]
fn keyboard_entry_preserves_latest_settings_and_has_one_undo_step() {
    let (mut app, output) = fixture();
    let attack = field(&mut app, SoundParameter::Attack);
    // A field stays alive while unrelated controls change the current assignment.
    let mut latest = instrument(&app, &output);
    latest.source = InstrumentSource::Synth(Waveform::Square);
    latest.sound.gain = Some(Rational::new(2, 7));
    send(
        &mut app,
        EditorCommand::SetSound {
            sound: output.clone(),
            definition: latest.clone(),
        },
    );
    let history = app.world().resource::<CommandHistory>().undo_len();
    let window = app.world_mut().spawn(bevy::window::PrimaryWindow).id();
    app.world_mut().resource_mut::<InputFocus>().set(attack);
    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .press(KeyCode::ControlLeft);
    key(&mut app, window, KeyCode::KeyA, None);
    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .release(KeyCode::ControlLeft);
    key(&mut app, window, KeyCode::KeyD, Some("59.125"));
    assert!(
        !app.world()
            .resource::<ButtonInput<KeyCode>>()
            .just_pressed(KeyCode::KeyD)
    );
    // Model refresh must not discard a draft while the field is focused.
    assert!(
        app.world()
            .get::<Text>(attack)
            .unwrap()
            .0
            .contains("59.125")
    );
    key(&mut app, window, KeyCode::Enter, None);
    let edited = instrument(&app, &output);
    assert_eq!(edited.effective_envelope().attack, Rational::new(473, 8));
    assert_eq!(edited.sound.gain, latest.sound.gain);
    assert_eq!(edited.source, latest.source);
    assert_eq!(app.world().resource::<InputFocus>().0, None);
    assert_eq!(
        app.world().resource::<CommandHistory>().undo_len(),
        history + 1
    );
    send(&mut app, EditorCommand::Undo);
    assert_eq!(instrument(&app, &output), latest);
    send(&mut app, EditorCommand::Redo);
    assert_eq!(instrument(&app, &output), edited);
    assert_eq!(
        app.world()
            .get::<exact_number::ExactNumber>(attack)
            .unwrap()
            .value,
        Rational::new(473, 8)
    );
}

#[test]
fn every_sound_parameter_accepts_precise_values_and_survives_save_and_history() {
    use super::super::instrument_shape::EffectParameter as E;
    use SoundParameter as P;
    let (mut app, output) = fixture();
    let original = instrument(&app, &output);
    let changes = [
        (P::Rate, Rational::new(1, 1000)),
        (P::Start, Rational::new(1, 997)),
        (P::End, Rational::new(996, 997)),
        (P::Gain, Rational::new(1, 7)),
        (P::Attack, Rational::new(60, 1)),
        (P::Decay, Rational::new(179, 3)),
        (P::Sustain, Rational::new(1, 7)),
        (P::Release, Rational::new(60, 1)),
        (P::DelayAmount, Rational::new(1, 7)),
        (P::DelayTime, Rational::new(1, 997)),
        (P::DelayFeedback, Rational::new(1, 7)),
        (P::DelayDamping, Rational::new(1, 7)),
        (P::ReverbAmount, Rational::new(1, 7)),
        (P::ReverbDecay, Rational::new(60, 1)),
        (P::ReverbDamping, Rational::new(1, 7)),
        (P::CompressorThreshold, Rational::new(1, 7)),
        (P::CompressorRatio, Rational::new(22, 7)),
        (P::CompressorKnee, Rational::new(71, 7)),
        (P::CompressorAttack, Rational::new(60, 1)),
        (P::CompressorRelease, Rational::new(60, 1)),
        (P::Effect(0, E::Cutoff), Rational::new(440, 3)),
        (P::Effect(0, E::Resonance), Rational::new(1, 7)),
        (P::Effect(1, E::Cutoff), Rational::new(70, 3)),
        (P::Effect(1, E::Resonance), Rational::new(1, 7)),
        (P::Effect(2, E::Amount), Rational::new(1, 7)),
        (P::Effect(2, E::Wet), Rational::new(1, 7)),
        (P::Effect(2, E::OutputGain), Rational::new(1, 7)),
    ];
    for (parameter, value) in changes {
        edit(&mut app, parameter, value);
        assert_eq!(
            parameter.exact_value(&instrument(&app, &output)),
            value,
            "{parameter:?}"
        );
    }
    let authored = instrument(&app, &output);
    let bytes = export_project_bytes(app.world().resource::<MusaicProject>()).unwrap();
    let reopened = import_project_bytes(&bytes, None).unwrap();
    assert_eq!(
        reopened.document.graph.sound_definition(&output).unwrap(),
        &authored
    );
    assert_eq!(
        app.world().resource::<CommandHistory>().undo_len(),
        changes.len()
    );
    for _ in changes {
        send(&mut app, EditorCommand::Undo);
    }
    assert_eq!(instrument(&app, &output), original);
    for _ in changes {
        send(&mut app, EditorCommand::Redo);
    }
    assert_eq!(instrument(&app, &output), authored);
    // Values below and above the convenient sample-speed slider remain reachable.
    edit(&mut app, P::Rate, Rational::new(64, 1));
    assert_eq!(instrument(&app, &output).rate, Rational::new(64, 1));
}

#[test]
fn invalid_exact_values_keep_focus_and_leave_project_and_history_unchanged() {
    use super::super::instrument_shape::EffectParameter as E;
    use SoundParameter as P;
    let (mut app, output) = fixture();
    let initial = instrument(&app, &output);
    for (parameter, value) in [
        (P::Rate, Rational::new(65, 1)),
        (P::Start, Rational::one()),
        (P::End, Rational::zero()),
        (P::Gain, Rational::new(5, 1)),
        (P::Attack, Rational::new(61, 1)),
        (P::Sustain, Rational::new(-1, 1)),
        (P::DelayTime, Rational::new(1, 1001)),
        (P::DelayFeedback, Rational::new(2, 1)),
        (P::ReverbDecay, Rational::new(61, 1)),
        (P::CompressorRatio, Rational::new(21, 1)),
        (P::CompressorKnee, Rational::new(41, 1)),
        (P::CompressorAttack, Rational::zero()),
        (P::CompressorRelease, Rational::new(61, 1)),
        (P::Effect(0, E::Cutoff), Rational::new(19, 1)),
        (P::Effect(2, E::Wet), Rational::new(2, 1)),
    ] {
        edit(&mut app, parameter, value);
        assert_eq!(instrument(&app, &output), initial, "{parameter:?}");
        assert_eq!(app.world().resource::<CommandHistory>().undo_len(), 0);
        let source = field(&mut app, parameter);
        assert_eq!(app.world().resource::<InputFocus>().0, Some(source));
        assert!(
            app.world()
                .get::<Text>(source)
                .unwrap()
                .0
                .contains("Enter a valid value")
        );
    }
}
