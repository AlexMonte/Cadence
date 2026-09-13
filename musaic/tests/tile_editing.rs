//! Authoring tools preserve complete expressions, nested identities, and history.
use bevy::prelude::*;
use musaic::{
    adapter::persistence::{export_project_bytes, import_project_bytes},
    application::{
        command::{EditorCommand, EditorCommandBus, editing::TileEdit},
        editor::{EditorPlugin, FocusTarget, SelectionMode, SelectionState},
        history::CommandHistory,
        pipeline::PlaybackPlugin,
        session::MusaicProject,
    },
    domain::{
        board::{BoardSlot, BoardSurfaceId},
        document::{
            self, AtomValue, ContainerKind, DocumentNodeKind, NodeLocation, NoteName,
            PlacementAddress, StackIndex, TileSpawnKind,
        },
    },
    infrastructure::app::{AppState, TransportMode},
};
use tessera::prelude::{AtomModifier, NodeId, Rational};

fn fixture() -> (App, NodeId, BoardSurfaceId, Vec<NodeId>) {
    fixture_atoms(vec![
        AtomValue::NoteName(NoteName::C),
        AtomValue::Octave(4),
        AtomValue::Modifier(AtomModifier::Fast(Rational::new(3, 1))),
        AtomValue::NoteName(NoteName::E),
        AtomValue::Octave(4),
    ])
}

fn fixture_atoms(atoms: Vec<AtomValue>) -> (App, NodeId, BoardSurfaceId, Vec<NodeId>) {
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
    let mut ids = Vec::new();
    for (i, atom) in atoms.into_iter().enumerate() {
        ids.push(
            project
                .document
                .graph
                .insert_tile(
                    &mut project.document.surfaces,
                    surface,
                    PlacementAddress::StackIndex(StackIndex(i)),
                    TileSpawnKind::Atom { atom },
                )
                .unwrap(),
        );
    }
    let mut app = App::new();
    app.add_plugins(bevy::state::app::StatesPlugin)
        .init_state::<AppState>()
        .init_state::<TransportMode>()
        .add_plugins(MinimalPlugins)
        .add_plugins((EditorPlugin, PlaybackPlugin));
    app.insert_state(AppState::Editor);
    send(
        &mut app,
        EditorCommand::AdoptProject {
            project,
            path: None,
        },
    );
    send(
        &mut app,
        EditorCommand::EnterContainer {
            container: container.clone(),
        },
    );
    (app, container, surface, ids)
}
fn send(app: &mut App, command: EditorCommand) {
    app.world_mut().write_message(EditorCommandBus(command));
    for _ in 0..3 {
        app.update();
    }
}
fn select(app: &mut App, node: NodeId) {
    send(
        app,
        EditorCommand::SelectNode {
            node,
            mode: SelectionMode::Replace,
        },
    );
}
fn edit(app: &mut App, action: TileEdit) {
    send(app, EditorCommand::EditTiles(action));
}
fn empty(app: &mut App, surface: BoardSurfaceId, index: usize) {
    send(
        app,
        EditorCommand::Focus {
            target: FocusTarget::StackInsert {
                surface,
                index: StackIndex(index),
            },
        },
    );
}
fn layout(app: &App, surface: BoardSurfaceId) -> Vec<(NodeId, NodeLocation, DocumentNodeKind)> {
    app.world()
        .resource::<MusaicProject>()
        .document
        .graph
        .nodes_on_surface(surface)
        .into_iter()
        .map(|(loc, n)| (n.id.clone(), loc, n.kind.clone()))
        .collect()
}

