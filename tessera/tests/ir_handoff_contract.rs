use std::collections::BTreeMap;

use tessera::prelude::{
    AtomTile, Container, ContainerAxis, ContainerId, ContainerKind, ContainerSurfaceTile,
    CycleDuration, CycleSpan, CycleTime, EventValue, FlowControlKind, FlowControlNode,
    FlowControlPolicy, InputEndpoint, InputPort, NodeId, NoteAtom, OutputNode, PatternEvent,
    PatternNodeIr, PortGroupId, PortMemberId, Rational, RootRelation, RootSurfaceNodeKind,
    StreamSource, StreamTarget, TesseraCompiler, TesseraProgram, TransformKind, TransformNode,
};

fn output_input(member: &str) -> InputEndpoint {
    InputEndpoint::GroupMember {
        group: PortGroupId::new("inputs"),
        member: PortMemberId::new(member),
    }
}

fn transform_input(port: &str) -> InputEndpoint {
    InputEndpoint::Socket(InputPort::new(port))
}

#[test]
fn slow_transform_emits_time_scale_ir() {
    let mut root_nodes = BTreeMap::new();
    let mut containers = BTreeMap::new();
    containers.insert(
        ContainerId::new("phrase"),
        Container {
            kind: ContainerKind::Sequence,
            axis: ContainerAxis::Time,
            stack: vec![ContainerSurfaceTile::Atom(AtomTile::Note(NoteAtom::new(
                "a",
            )))],
        },
    );
    root_nodes.insert(
        NodeId::new("phrase"),
        RootSurfaceNodeKind::Container {
            container: ContainerId::new("phrase"),
        },
    );
    root_nodes.insert(
        NodeId::new("slow"),
        RootSurfaceNodeKind::Transform(TransformNode::new(TransformKind::Slow)),
    );
    root_nodes.insert(
        NodeId::new("out"),
        RootSurfaceNodeKind::Output(OutputNode::default()),
    );
    let ir = TesseraCompiler::new()
        .compile_ir(&TesseraProgram {
            root_nodes,
            containers,
            relations: vec![
                RootRelation::FlowsTo {
                    from: StreamSource::node(NodeId::new("phrase")),
                    to: StreamTarget::TransformInput {
                        node: NodeId::new("slow"),
                        endpoint: transform_input("main"),
                    },
                },
                RootRelation::FlowsTo {
                    from: StreamSource::node(NodeId::new("slow")),
                    to: StreamTarget::OutputInput {
                        node: NodeId::new("out"),
                        endpoint: output_input("main"),
                    },
                },
            ],
        })
        .expect("program should compile");

    assert!(matches!(
        ir.outputs[0].root,
        PatternNodeIr::TimeScale { .. }
    ));
}

#[test]
fn mask_flow_emits_mask_clip_ir() {
    let mut root_nodes = BTreeMap::new();
    let mut containers = BTreeMap::new();
    containers.insert(
        ContainerId::new("main"),
        Container {
            kind: ContainerKind::Sequence,
            axis: ContainerAxis::Time,
            stack: vec![ContainerSurfaceTile::Atom(AtomTile::Note(NoteAtom::new(
                "a",
            )))],
        },
    );
    containers.insert(
        ContainerId::new("mask_src"),
        Container {
            kind: ContainerKind::Sequence,
            axis: ContainerAxis::Time,
            stack: vec![ContainerSurfaceTile::Atom(AtomTile::Scalar(
                tessera::prelude::ScalarAtom::integer(1),
            ))],
        },
    );
    root_nodes.insert(
        NodeId::new("main"),
        RootSurfaceNodeKind::Container {
            container: ContainerId::new("main"),
        },
    );
    root_nodes.insert(
        NodeId::new("mask_src"),
        RootSurfaceNodeKind::Container {
            container: ContainerId::new("mask_src"),
        },
    );
    root_nodes.insert(
        NodeId::new("mask_flow"),
        RootSurfaceNodeKind::FlowControl(
            FlowControlNode::new(FlowControlKind::Mask).with_policy(FlowControlPolicy::MaskClip),
        ),
    );
    root_nodes.insert(
        NodeId::new("out"),
        RootSurfaceNodeKind::Output(OutputNode::default()),
    );
    let ir = TesseraCompiler::new()
        .compile_ir(&TesseraProgram {
            root_nodes,
            containers,
            relations: vec![
                RootRelation::FlowsTo {
                    from: StreamSource::node(NodeId::new("main")),
                    to: StreamTarget::FlowControlInput {
                        node: NodeId::new("mask_flow"),
                        endpoint: transform_input("main"),
                    },
                },
                RootRelation::FlowsTo {
                    from: StreamSource::node(NodeId::new("mask_src")),
                    to: StreamTarget::FlowControlInput {
                        node: NodeId::new("mask_flow"),
                        endpoint: transform_input("mask"),
                    },
                },
                RootRelation::FlowsTo {
                    from: StreamSource::node(NodeId::new("mask_flow")),
                    to: StreamTarget::OutputInput {
                        node: NodeId::new("out"),
                        endpoint: output_input("main"),
                    },
                },
            ],
        })
        .expect("mask program should compile");

    assert!(matches!(ir.outputs[0].root, PatternNodeIr::MaskClip { .. }));
}

