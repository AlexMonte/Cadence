use tessera::prelude::*;

fn id(name: &str) -> NodeId {
    NodeId::new(name)
}
fn out() -> OutputEndpoint {
    OutputEndpoint::Socket(OutputPort::new("out"))
}
fn main_input() -> InputEndpoint {
    InputEndpoint::Socket(InputPort::new("main"))
}

fn routed(stack: Vec<ContainerSurfaceTile>, wires: bool) -> AuthoredTesseraProgram {
    let mut b = Board::new();
    b.at(0, 0).named("source").sequence(stack).unwrap();
    if wires {
        b.at(1, 0)
            .named("straight")
            .transform(TransformKind::Wire)
            .unwrap();
        let turn = b
            .at(2, 0)
            .named("turn")
            .transform(TransformKind::Wire)
            .unwrap();
        b.bind_output_side(&turn, out(), SpatialSide::South)
            .unwrap();
        let sink = b.at(2, 1).named("sink").output().unwrap();
        b.bind_input_side(
            &sink,
            InputEndpoint::GroupMember {
                group: PortGroupId::new("inputs"),
                member: PortMemberId::new("main"),
            },
            SpatialSide::North,
        )
        .unwrap();
    } else {
        b.at(1, 0).named("sink").output().unwrap();
    }
    let mut program = b.finish();
    if let RootSurfaceNodeKind::Output(sink) =
        program.root_surface.nodes.get_mut(&id("sink")).unwrap()
    {
        sink.signature.input_groups[0].shape = StreamShape::Any;
    }
    program
}

#[test]
fn straight_and_bent_wires_preserve_notes_values_and_effects_exactly() {
    let compiler = TesseraCompiler::new();
    for (stack, shape) in [
        (
            SequenceStack::new()
                .note("c")
                .octave(4)
                .note("e")
                .octave(4)
                .build(),
            StreamShape::NotePattern,
        ),
        (
            SequenceStack::new().scalar(2).scalar(3).build(),
            StreamShape::ScalarPattern,
        ),
        (
            vec![ContainerSurfaceTile::Atom(AtomTile::Modifier(
                AtomModifier::Delay(DelayParameters::default()),
            ))],
            StreamShape::ControlPattern,
        ),
    ] {
        let direct = compiler
            .compile_authored(&routed(stack.clone(), false))
            .unwrap()
            .ir;
        let program = routed(stack, true);
        assert_eq!(
            compiler.authored_output_shape(&program, &id("turn"), &out()),
            Some(shape)
        );
        let saved: AuthoredTesseraProgram =
            serde_json::from_slice(&serde_json::to_vec(&program).unwrap()).unwrap();
        let with_wires = compiler.compile_authored(&saved).unwrap().ir;
        for cycle in [-1, 0, 11] {
            let span = CycleSpan::new(
                CycleTime(Rational::from_integer(cycle)),
                CycleDuration(Rational::one()),
            );
            assert_eq!(
                direct.outputs[0].root.query(span).events,
                with_wires.outputs[0].root.query(span).events
            );
        }
    }
}

#[test]
fn wire_preserves_type_checks_at_the_receiving_input() {
    let mut b = Board::new();
    b.at(0, 0).named("notes").sequence(notes(["c"])).unwrap();
    let wire = b
        .at(1, 0)
        .named("wire")
        .transform(TransformKind::Wire)
        .unwrap();
    b.bind_output_side(&wire, out(), SpatialSide::South)
        .unwrap();
    b.at(0, 1).named("main").sequence(notes(["e"])).unwrap();
    b.at(1, 1)
        .named("fast")
        .transform(TransformKind::Fast)
        .unwrap();
    b.at(2, 1).named("out").output().unwrap();
    // A note stream routed through a wire is still not a numeric speed factor.
    assert!(
        TesseraCompiler::new()
            .compile_authored(&b.finish())
            .is_err()
    );
}

#[test]
fn explicit_input_wins_over_a_different_spatial_neighbor() {
    let mut program = routed(notes(["c"]), true);
    let mut extra = Board::new();
    extra
        .at(10, 10)
        .named("chosen")
        .sequence(notes(["g"]))
        .unwrap();
    let extra = extra.finish();
    program.containers.extend(extra.containers);
    program.root_surface.nodes.extend(extra.root_surface.nodes);
    program
        .root_surface
        .placements
        .extend(extra.root_surface.placements);
    program
        .root_surface
        .bindings
        .extend(extra.root_surface.bindings);
    program
        .root_surface
        .explicit_relations
        .push(RootRelation::FlowsTo {
            from: StreamSource::node(id("chosen")),
            to: StreamTarget::TransformInput {
                node: id("straight"),
                endpoint: main_input(),
            },
        });
    let compiled = TesseraCompiler::new()
        .compile_authored(&program)
        .unwrap()
        .ir;
    let expected = TesseraCompiler::new()
        .compile_authored(&routed(notes(["g"]), false))
        .unwrap()
        .ir;
    let span = CycleSpan::new(CycleTime(Rational::zero()), CycleDuration(Rational::one()));
    let actual = compiled.outputs[0].root.query(span).events;
    assert_eq!(actual.len(), 1);
    assert_eq!(
        actual[0].source.as_ref().unwrap().container,
        Some(ContainerId::new("chosen"))
    );
    assert_eq!(
        actual[0].value,
        expected.outputs[0].root.query(span).events[0].value
    );
}

#[test]
fn numeric_wire_drives_a_real_speed_input_with_other_unfinished_neighbors() {
    let mut b = Board::new();
    let scalar = b
        .at(0, 0)
        .named("factor")
        .scalar(Rational::from_integer(3))
        .unwrap();
    let wire = b
        .at(1, 0)
        .named("wire")
        .transform(TransformKind::Wire)
        .unwrap();
    b.bind_output_side(&wire, out(), SpatialSide::South)
        .unwrap();
    b.at(0, 1).named("notes").sequence(notes(["c"])).unwrap();
    b.at(1, 1)
        .named("fast")
        .transform(TransformKind::Fast)
        .unwrap();
    b.at(2, 1).named("sink").output().unwrap();
    let program = b.clone().finish();
    let ir = TesseraCompiler::new()
        .compile_authored(&program)
        .unwrap()
        .ir;
    let span = CycleSpan::new(CycleTime(Rational::zero()), CycleDuration(Rational::one()));
    assert_eq!(
        ir.outputs[0]
            .root
            .query(span)
            .events
            .iter()
            .map(|event| event.span.start.0)
            .collect::<Vec<_>>(),
        vec![Rational::zero(), Rational::new(1, 3), Rational::new(2, 3)]
    );
    // Type inspection only follows this wire's source, even while another
    // nearby input is temporarily facing an output that has not been enabled.
    b.bind_output_side(&scalar, out(), SpatialSide::East)
        .unwrap();
    let unfinished = b
        .at(1, -1)
        .named("unfinished")
        .transform(TransformKind::Gain)
        .unwrap();
    b.bind_input_side(&unfinished, main_input(), SpatialSide::South)
        .unwrap();
    assert_eq!(
        TesseraCompiler::new().authored_output_shape(&b.finish(), &id("wire"), &out()),
        Some(StreamShape::ScalarPattern)
    );
}