#[test]
fn removing_an_accidental_preserves_the_note_and_closes_only_its_slot() {
    let (mut app, _, surface, ids) = fixture_atoms(vec![
        AtomValue::NoteName(NoteName::C),
        AtomValue::Octave(4),
        AtomValue::Accidental(document::Accidental::Sharp),
        AtomValue::Modifier(AtomModifier::Fast(Rational::new(2, 1))),
        AtomValue::NoteName(NoteName::E),
        AtomValue::Octave(4),
    ]);
    let before = layout(&app, surface);
    select(&mut app, ids[0].clone());
    edit(
        &mut app,
        TileEdit::RemoveAccidental {
            node: ids[2].clone(),
        },
    );
    let after = layout(&app, surface);
    assert_eq!(after.len(), before.len() - 1);
    let project = app.world().resource::<MusaicProject>();
    assert!(project.document.graph.contains_node(&ids[0]));
    assert!(project.document.graph.contains_node(&ids[1]));
    assert!(!project.document.graph.contains_node(&ids[2]));
    assert_eq!(
        project.document.graph.location_of(&ids[3]).unwrap().address,
        PlacementAddress::StackIndex(StackIndex(2))
    );
    let reopened = import_project_bytes(&export_project_bytes(project).unwrap(), None).unwrap();
    assert!(!reopened.document.graph.contains_node(&ids[2]));
    send(&mut app, EditorCommand::Undo);
    assert_eq!(layout(&app, surface), before);
    send(&mut app, EditorCommand::Redo);
    assert_eq!(layout(&app, surface), after);
    edit(
        &mut app,
        TileEdit::RemoveAccidental {
            node: ids[0].clone(),
        },
    );
    assert_eq!(
        layout(&app, surface),
        after,
        "A pitch cannot be removed by the accidental action"
    );
}

#[test]
fn duplicate_copies_the_whole_note_and_redo_keeps_ids_for_later_edits() {
    let (mut app, _, surface, ids) = fixture();
    select(&mut app, ids[0].clone());
    let before = layout(&app, surface);
    edit(&mut app, TileEdit::Duplicate);
    let after = layout(&app, surface);
    assert_eq!(
        after.len(),
        before.len() + 3,
        "Pitch, octave and speed travel together"
    );
    let new = app
        .world()
        .resource::<SelectionState>()
        .nodes
        .iter()
        .next()
        .unwrap()
        .clone();
    send(
        &mut app,
        EditorCommand::SetAtomValue {
            node: new.clone(),
            value: AtomValue::NoteName(NoteName::G),
        },
    );
    send(&mut app, EditorCommand::Undo);
    send(&mut app, EditorCommand::Undo);
    assert_eq!(layout(&app, surface), before);
    send(&mut app, EditorCommand::Redo);
    assert_eq!(layout(&app, surface), after);
    send(&mut app, EditorCommand::Redo);
    assert!(
        matches!(&app.world().resource::<MusaicProject>().document.graph.node(&new).unwrap().kind, DocumentNodeKind::Atom(a) if a.atom == AtomValue::NoteName(NoteName::G))
    );
    let bytes = export_project_bytes(app.world().resource::<MusaicProject>()).unwrap();
    let restored = import_project_bytes(&bytes, None).unwrap();
    assert!(restored.document.graph.contains_node(&new));
}

#[test]
fn move_to_empty_slot_preserves_ids_and_rejected_overlap_is_atomic() {
    let (mut app, _, surface, ids) = fixture();
    let before = layout(&app, surface);
    select(&mut app, ids[0].clone());
    empty(&mut app, surface, 8);
    edit(&mut app, TileEdit::MoveHere);
    for (offset, id) in ids[..3].iter().enumerate() {
        assert_eq!(
            app.world()
                .resource::<MusaicProject>()
                .document
                .graph
                .location_of(id)
                .unwrap()
                .address,
            PlacementAddress::StackIndex(StackIndex(8 + offset))
        );
    }
    let moved = layout(&app, surface);
    empty(&mut app, surface, 2);
    edit(&mut app, TileEdit::MoveHere);
    assert_eq!(layout(&app, surface), moved);
    assert_eq!(app.world().resource::<CommandHistory>().undo_len(), 1);
    send(&mut app, EditorCommand::Undo);
    assert_eq!(layout(&app, surface), before);
    send(&mut app, EditorCommand::Redo);
    assert_eq!(layout(&app, surface), moved);
}

