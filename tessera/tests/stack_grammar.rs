use tessera::prelude::{
    AtomOperatorToken, AtomTile, ContainerSurfaceTile, NoteAtom, NoteValue, Rational, ScalarAtom,
    StackCompound, StackPiece, stack_compound_from_tiles,
};

#[test]
fn stack_compound_roundtrips_from_linear_tiles() {
    let tiles = vec![
        ContainerSurfaceTile::Atom(AtomTile::Note(NoteAtom::new("e"))),
        ContainerSurfaceTile::Atom(AtomTile::Scalar(ScalarAtom::integer(2))),
        ContainerSurfaceTile::Atom(AtomTile::Operator(AtomOperatorToken::Elongate)),
        ContainerSurfaceTile::Atom(AtomTile::Scalar(ScalarAtom::integer(2))),
    ];

    let compound = stack_compound_from_tiles(&tiles, 0);
    let resolved = compound.resolve().expect("compound resolves");
    assert_eq!(resolved.modifiers.len(), 1);

    let mut manual = StackCompound::new();
    for piece in [
        StackPiece::note(NoteValue::E, "e"),
        StackPiece::Operator(AtomOperatorToken::Elongate),
        StackPiece::Scalar(Rational::from_integer(2)),
        StackPiece::Scalar(Rational::from_integer(2)),
    ] {
        manual.try_push(piece).expect("push");
    }
    assert_eq!(resolved, manual.resolve().expect("manual resolves"));
}

#[test]
fn stack_compound_permuted_tile_order_matches_manual() {
    let permuted = vec![
        ContainerSurfaceTile::Atom(AtomTile::Note(NoteAtom::new("e"))),
        ContainerSurfaceTile::Atom(AtomTile::Operator(AtomOperatorToken::Elongate)),
        ContainerSurfaceTile::Atom(AtomTile::Scalar(ScalarAtom::integer(2))),
        ContainerSurfaceTile::Atom(AtomTile::Scalar(ScalarAtom::integer(2))),
    ];
    let canonical = vec![
        ContainerSurfaceTile::Atom(AtomTile::Note(NoteAtom::new("e"))),
        ContainerSurfaceTile::Atom(AtomTile::Scalar(ScalarAtom::integer(2))),
        ContainerSurfaceTile::Atom(AtomTile::Operator(AtomOperatorToken::Elongate)),
        ContainerSurfaceTile::Atom(AtomTile::Scalar(ScalarAtom::integer(2))),
    ];

    assert_eq!(
        stack_compound_from_tiles(&permuted, 0)
            .resolve()
            .expect("permuted"),
        stack_compound_from_tiles(&canonical, 0)
            .resolve()
            .expect("canonical")
    );
}
