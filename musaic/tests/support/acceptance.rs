//! Native command-path fixtures shared by current sample acceptance tests.
#![allow(dead_code)]

use bevy::prelude::*;
use musaic::{
    application::{
        command::{EditorCommand, EditorCommandBus},
        editor::EditorPlugin,
        pipeline::PlaybackPlugin,
        session::MusaicProject,
    },
    domain::{
        board::BoardSlot,
        document::{
            AtomValue, ContainerKind, NoteName, PlacementAddress, StackIndex, TileSpawnKind,
            bind_tiles, export_document_program,
        },
    },
    infrastructure::app::{AppState, MusaicSet, TransportMode},
};
use tessera::prelude::NodeId;

pub fn editor(project: MusaicProject) -> App {
    let mut app = App::new();
    app.add_plugins(bevy::state::app::StatesPlugin)
        .init_state::<AppState>()
        .init_state::<TransportMode>()
        .add_plugins(MinimalPlugins)
        .add_plugins((EditorPlugin, PlaybackPlugin));
    app.configure_sets(
        Update,
        (
            MusaicSet::Input,
            MusaicSet::Commands,
            MusaicSet::DocumentMutation,
            MusaicSet::Compile,
            MusaicSet::Lower,
            MusaicSet::Runtime,
            MusaicSet::SceneSync,
            MusaicSet::RenderUi,
        )
            .chain(),
    )
    .configure_sets(
        Update,
        (
            cadence::bevy::CadenceSet::ReplaceScores.in_set(MusaicSet::Runtime),
            cadence::bevy::CadenceSet::Tick.in_set(MusaicSet::Runtime),
        ),
    );
    app.insert_state(AppState::Editor);
    app.world_mut()
        .write_message(EditorCommandBus(EditorCommand::AdoptProject {
            project,
            path: None,
        }));
    app.update();
    app
}

pub fn send(app: &mut App, command: EditorCommand) {
    app.world_mut().write_message(EditorCommandBus(command));
    app.update();
}

pub fn notes() -> (MusaicProject, NodeId, NodeId) {
    let mut project = MusaicProject::new_empty();
    let root = project.document.root_surface;
    let source = project
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
    let surface = project.document.graph.container_surface(&source).unwrap();
    for (index, note) in [NoteName::C, NoteName::E].into_iter().enumerate() {
        project
            .document
            .graph
            .insert_tile(
                &mut project.document.surfaces,
                surface,
                PlacementAddress::StackIndex(StackIndex(index)),
                TileSpawnKind::Atom {
                    atom: AtomValue::NoteName(note),
                },
            )
            .unwrap();
    }
    let instrument = project
        .document
        .graph
        .insert_tile(
            &mut project.document.surfaces,
            root,
            PlacementAddress::BoardSlot(BoardSlot::new(5, 0)),
            TileSpawnKind::sound(musaic::domain::instrument::InstrumentDefinition::default()),
        )
        .unwrap();
    let output = project
        .document
        .graph
        .insert_tile(
            &mut project.document.surfaces,
            root,
            PlacementAddress::BoardSlot(BoardSlot::new(6, 0)),
            TileSpawnKind::Output {
                name: "Music".into(),
            },
        )
        .unwrap();
    let mut program = export_document_program(&project.document).unwrap();
    for (from, to) in [(&source, &instrument), (&instrument, &output)] {
        bind_tiles(&mut program, from, to, tessera::prelude::SpatialSide::East).unwrap();
    }
    project.document.replace_connections_from(&program);
    (project, source, instrument)
}

pub struct Folder(pub std::path::PathBuf);
impl Folder {
    pub fn new() -> Self {
        static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let path = std::env::temp_dir().join(format!(
            "musaic-acceptance-{}-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&path).unwrap();
        Self(path)
    }
}
impl Drop for Folder {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

pub fn wait_for(app: &mut App, done: impl Fn(&App) -> bool) {
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    while !done(app) {
        assert!(
            std::time::Instant::now() < deadline,
            "worker did not finish in ten seconds"
        );
        std::thread::sleep(std::time::Duration::from_millis(1));
        app.update();
    }
}

pub fn wav(rate: u32, frames: usize, amplitude: i16) -> Vec<u8> {
    let length = (frames * 2) as u32;
    let mut bytes = Vec::new();
    bytes.extend_from_slice(b"RIFF");
    bytes.extend_from_slice(&(36 + length).to_le_bytes());
    bytes.extend_from_slice(b"WAVEfmt ");
    bytes.extend_from_slice(&16_u32.to_le_bytes());
    bytes.extend_from_slice(&1_u16.to_le_bytes());
    bytes.extend_from_slice(&1_u16.to_le_bytes());
    bytes.extend_from_slice(&rate.to_le_bytes());
    bytes.extend_from_slice(&(rate * 2).to_le_bytes());
    bytes.extend_from_slice(&2_u16.to_le_bytes());
    bytes.extend_from_slice(&16_u16.to_le_bytes());
    bytes.extend_from_slice(b"data");
    bytes.extend_from_slice(&length.to_le_bytes());
    for index in 0..frames {
        // A periodic waveform makes tuning changes visible in actual rendered PCM.
        let sample = if index % 40 < 20 {
            amplitude
        } else {
            -amplitude
        };
        bytes.extend_from_slice(&sample.to_le_bytes());
    }
    bytes
}