#[test]
fn group_wraps_complete_notes_and_undo_restores_following_positions() {
    let (mut app, _, surface, ids) = fixture();
    select(&mut app, ids[0].clone());
    let before = layout(&app, surface);
    edit(&mut app, TileEdit::Group(ContainerKind::Sequence));
    let group = app
        .world()
        .resource::<SelectionState>()
        .nodes
        .iter()
        .next()
        .unwrap()
        .clone();
    let project = app.world().resource::<MusaicProject>();
    let local = project.document.graph.container_surface(&group).unwrap();
    assert_eq!(project.document.graph.nodes_on_surface(local).len(), 3);
    assert_eq!(
        project.document.graph.location_of(&ids[3]).unwrap().address,
        PlacementAddress::StackIndex(StackIndex(1))
    );
    send(&mut app, EditorCommand::Undo);
    assert_eq!(layout(&app, surface), before);
    send(&mut app, EditorCommand::Redo);
    assert_eq!(
        app.world()
            .resource::<MusaicProject>()
            .document
            .graph
            .container_surface(&group),
        Some(local)
    );
    assert_eq!(
        app.world()
            .resource::<MusaicProject>()
            .document
            .graph
            .location_of(&ids[0])
            .unwrap()
            .surface,
        local
    );
}

#[test]
fn copied_container_has_independent_children_and_survives_source_deletion() {
    let (mut app, container, _, _) = fixture();
    let root = app
        .world()
        .resource::<MusaicProject>()
        .document
        .root_surface;
    send(&mut app, EditorCommand::NavigateToSurface { surface: root });
    select(&mut app, container.clone());
    edit(&mut app, TileEdit::Copy);
    send(&mut app, EditorCommand::DeleteSelection);
    send(
        &mut app,
        EditorCommand::Focus {
            target: FocusTarget::EmptySlot {
                surface: root,
                slot: BoardSlot::new(4, 0),
            },
        },
    );
    edit(&mut app, TileEdit::Paste);
    let project = app.world().resource::<MusaicProject>();
    let new = app
        .world()
        .resource::<SelectionState>()
        .nodes
        .iter()
        .next()
        .unwrap();
    assert_ne!(new, &container);
    assert_eq!(
        project
            .document
            .graph
            .nodes_on_surface(project.document.graph.container_surface(new).unwrap())
            .len(),
        5
    );
    assert_eq!(app.world().resource::<CommandHistory>().undo_len(), 2);
}

#[test]
fn changing_project_clears_clipboard_to_prevent_dangling_sample_references() {
    let (mut app, _, _, ids) = fixture();
    select(&mut app, ids[0].clone());
    edit(&mut app, TileEdit::Copy);
    send(&mut app, EditorCommand::NewProject);
    let root = app
        .world()
        .resource::<MusaicProject>()
        .document
        .root_surface;
    send(
        &mut app,
        EditorCommand::Focus {
            target: FocusTarget::EmptySlot {
                surface: root,
                slot: BoardSlot::new(0, 0),
            },
        },
    );
    edit(&mut app, TileEdit::Paste);
    assert_eq!(
        app.world()
            .resource::<MusaicProject>()
            .document
            .graph
            .nodes()
            .count(),
        0
    );
}

#[test]
fn moving_containers_cannot_create_a_cycle() {
    use std::collections::BTreeMap;
    let mut project = MusaicProject::new_empty();
    let root = project.document.root_surface;
    let a = project
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
    let b = project
        .document
        .graph
        .insert_tile(
            &mut project.document.surfaces,
            root,
            PlacementAddress::BoardSlot(BoardSlot::new(5, 0)),
            TileSpawnKind::Container {
                kind: ContainerKind::Sequence,
            },
        )
        .unwrap();
    let graph = project.document.graph.clone();
    let into = |id: &NodeId| NodeLocation {
        surface: graph.container_surface(id).unwrap(),
        address: PlacementAddress::StackIndex(StackIndex(0)),
    };
    assert!(
        project
            .document
            .graph
            .relocate_nodes(
                &project.document.surfaces,
                &BTreeMap::from([(a.clone(), into(&b)), (b, into(&a))])
            )
            .is_err()
    );
    assert_eq!(project.document.graph, graph);
}

