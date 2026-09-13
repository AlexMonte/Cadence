//! A host can choose compatible ports before the complete board is connected.
use tessera::prelude::*;
fn output() -> OutputEndpoint {
    OutputEndpoint::Socket(OutputPort::new("out"))
}
#[test]
fn incomplete_unrelated_patterns_do_not_prevent_note_scalar_and_effect_port_inspection() {
    let mut board = Board::new();
    board
        .at(0, 0)
        .named("notes")
        .sequence(SequenceStack::new().note("c").octave(4).build())
        .unwrap();
    board
        .at(2, 0)
        .named("values")
        .sequence(SequenceStack::new().scalar(2).scalar(3).build())
        .unwrap();
    board
        .at(4, 0)
        .named("unfinished")
        .sequence(vec![ContainerSurfaceTile::Atom(AtomTile::Operator(
            AtomOperatorToken::Fast,
        ))])
        .unwrap();
    board.at(6, 0).named("out").output().unwrap();
    let program = board.finish();
    let compiler = TesseraCompiler::new();
    assert_eq!(
        compiler.authored_output_shape(&program, &NodeId::new("notes"), &output()),
        Some(StreamShape::NotePattern)
    );
    assert_eq!(
        compiler.authored_output_shape(&program, &NodeId::new("values"), &output()),
        Some(StreamShape::ScalarPattern)
    );
    assert_eq!(
        compiler.authored_output_shape(&program, &NodeId::new("unfinished"), &output()),
        None
    );
    assert_eq!(
        compiler.authored_output_shape(&program, &NodeId::new("out"), &output()),
        None
    );
    assert!(TesseraCompiler::stream_shape_compatible(
        StreamShape::ScalarPattern,
        StreamShape::ControlPattern
    ));
    assert!(!TesseraCompiler::stream_shape_compatible(
        StreamShape::NotePattern,
        StreamShape::ScalarPattern
    ));
}
#[test]
fn custom_flow_socket_shapes_and_member_names_remain_authoritative() {
    let mut flow = FlowControlNode::new(FlowControlKind::Layer);
    flow.signature.output_sockets[0].port = OutputPort::new("meter");
    flow.signature.output_sockets[0].shape = StreamShape::ScalarPattern;
    let mut b = Board::new();
    b.at(0, 0).named("custom").flow_control_node(flow).unwrap();
    let program = b.finish();
    let compiler = TesseraCompiler::new();
    assert_eq!(
        compiler.authored_output_shape(
            &program,
            &NodeId::new("custom"),
            &OutputEndpoint::Socket(OutputPort::new("meter"))
        ),
        Some(StreamShape::ScalarPattern)
    );
    assert_eq!(
        compiler.authored_output_shape(&program, &NodeId::new("custom"), &output()),
        None
    );
}
#[test]
fn cyclic_nested_container_shapes_fail_without_recursing_forever() {
    let mut b = Board::new();
    b.at(0, 0)
        .named("nested")
        .sequence(SequenceStack::new().build())
        .unwrap();
    let mut program = b.finish();
    let RootSurfaceNodeKind::Container { container } =
        program.root_surface.nodes[&NodeId::new("nested")].clone()
    else {
        unreachable!()
    };
    program
        .containers
        .get_mut(&container)
        .unwrap()
        .stack
        .push(ContainerSurfaceTile::NestedContainer(container));
    assert_eq!(
        TesseraCompiler::new().authored_output_shape(&program, &NodeId::new("nested"), &output()),
        None
    );
}
