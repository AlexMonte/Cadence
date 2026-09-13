//! Named drum hits remain ordinary authored tiles through playback and persistence.
#[path = "support/acceptance.rs"]
mod support;

use bevy::prelude::App;
use cadence::prelude::{CadenceCompiler, Intent, Span, Time};
use musaic::{
    MusaicProject,
    application::{
        audio_export::render_project_wav,
        command::EditorCommand,
        pipeline::{
            runtime::RuntimeState,
            scene_sync::{TileSurfaceContent, VisibleBoardState},
        },
    },
    domain::{
        document::{AtomValue, DocumentNodeKind, DrumHit},
        instrument::InstrumentSource,
    },
};
use tessera::prelude::NodeId;

fn drum_hits(app: &App) -> Vec<(Time, String)> {
    let scores = &app
        .world()
        .resource::<RuntimeState>()
        .compiled
        .as_ref()
        .expect("reference beat compiles")
        .scores;
    assert_eq!(scores.len(), 1);
    let prepared = scores.values().next().expect("Drums output score");
    CadenceCompiler::new()
        .preview(prepared, &Span::new(Time::ZERO, Time::ONE).unwrap())
        .unwrap()
        .starts()
        .map(|event| {
            let projected = event.projected();
            let Intent::Sample(sample) = projected.intent() else {
                panic!("a Kit must lower named hits to samples")
            };
            (projected.whole().start(), sample.sample_id.clone())
        })
        .collect()
}

fn snare_node(project: &MusaicProject) -> NodeId {
    project
        .document
        .graph
        .nodes()
        .find_map(|node| match &node.kind {
            DocumentNodeKind::Atom(atom) if atom.atom == AtomValue::DrumHit(DrumHit::Sd) => {
                Some(node.id.clone())
            }
            _ => None,
        })
        .expect("reference beat has an editable snare atom")
}

#[test]
fn connected_named_beat_plays_edits_undoes_and_survives_save_open() {
    let project = MusaicProject::reference_demo();
    let original_graph = project.document.graph.clone();
    let snare = snare_node(&project);
    let kit = project
        .document
        .graph
        .nodes()
        .find_map(|node| {
            matches!(&node.kind, DocumentNodeKind::Sound(sound) if matches!(sound.definition.source, InstrumentSource::Kit))
                .then(|| node.id.clone())
        })
        .expect("reference beat has a Kit Sound tile");
    let output = project
        .document
        .graph
        .nodes()
        .find_map(|node| match &node.kind {
            DocumentNodeKind::Output(output) if output.name == "Drums" => Some(node.id.clone()),
            _ => None,
        })
        .expect("reference beat has a Drums output");
    let connections = musaic::domain::document::connection_policy::endpoint_connections(
        &musaic::domain::document::export_document_program(&project.document).unwrap(),
    );
    assert!(
        connections
            .iter()
            .any(|edge| edge.from == kit && edge.to == output)
    );

    let before_wav = render_project_wav(&project, 1).expect("reference beat renders");
    let mut app = support::editor(project);
    app.update();
    assert!(
        app.world()
            .resource::<VisibleBoardState>()
            .nodes
            .iter()
            .any(|node| {
                node.node == kit
                    && matches!(
                        &node.surface_content,
                        TileSurfaceContent::Transform { label, .. } if label == "Kit"
                    )
            })
    );
    assert_eq!(
        drum_hits(&app),
        vec![
            (Time::ZERO, "bd".into()),
            (Time::new(1, 4), "sd".into()),
            (Time::new(1, 2), "bd".into()),
            (Time::new(3, 4), "sd".into()),
        ]
    );

    support::send(
        &mut app,
        EditorCommand::SetAtomValue {
            node: snare,
            value: AtomValue::DrumHit(DrumHit::Hh),
        },
    );
    app.update();
    let edited_hits = drum_hits(&app);
    assert_eq!(
        edited_hits,
        vec![
            (Time::ZERO, "bd".into()),
            (Time::new(1, 4), "hh".into()),
            (Time::new(1, 2), "bd".into()),
            (Time::new(3, 4), "hh".into()),
        ]
    );
    let edited_wav =
        render_project_wav(app.world().resource::<MusaicProject>(), 1).expect("edit renders");
    assert_ne!(edited_wav, before_wav, "editing a hit changes audible PCM");

    support::send(&mut app, EditorCommand::Undo);
    app.update();
    assert_eq!(
        app.world().resource::<MusaicProject>().document.graph,
        original_graph
    );
    assert_eq!(
        render_project_wav(app.world().resource::<MusaicProject>(), 1).unwrap(),
        before_wav
    );
    support::send(&mut app, EditorCommand::Redo);
    app.update();

    let folder = support::Folder::new();
    let path = folder.0.join("named-drum-beat.musaic.json");
    support::send(
        &mut app,
        EditorCommand::SaveProjectAs { path: path.clone() },
    );
    support::send(&mut app, EditorCommand::NewProject);
    support::send(&mut app, EditorCommand::OpenProject { path });
    app.update();

    let reopened = app.world().resource::<MusaicProject>();
    assert!(
        reopened.document.graph.nodes().any(|node| {
            matches!(&node.kind, DocumentNodeKind::Sound(sound) if matches!(sound.definition.source, InstrumentSource::Kit))
        })
    );
    assert_eq!(drum_hits(&app), edited_hits);
    assert_eq!(render_project_wav(reopened, 1).unwrap(), edited_wav);
}