#[test]
fn three_chained_containers_emit_concat_ir() {
    let mut root_nodes = BTreeMap::new();
    let mut containers = BTreeMap::new();
    for id in ["a", "b", "c"] {
        containers.insert(
            ContainerId::new(id),
            Container {
                kind: ContainerKind::Sequence,
                axis: ContainerAxis::Time,
                stack: vec![ContainerSurfaceTile::Atom(AtomTile::Note(NoteAtom::new(
                    id,
                )))],
            },
        );
        root_nodes.insert(
            NodeId::new(id),
            RootSurfaceNodeKind::Container {
                container: ContainerId::new(id),
            },
        );
    }
    root_nodes.insert(
        NodeId::new("out"),
        RootSurfaceNodeKind::Output(OutputNode::default()),
    );
    let ir = TesseraCompiler::new()
        .compile_ir(&TesseraProgram {
            root_nodes,
            containers,
            relations: vec![
                RootRelation::ChainedTo {
                    from: StreamSource::node(NodeId::new("a")),
                    to: NodeId::new("b"),
                },
                RootRelation::ChainedTo {
                    from: StreamSource::node(NodeId::new("b")),
                    to: NodeId::new("c"),
                },
                RootRelation::FlowsTo {
                    from: StreamSource::node(NodeId::new("c")),
                    to: StreamTarget::OutputInput {
                        node: NodeId::new("out"),
                        endpoint: output_input("main"),
                    },
                },
            ],
        })
        .expect("chained program should compile");

    fn contains_concat(node: &PatternNodeIr) -> bool {
        match node {
            PatternNodeIr::Concat { children } => children.len() >= 2,
            PatternNodeIr::TimeScale { inner, .. }
            | PatternNodeIr::Shift { inner, .. }
            | PatternNodeIr::ReflectCycle { inner }
            | PatternNodeIr::Degrade { inner, .. }
            | PatternNodeIr::Deduplicate { inner, .. } => contains_concat(inner),
            PatternNodeIr::MaskClip { source, .. } => contains_concat(source),
            PatternNodeIr::Merge { children }
            | PatternNodeIr::CycleRoute { children }
            | PatternNodeIr::CycleSlots { children } => children.iter().any(contains_concat),
            _ => false,
        }
    }

    assert!(contains_concat(&ir.outputs[0].root));
    let events = ir.outputs[0].events();
    assert_eq!(events.len(), 3);
}

#[test]
fn compiled_events_carry_container_provenance() {
    let mut containers = BTreeMap::new();
    containers.insert(
        ContainerId::new("kick_phrase"),
        Container {
            kind: ContainerKind::Sequence,
            axis: ContainerAxis::Time,
            stack: vec![ContainerSurfaceTile::Atom(AtomTile::Note(NoteAtom::new(
                "a",
            )))],
        },
    );
    let mut root_nodes = BTreeMap::new();
    root_nodes.insert(
        NodeId::new("kick_phrase"),
        RootSurfaceNodeKind::Container {
            container: ContainerId::new("kick_phrase"),
        },
    );
    root_nodes.insert(
        NodeId::new("out"),
        RootSurfaceNodeKind::Output(OutputNode::default()),
    );
    let ir = TesseraCompiler::new()
        .compile_ir(&TesseraProgram {
            root_nodes,
            containers,
            relations: vec![RootRelation::FlowsTo {
                from: StreamSource::node(NodeId::new("kick_phrase")),
                to: StreamTarget::OutputInput {
                    node: NodeId::new("out"),
                    endpoint: output_input("main"),
                },
            }],
        })
        .expect("program should compile");
    let events = ir.outputs[0].events();
    let source = events
        .first()
        .and_then(|event| event.source.as_ref())
        .expect("event should carry provenance");
    assert_eq!(source.container, Some(ContainerId::new("kick_phrase")));
}

