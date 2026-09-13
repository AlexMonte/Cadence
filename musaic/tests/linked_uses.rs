#[path = "support/acceptance.rs"]
mod support;
use musaic::{
    MusaicProject,
    application::{
        trick_uses::{linked_uses, source_owners},
        tricks,
    },
    domain::{
        board::BoardSlot,
        document::{
            ContainerKind, GraphTilePrototypeId, PlacementAddress, StackIndex, TileSpawnKind,
        },
    },
};
use tessera::prelude::NodeId;
fn add(p: &mut MusaicProject, x: i32, y: i32, tile: TileSpawnKind) -> NodeId {
    p.document
        .graph
        .insert_tile(
            &mut p.document.surfaces,
            p.document.root_surface,
            PlacementAddress::BoardSlot(BoardSlot::new(x, y)),
            tile,
        )
        .unwrap()
}
#[test]
fn lists_every_authored_link_once_with_nested_location_and_shared_note_ownership() {
    let (mut p, source, _) = support::notes();
    let child = p
        .document
        .graph
        .nodes_on_surface(p.document.graph.container_surface(&source).unwrap())[0]
        .1
        .id
        .clone();
    let id = tricks::define(&mut p.document, source.clone(), "Theme".into(), None).unwrap();
    let a = add(
        &mut p,
        10,
        2,
        TileSpawnKind::TrickInstance {
            prototype: GraphTilePrototypeId(id),
        },
    );
    let host = add(
        &mut p,
        -8,
        5,
        TileSpawnKind::Container {
            kind: ContainerKind::Sequence,
        },
    );
    let nested = p
        .document
        .graph
        .insert_tile(
            &mut p.document.surfaces,
            p.document.graph.container_surface(&host).unwrap(),
            PlacementAddress::StackIndex(StackIndex(3)),
            TileSpawnKind::TrickInstance {
                prototype: GraphTilePrototypeId(id),
            },
        )
        .unwrap();
    let other = tricks::define(&mut p.document, host.clone(), "Arrangement".into(), None).unwrap();
    add(
        &mut p,
        15,
        2,
        TileSpawnKind::TrickInstance {
            prototype: GraphTilePrototypeId(other),
        },
    );
    let uses = linked_uses(&p.document, id).unwrap();
    assert_eq!(
        uses.tiles.len(),
        2,
        "authored tiles, not expanded calls to the containing arrangement"
    );
    assert!(uses.tiles.iter().any(|t| t.node == a));
    let label = &uses
        .tiles
        .iter()
        .find(|t| t.node == nested)
        .unwrap()
        .location;
    assert!(
        label.contains("Tile 4") && label.contains("Pattern board"),
        "{label}"
    );
    assert_eq!(
        source_owners(&p.document, &child),
        vec![(id, "Theme".into(), 2)]
    );
    assert_eq!(
        tricks::paint(&p, &child).linked_sources,
        source_owners(&p.document, &child)
    );
    assert!(source_owners(&p.document, &a).is_empty());
    let report = linked_uses(&p.document, other).unwrap();
    assert_eq!(report.tiles.len(), 1);
    assert!(linked_uses(&p.document, 9999).is_none());
    let paint = tricks::paint(&p, &a);
    assert_eq!(paint.linked_count, 2);
    assert_eq!(paint.definition.unwrap().2, source);
}

#[test]
fn undo_and_redo_of_a_shared_name_keep_the_inspected_source_note() {
    use musaic::application::{
        command::{EditorCommand as C, editing::TileEdit},
        editor::{EditorAttention, FocusTarget, SelectionMode},
    };
    let (mut p, source, _) = support::notes();
    let surface = p.document.graph.container_surface(&source).unwrap();
    let note = p.document.graph.nodes_on_surface(surface)[0].1.id.clone();
    let id = tricks::define(&mut p.document, source, "Theme".into(), None).unwrap();
    let mut app = support::editor(p);
    support::send(&mut app, C::NavigateToSurface { surface });
    support::send(
        &mut app,
        C::SelectNode {
            node: note.clone(),
            mode: SelectionMode::Replace,
        },
    );
    support::send(
        &mut app,
        C::EditTiles(TileEdit::RenameTrick {
            id,
            name: "New theme".into(),
        }),
    );
    for (command, name) in [(C::Undo, "Theme"), (C::Redo, "New theme")] {
        support::send(&mut app, command);
        let attention = app.world().resource::<EditorAttention>();
        assert_eq!(attention.focus, FocusTarget::Tile { node: note.clone() });
        assert_eq!(attention.active_board(), surface);
        assert_eq!(
            app.world().resource::<MusaicProject>().document.tricks[&id].name,
            name
        );
    }
}