proptest::proptest! {
    #[test]
    fn relocation_and_its_inverse_preserve_the_graph(x in -100i32..100, y in -100i32..100) {
        use std::collections::BTreeMap;
        let mut project=MusaicProject::new_empty(); let root=project.document.root_surface;
        let id=project.document.graph.insert_tile(&mut project.document.surfaces,root,PlacementAddress::BoardSlot(BoardSlot::new(0,0)),TileSpawnKind::Container{kind:ContainerKind::Sequence}).unwrap();
        let before=project.document.graph.clone(); let old=before.location_of(&id).unwrap();
        project.document.graph.relocate_nodes(&project.document.surfaces,&BTreeMap::from([(id.clone(),NodeLocation{surface:root,address:PlacementAddress::BoardSlot(BoardSlot::new(x,y))})])).unwrap();
        project.document.graph.relocate_nodes(&project.document.surfaces,&BTreeMap::from([(id,old)])).unwrap();
        proptest::prop_assert_eq!(project.document.graph,before);
    }
}

#[test]
fn delete_selected_note_removes_owned_modifiers_and_redoes_without_selection() {
    let (mut app, _, surface, ids) = fixture();
    let before = layout(&app, surface);
    select(&mut app, ids[0].clone());
    send(&mut app, EditorCommand::DeleteSelection);
    assert_eq!(layout(&app, surface).len(), 2);
    send(&mut app, EditorCommand::Undo);
    assert_eq!(layout(&app, surface), before);
    send(&mut app, EditorCommand::ClearSelection);
    send(&mut app, EditorCommand::Redo);
    assert_eq!(layout(&app, surface).len(), 2);
}

#[test]
fn multi_selection_duplicate_keeps_every_selected_expression() {
    let (mut app, _, surface, ids) = fixture();
    select(&mut app, ids[0].clone());
    send(
        &mut app,
        EditorCommand::SelectNode {
            node: ids[3].clone(),
            mode: SelectionMode::Add,
        },
    );
    edit(&mut app, TileEdit::Duplicate);
    assert_eq!(layout(&app, surface).len(), 10);
}

#[test]
fn placed_atom_redo_keeps_identity_for_later_value_edits() {
    use musaic::application::editor::transaction::PlacementTarget;
    let (mut app, _, surface, _) = fixture();
    send(
        &mut app,
        EditorCommand::PlaceTile {
            target: PlacementTarget::StackIndex {
                surface,
                index: StackIndex(7),
            },
            tile: TileSpawnKind::Atom {
                atom: AtomValue::Number(2),
            },
        },
    );
    let placed = app
        .world()
        .resource::<SelectionState>()
        .nodes
        .iter()
        .next()
        .unwrap()
        .clone();
    send(
        &mut app,
        EditorCommand::SetAtomValue {
            node: placed.clone(),
            value: AtomValue::Number(9),
        },
    );
    send(&mut app, EditorCommand::Undo);
    send(&mut app, EditorCommand::Undo);
    assert!(
        !app.world()
            .resource::<MusaicProject>()
            .document
            .graph
            .contains_node(&placed)
    );
    send(&mut app, EditorCommand::Redo);
    send(&mut app, EditorCommand::Redo);
    let project = app.world().resource::<MusaicProject>();
    assert!(
        matches!(project.document.graph.node(&placed).map(|node| &node.kind), Some(DocumentNodeKind::Atom(atom)) if atom.atom == AtomValue::Number(9)),
        "redo must restore the exact identity targeted by the following edit"
    );
}