#[test]
fn pattern_event_source_deserializes_without_field() {
    let event = PatternEvent::new(
        CycleSpan {
            start: CycleTime(Rational::zero()),
            duration: CycleDuration(Rational::one()),
        },
        EventValue::Note {
            value: "a".to_string(),
            octave: None,
        },
    );
    let json = serde_json::to_string(&event).expect("serialize");
    let decoded: PatternEvent = serde_json::from_str(&json).expect("deserialize");
    assert!(decoded.source.is_none());
}

#[test]
fn gain_with_pattern_modulator_emits_merge_with_control_stream() {
    let mut root_nodes = BTreeMap::new();
    let mut containers = BTreeMap::new();
    containers.insert(
        ContainerId::new("main"),
        Container {
            kind: ContainerKind::Sequence,
            axis: ContainerAxis::Time,
            stack: vec![ContainerSurfaceTile::Atom(AtomTile::Note(NoteAtom::new(
                "a",
            )))],
        },
    );
    containers.insert(
        ContainerId::new("mod"),
        Container {
            kind: ContainerKind::Sequence,
            axis: ContainerAxis::Time,
            stack: vec![
                ContainerSurfaceTile::Atom(AtomTile::Scalar(
                    tessera::prelude::ScalarAtom::integer(2),
                )),
                ContainerSurfaceTile::Atom(AtomTile::Scalar(
                    tessera::prelude::ScalarAtom::integer(4),
                )),
            ],
        },
    );
    root_nodes.insert(
        NodeId::new("main"),
        RootSurfaceNodeKind::Container {
            container: ContainerId::new("main"),
        },
    );
    root_nodes.insert(
        NodeId::new("mod"),
        RootSurfaceNodeKind::Container {
            container: ContainerId::new("mod"),
        },
    );
    root_nodes.insert(
        NodeId::new("gain"),
        RootSurfaceNodeKind::Transform(TransformNode::new(TransformKind::Gain)),
    );
    root_nodes.insert(
        NodeId::new("out"),
        RootSurfaceNodeKind::Output(OutputNode::default()),
    );
    let ir = TesseraCompiler::new()
        .compile_ir(&TesseraProgram {
            root_nodes,
            containers,
            relations: vec![
                RootRelation::FlowsTo {
                    from: StreamSource::node(NodeId::new("main")),
                    to: StreamTarget::TransformInput {
                        node: NodeId::new("gain"),
                        endpoint: transform_input("main"),
                    },
                },
                RootRelation::FlowsTo {
                    from: StreamSource::node(NodeId::new("mod")),
                    to: StreamTarget::TransformInput {
                        node: NodeId::new("gain"),
                        endpoint: transform_input("amount"),
                    },
                },
                RootRelation::FlowsTo {
                    from: StreamSource::node(NodeId::new("gain")),
                    to: StreamTarget::OutputInput {
                        node: NodeId::new("out"),
                        endpoint: output_input("main"),
                    },
                },
            ],
        })
        .expect("gain modulation should compile");

    fn contains_control_merge(node: &PatternNodeIr) -> bool {
        match node {
            PatternNodeIr::Merge { children } => {
                children.iter().any(|child| {
                    matches!(child, PatternNodeIr::ControlStream(_))
                        || matches!(child, PatternNodeIr::EventStream(_))
                }) && children.len() >= 2
            }
            PatternNodeIr::TimeScale { inner, .. } | PatternNodeIr::ReflectCycle { inner } => {
                contains_control_merge(inner)
            }
            _ => false,
        }
    }

    assert!(contains_control_merge(&ir.outputs[0].root));
}

#[test]
fn pattern_ir_round_trips_through_serde_json() {
    let mut root_nodes = BTreeMap::new();
    let mut containers = BTreeMap::new();
    containers.insert(
        ContainerId::new("phrase"),
        Container {
            kind: ContainerKind::Sequence,
            axis: ContainerAxis::Time,
            stack: vec![ContainerSurfaceTile::Atom(AtomTile::Note(NoteAtom::new(
                "a",
            )))],
        },
    );
    root_nodes.insert(
        NodeId::new("phrase"),
        RootSurfaceNodeKind::Container {
            container: ContainerId::new("phrase"),
        },
    );
    root_nodes.insert(
        NodeId::new("out"),
        RootSurfaceNodeKind::Output(OutputNode::default()),
    );
    let ir = TesseraCompiler::new()
        .compile_ir(&TesseraProgram {
            root_nodes,
            containers,
            relations: vec![RootRelation::FlowsTo {
                from: StreamSource::node(NodeId::new("phrase")),
                to: StreamTarget::OutputInput {
                    node: NodeId::new("out"),
                    endpoint: output_input("main"),
                },
            }],
        })
        .expect("program should compile");
    let json = serde_json::to_string(&ir).expect("serialize");
    let decoded: tessera::domain::PatternIr = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(ir, decoded);
}
