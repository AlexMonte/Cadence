use tessera::prelude::*;
fn r(n: i64, d: i64) -> Rational {
    Rational::new(n, d)
}
fn board(kind: TransformKind, values: Vec<ContainerSurfaceTile>) -> AuthoredTesseraProgram {
    let mut board = Board::new();
    board
        .at(0, 2)
        .named("note")
        .footprint(TileFootprint::new(2, 2))
        .sequence(notes(["c"]))
        .unwrap();
    board.at(2, 2).named("effect").transform(kind).unwrap();
    board
        .at(3, 2)
        .named("out")
        .footprint(TileFootprint::new(2, 2))
        .output()
        .unwrap();
    let values = board
        .at(2, 0)
        .named("values")
        .footprint(TileFootprint::new(2, 2))
        .sequence(values)
        .unwrap();
    board
        .bind_output_side(
            &values,
            OutputEndpoint::Socket(OutputPort::new("out")),
            SpatialSide::South,
        )
        .unwrap();
    board.finish()
}
fn tile(value: AtomModifier) -> ContainerSurfaceTile {
    ContainerSurfaceTile::Atom(AtomTile::Modifier(value))
}
#[test]
fn complete_effect_tiles_can_own_control_time_slots() {
    for (kind, a, b) in [
        (
            TransformKind::Delay,
            AtomModifier::Delay(DelayParameters::default()),
            AtomModifier::Delay(DelayParameters {
                amount: r(3, 4),
                time: r(1, 8),
                ..Default::default()
            }),
        ),
        (
            TransformKind::Reverb,
            AtomModifier::Reverb(ReverbParameters::default()),
            AtomModifier::Reverb(ReverbParameters {
                amount: r(3, 4),
                decay: r(3, 1),
                ..Default::default()
            }),
        ),
        (
            TransformKind::Compressor,
            AtomModifier::Compressor(CompressorParameters::default()),
            AtomModifier::Compressor(CompressorParameters {
                knee_db: Rational::zero(),
                threshold: r(1, 8),
                ratio: r(12, 1),
                ..Default::default()
            }),
        ),
    ] {
        let authored = board(kind, vec![tile(a), tile(b)]);
        let encoded = serde_json::to_vec(&authored).unwrap();
        let restored: AuthoredTesseraProgram = serde_json::from_slice(&encoded).unwrap();
        let ir = TesseraCompiler::new()
            .compile_authored(&restored)
            .unwrap()
            .ir;
        for cycle in [0, 17] {
            let events = ir.outputs[0]
                .root
                .query(CycleSpan::new(
                    CycleTime(r(cycle, 1)),
                    CycleDuration(Rational::one()),
                ))
                .events;
            let controls: Vec<_> = events
                .iter()
                .filter(|event| !event.fields.is_empty())
                .collect();
            assert_eq!(controls.len(), 2, "{kind:?}: {events:?}");
            assert_ne!(controls[0].fields, controls[1].fields);
            assert_eq!(controls[0].span.duration.0, r(1, 2));
            assert_eq!(controls[1].span.start.0, r(cycle, 1) + r(1, 2));
        }
    }
}
#[test]
fn typed_effect_inputs_reject_another_effect_or_a_number() {
    for bad in [
        tile(AtomModifier::Reverb(ReverbParameters::default())),
        ContainerSurfaceTile::Atom(AtomTile::Scalar(ScalarAtom::integer(1))),
    ] {
        let authored = board(
            TransformKind::Delay,
            vec![tile(AtomModifier::Delay(DelayParameters::default())), bad],
        );
        assert!(TesseraCompiler::new().compile_authored(&authored).is_err());
    }
}
#[test]
fn rests_keep_the_type_and_gaps_of_control_patterns() {
    for (kind, value) in [
        (
            TransformKind::Delay,
            tile(AtomModifier::Delay(DelayParameters::default())),
        ),
        (
            TransformKind::Expression,
            ContainerSurfaceTile::Atom(AtomTile::Scalar(ScalarAtom { value: r(1, 4) })),
        ),
    ] {
        let authored = board(
            kind,
            vec![value, ContainerSurfaceTile::Atom(AtomTile::Rest)],
        );
        assert!(
            TesseraCompiler::new().compile_authored(&authored).is_ok(),
            "{kind:?}"
        );
    }
}

#[test]
fn modulation_patterns_validate_their_target_and_every_owned_range() {
    for (parameter, kind) in [
        (ParameterKey::Gain, TransformKind::Gain),
        (ParameterKey::Velocity, TransformKind::Velocity),
        (ParameterKey::PlaybackRate, TransformKind::PlaybackRate),
        (ParameterKey::LowPassCutoff, TransformKind::LowPassCutoff),
        (ParameterKey::Transpose, TransformKind::Transpose),
    ] {
        let value = ModulationParameters::for_parameter(parameter);
        let modulation = |value| tile(AtomModifier::Modulation { parameter, value });
        let authored = board(
            kind,
            vec![
                modulation(value),
                ContainerSurfaceTile::Atom(AtomTile::Rest),
            ],
        );
        let compiled = TesseraCompiler::new().compile_authored(&authored);
        assert!(compiled.is_ok(), "{parameter:?}: {compiled:?}");
        let reversed = ModulationParameters {
            minimum: r(3, 1),
            maximum: r(2, 1),
            ..value
        };
        let authored = board(kind, vec![modulation(value), modulation(reversed)]);
        assert!(
            TesseraCompiler::new().compile_authored(&authored).is_err(),
            "{parameter:?}"
        );
    }
    let gain = tile(AtomModifier::Modulation {
        parameter: ParameterKey::Gain,
        value: ModulationParameters::default(),
    });
    assert!(
        TesseraCompiler::new()
            .compile_authored(&board(TransformKind::LowPassCutoff, vec![gain]))
            .is_err()
    );
}