#[test]
fn placed_nested_container_redo_keeps_its_surface_and_later_child_identity() {
    use musaic::application::editor::transaction::PlacementTarget;
    let (mut app, _, parent_surface, _) = fixture();
    send(
        &mut app,
        EditorCommand::PlaceTile {
            target: PlacementTarget::StackIndex {
                surface: parent_surface,
                index: StackIndex(7),
            },
            tile: TileSpawnKind::Container {
                kind: ContainerKind::Sequence,
            },
        },
    );
    let container = app
        .world()
        .resource::<SelectionState>()
        .nodes
        .iter()
        .next()
        .unwrap()
        .clone();
    let surface = app
        .world()
        .resource::<MusaicProject>()
        .document
        .graph
        .container_surface(&container)
        .unwrap();
    send(
        &mut app,
        EditorCommand::EnterContainer {
            container: container.clone(),
        },
    );
    send(
        &mut app,
        EditorCommand::PlaceTile {
            target: PlacementTarget::StackIndex {
                surface,
                index: StackIndex(0),
            },
            tile: TileSpawnKind::Atom {
                atom: AtomValue::NoteName(NoteName::C),
            },
        },
    );
    let child = app
        .world()
        .resource::<SelectionState>()
        .nodes
        .iter()
        .next()
        .unwrap()
        .clone();
    send(
        &mut app,
        EditorCommand::SetAtomValue {
            node: child.clone(),
            value: AtomValue::NoteName(NoteName::G),
        },
    );
    for _ in 0..3 {
        send(&mut app, EditorCommand::Undo);
    }
    assert!(
        !app.world()
            .resource::<MusaicProject>()
            .document
            .graph
            .contains_node(&container)
    );
    for _ in 0..3 {
        send(&mut app, EditorCommand::Redo);
    }
    let project = app.world().resource::<MusaicProject>();
    assert_eq!(
        project.document.graph.container_surface(&container),
        Some(surface)
    );
    assert_eq!(
        project.document.graph.location_of(&child).unwrap().surface,
        surface
    );
    assert!(
        matches!(&project.document.graph.node(&child).unwrap().kind, DocumentNodeKind::Atom(atom) if atom.atom == AtomValue::NoteName(NoteName::G))
    );
}

#[test]
fn placement_history_restores_auto_connected_neighbor_bindings_in_both_directions() {
    use musaic::application::editor::transaction::PlacementTarget;
    let (mut app, container, _, _) = fixture();
    let root = app
        .world()
        .resource::<MusaicProject>()
        .document
        .root_surface;
    send(&mut app, EditorCommand::NavigateToSurface { surface: root });
    select(&mut app, container.clone());
    let before_connections = app
        .world()
        .resource::<MusaicProject>()
        .document
        .connections
        .clone();
    let before_visible = document::connections_from_program(
        &document::export_document_program(&app.world().resource::<MusaicProject>().document)
            .unwrap(),
    );
    send(
        &mut app,
        EditorCommand::PlaceTile {
            target: PlacementTarget::BoardSlot {
                surface: root,
                slot: BoardSlot::new(5, 0),
            },
            tile: TileSpawnKind::Output {
                name: "Main".into(),
            },
        },
    );
    let output = app
        .world()
        .resource::<SelectionState>()
        .nodes
        .iter()
        .next()
        .unwrap()
        .clone();
    let after_connections = app
        .world()
        .resource::<MusaicProject>()
        .document
        .connections
        .clone();
    let after_visible = document::connections_from_program(
        &document::export_document_program(&app.world().resource::<MusaicProject>().document)
            .unwrap(),
    );
    assert_ne!(
        before_visible, after_visible,
        "placement creates a visible source-to-output connection"
    );
    for _ in 0..2 {
        send(&mut app, EditorCommand::Undo);
        let project = app.world().resource::<MusaicProject>();
        assert_eq!(project.document.connections, before_connections);
        assert_eq!(
            document::connections_from_program(
                &document::export_document_program(&project.document).unwrap()
            ),
            before_visible
        );
        assert!(!project.document.graph.contains_node(&output));
        send(&mut app, EditorCommand::Redo);
        let project = app.world().resource::<MusaicProject>();
        assert_eq!(project.document.connections, after_connections);
        assert_eq!(
            document::connections_from_program(
                &document::export_document_program(&project.document).unwrap()
            ),
            after_visible
        );
        assert!(project.document.graph.contains_node(&output));
    }
}

