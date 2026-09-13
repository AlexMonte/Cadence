//! First sound, ordinary editing, and persistence through the real command owner.
#[path = "support/acceptance.rs"]
mod support;
use bevy::prelude::App;
use cadence::{
    adapter::audio::{AudioRenderer, AudioRendererSettings, Frame},
    infrastructure::playback::{PlaybackRuntime, PlaybackSettings},
    prelude::{CadenceCompiler, ControlKey, ControlValue, PreparedScore, Span, Time},
};
use musaic::{
    MusaicProject,
    application::{command::EditorCommand, pipeline::runtime::RuntimeState, session::first_loop},
    domain::document::{AtomValue, NoteName},
};

fn score(app: &App) -> PreparedScore {
    let scores = &app
        .world()
        .resource::<RuntimeState>()
        .compiled
        .as_ref()
        .unwrap()
        .scores;
    assert_eq!(scores.len(), 1);
    scores.values().next().unwrap().clone()
}
fn notes(score: &PreparedScore) -> Vec<(Time, Time, i64)> {
    CadenceCompiler::new()
        .preview(score, &Span::new(Time::ZERO, Time::ONE).unwrap())
        .unwrap()
        .starts()
        .map(|event| {
            let moment = event.projected();
            let Some(ControlValue::Scalar(pitch)) = moment.controls().get(&ControlKey::Pitch)
            else {
                panic!("pitched note");
            };
            (moment.whole().start(), moment.whole().end(), *pitch as i64)
        })
        .collect()
}
fn pcm(project: &MusaicProject, score: PreparedScore) -> Vec<Frame> {
    let (audio, mut renderer) =
        AudioRenderer::split(AudioRendererSettings::new(8_000, 4096)).unwrap();
    let cps = project.document.playback.cycles_per_second();
    let mut runtime = PlaybackRuntime::new(
        PlaybackSettings {
            cps,
            look_ahead: Time::new(1, 4),
            step: Time::new(1, 64),
        },
        audio.clone(),
    );
    runtime.play_prepared_score(score).unwrap();
    let mut pcm = vec![Frame::ZERO; (8_000.0 / cps.value()).round() as usize];
    for block in pcm.chunks_mut(128) {
        runtime.tick().unwrap();
        renderer.render(block);
        assert_eq!(audio.resource_limit_count(), 0);
    }
    assert!(
        pcm.iter()
            .all(|f| f.left.is_finite() && f.right.is_finite())
    );
    assert!(
        pcm.iter().any(|f| f.left.abs() > 1e-6),
        "starter really produces audio"
    );
    pcm
}

#[test]
fn starter_is_audible_editable_undoable_and_survives_save_open() {
    let starter = first_loop();
    let original_document = starter.project.document.clone();
    let mut app = support::editor(starter.project);
    app.update();
    let before = score(&app);
    assert_eq!(
        notes(&before),
        vec![
            (Time::ZERO, Time::new(1, 4), 60),
            (Time::new(1, 4), Time::new(1, 2), 64),
            (Time::new(1, 2), Time::new(3, 4), 67),
        ]
    );
    let before_pcm = pcm(app.world().resource::<MusaicProject>(), before);
    support::send(
        &mut app,
        EditorCommand::SetAtomValue {
            node: starter.first_note,
            value: AtomValue::NoteName(NoteName::D),
        },
    );
    app.update();
    let edited = score(&app);
    assert_eq!(notes(&edited)[0], (Time::ZERO, Time::new(1, 4), 62));
    let edited_pcm = pcm(app.world().resource::<MusaicProject>(), edited.clone());
    assert_ne!(edited_pcm, before_pcm);
    support::send(&mut app, EditorCommand::Undo);
    app.update();
    assert_eq!(
        app.world().resource::<MusaicProject>().document.graph,
        original_document.graph
    );
    assert_eq!(
        pcm(app.world().resource::<MusaicProject>(), score(&app)),
        before_pcm
    );
    support::send(&mut app, EditorCommand::Redo);
    app.update();
    let folder = support::Folder::new();
    let path = folder.0.join("first-loop.musaic.json");
    support::send(
        &mut app,
        EditorCommand::SaveProjectAs { path: path.clone() },
    );
    let saved = app.world().resource::<MusaicProject>().document.clone();
    support::send(&mut app, EditorCommand::NewProject);
    support::send(&mut app, EditorCommand::OpenProject { path });
    app.update();
    assert_eq!(
        app.world().resource::<MusaicProject>().document.graph,
        saved.graph
    );
    assert_eq!(notes(&score(&app)), notes(&edited));
    assert_eq!(
        pcm(app.world().resource::<MusaicProject>(), score(&app)),
        edited_pcm
    );
}