#[test]
fn occupied_container_anchor_rejects_without_orphaning_nested_content() {
    use musaic::application::editor::transaction::PlacementTarget;
    let (mut app, container, surface, ids) = fixture();
    let before = layout(&app, surface);
    let history = app.world().resource::<CommandHistory>().undo_len();
    let root = app
        .world()
        .resource::<MusaicProject>()
        .document
        .root_surface;
    send(&mut app, EditorCommand::NavigateToSurface { surface: root });
    send(
        &mut app,
        EditorCommand::PlaceTile {
            target: PlacementTarget::BoardSlot {
                surface: root,
                slot: BoardSlot::new(0, 0),
            },
            tile: TileSpawnKind::Atom {
                atom: AtomValue::Number(2),
            },
        },
    );
    assert_eq!(layout(&app, surface), before);
    assert_eq!(
        app.world()
            .resource::<MusaicProject>()
            .document
            .graph
            .container_surface(&container),
        Some(surface)
    );
    assert!(ids.iter().all(|id| {
        app.world()
            .resource::<MusaicProject>()
            .document
            .graph
            .contains_node(id)
    }));
    assert_eq!(app.world().resource::<CommandHistory>().undo_len(), history);
}

#[test]
fn instrument_fragments_restore_sound_and_bindings_without_touching_unrelated_state() {
    use musaic::{
        application::editor::transaction::PlacementTarget,
        domain::instrument::{InstrumentDefinition, InstrumentSource, Waveform},
    };
    let (mut app, container, _, _) = fixture();
    let root = app
        .world()
        .resource::<MusaicProject>()
        .document
        .root_surface;
    send(&mut app, EditorCommand::NavigateToSurface { surface: root });
    select(&mut app, container.clone());
    send(
        &mut app,
        EditorCommand::PlaceTile {
            target: PlacementTarget::BoardSlot {
                surface: root,
                slot: BoardSlot::new(5, 0),
            },
            tile: TileSpawnKind::sound(InstrumentDefinition::default()),
        },
    );
    let output = app
        .world()
        .resource::<SelectionState>()
        .nodes
        .iter()
        .next()
        .unwrap()
        .clone();
    let instrument = InstrumentDefinition::new(InstrumentSource::Synth(Waveform::Square));
    // Sound design is part of the copied document node.
    {
        let mut project = app.world_mut().resource_mut::<MusaicProject>();
        project
            .set_sound_definition(&output, instrument.clone())
            .unwrap();
        project
            .sound_library
            .insert("Unrelated".into(), instrument.clone());
    }
    let bindings = {
        let project = app.world().resource::<MusaicProject>();
        let program = document::export_document_program(&project.document).unwrap();
        document::connection_policy::effective_bindings(&program, &output)
    };
    select(&mut app, output.clone());
    edit(&mut app, TileEdit::Duplicate);
    let duplicate = app
        .world()
        .resource::<SelectionState>()
        .nodes
        .iter()
        .next()
        .unwrap()
        .clone();
    assert_ne!(duplicate, output);
    for _ in 0..2 {
        let project = app.world().resource::<MusaicProject>();
        assert_eq!(project.sound_definition(&duplicate).unwrap(), &instrument);
        let program = document::export_document_program(&project.document).unwrap();
        assert_eq!(
            document::connection_policy::effective_bindings(&program, &duplicate),
            bindings
        );
        send(&mut app, EditorCommand::Undo);
        let project = app.world().resource::<MusaicProject>();
        assert!(project.sound_definition(&duplicate).is_none());
        assert!(!project.document.graph.contains_node(&duplicate));
        assert_eq!(project.sound_library["Unrelated"], instrument);
        assert_eq!(project.sound_definition(&output).unwrap(), &instrument);
        send(&mut app, EditorCommand::Redo);
    }
}
