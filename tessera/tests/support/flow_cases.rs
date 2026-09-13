use std::collections::BTreeMap;
use tessera::prelude::*;
fn id(s: &str) -> NodeId {
    NodeId::new(s)
}
fn note(s: &str, octave: i64) -> ContainerSurfaceTile {
    ContainerSurfaceTile::Atom(AtomTile::Note(NoteAtom::new(s).with_octave(octave)))
}
fn scalar(n: i64) -> ContainerSurfaceTile {
    ContainerSurfaceTile::Atom(AtomTile::Scalar(ScalarAtom::integer(n)))
}
fn input(group: &str, member: &str) -> InputEndpoint {
    InputEndpoint::GroupMember {
        group: PortGroupId::new(group),
        member: PortMemberId::new(member),
    }
}
pub fn span(n: i64, d: i64, len_n: i64, len_d: i64) -> CycleSpan {
    CycleSpan::new(
        CycleTime(Rational::new(n, d)),
        CycleDuration(Rational::new(len_n, len_d)),
    )
}
pub fn policies() -> Vec<(FlowControlKind, FlowControlPolicy)> {
    use FlowControlKind as K;
    use FlowControlPolicy as P;
    vec![
        (K::Layer, P::Layer),
        (K::Merge, P::MergeAppend),
        (K::Merge, P::MergeInterleave),
        (K::Merge, P::MergePriority),
        (K::Merge, P::MergeDeduplicate),
        (K::Mix, P::MixFieldBlend),
        (K::Mix, P::MixWeighted),
        (K::Mix, P::MixGainAverage),
        (K::Split, P::SplitCopyToAll),
        (K::Split, P::SplitByIndexModulo),
        (K::Split, P::SplitByEventField),
        (
            K::Split,
            P::SplitByPitchRange {
                threshold_octave: 4,
            },
        ),
        (K::Mask, P::MaskGate),
        (K::Mask, P::MaskScale),
        (K::Mask, P::MaskClip),
        (K::Mask, P::MaskInvertGate),
        (K::Switch, P::SwitchCycleIndex),
        (K::Switch, P::SwitchControlValue),
        (K::Switch, P::SwitchSeededRandom),
        (K::Route, P::RouteByEventField),
        (K::Route, P::RouteByIndexModulo),
        (K::Route, P::RouteByControlValue),
        (K::Route, P::RouteByLabel),
        (K::Choice, P::ChoiceCycle),
        (K::Choice, P::ChoiceSeededRandom),
        (K::Choice, P::ChoiceWeighted),
    ]
}
pub fn authored_program(kind: FlowControlKind, policy: FlowControlPolicy) -> TesseraProgram {
    let mut containers = BTreeMap::from([
        (
            ContainerId::new("a"),
            Container::new(ContainerKind::Alternate, vec![note("c", 4), note("d", 4)]),
        ),
        (
            ContainerId::new("b"),
            Container::new(ContainerKind::Alternate, vec![note("g", 4), note("a", 4)]),
        ),
        (
            ContainerId::new("main"),
            Container::new(
                ContainerKind::Sequence,
                vec![
                    ContainerSurfaceTile::NestedContainer(ContainerId::new("a")),
                    note("e", 3),
                    note("b", 5),
                ],
            ),
        ),
        (
            ContainerId::new("control"),
            Container::new(ContainerKind::Alternate, vec![scalar(0), scalar(1)]),
        ),
    ]);
    // A held note deliberately crosses the slot/cycle boundary.
    containers
        .get_mut(&ContainerId::new("main"))
        .unwrap()
        .stack
        .push(ContainerSurfaceTile::Atom(AtomTile::Modifier(
            AtomModifier::Legato(Rational::from_integer(4)),
        )));
    let mut root_nodes = BTreeMap::new();
    for name in ["a", "b", "main", "control"] {
        root_nodes.insert(
            id(name),
            RootSurfaceNodeKind::Container {
                container: ContainerId::new(name),
            },
        );
    }
    let mut control = FlowControlNode::new(kind).with_policy(policy);
    if matches!(control.policy, FlowControlPolicy::RouteByLabel) {
        control.members.outputs.insert(
            PortGroupId::new("routes"),
            vec![PortMemberId::new("c"), PortMemberId::new("d")],
        );
    }
    if matches!(
        control.policy,
        FlowControlPolicy::SplitByEventField | FlowControlPolicy::RouteByEventField
    ) {
        let group = if kind == FlowControlKind::Split {
            "branches"
        } else {
            "routes"
        };
        control.members.outputs.insert(
            PortGroupId::new(group),
            vec![PortMemberId::new("legato"), PortMemberId::new("plain")],
        );
    }
    let mut relations = Vec::new();
    for socket in &control.signature.input_sockets {
        let source = if socket.port.0 == "main" {
            "main"
        } else {
            "control"
        };
        relations.push(RootRelation::FlowsTo {
            from: StreamSource::node(id(source)),
            to: StreamTarget::FlowControlInput {
                node: id("flow"),
                endpoint: InputEndpoint::Socket(socket.port.clone()),
            },
        });
    }
    for (group, members) in &control.members.inputs {
        for (index, member) in members.iter().enumerate() {
            relations.push(RootRelation::FlowsTo {
                from: StreamSource::node(id(if index == 0 { "a" } else { "b" })),
                to: StreamTarget::FlowControlInput {
                    node: id("flow"),
                    endpoint: input(&group.0, &member.0),
                },
            });
        }
    }
    let outputs = control
        .signature
        .output_sockets
        .iter()
        .map(|s| OutputEndpoint::Socket(s.port.clone()))
        .chain(control.members.outputs.iter().flat_map(|(g, ms)| {
            ms.iter().map(|m| OutputEndpoint::GroupMember {
                group: g.clone(),
                member: m.clone(),
            })
        }))
        .collect::<Vec<_>>();
    for (index, endpoint) in outputs.into_iter().enumerate() {
        let output = id(&format!("out{index}"));
        root_nodes.insert(
            output.clone(),
            RootSurfaceNodeKind::Output(OutputNode::default()),
        );
        relations.push(RootRelation::FlowsTo {
            from: StreamSource {
                node: id("flow"),
                endpoint,
            },
            to: StreamTarget::OutputInput {
                node: output,
                endpoint: input("inputs", "main"),
            },
        });
    }
    root_nodes.insert(id("flow"), RootSurfaceNodeKind::FlowControl(control));
    TesseraProgram {
        root_nodes,
        containers,
        relations,
    }
}
pub fn authored(kind: FlowControlKind, policy: FlowControlPolicy) -> PatternIr {
    TesseraCompiler::new()
        .compile_authored(&authored_input(kind, policy))
        .unwrap()
        .ir
}

pub fn authored_input(kind: FlowControlKind, policy: FlowControlPolicy) -> AuthoredTesseraProgram {
    as_authored(&authored_program(kind, policy))
}
#[path = "compiler.rs"]
mod compiler_support;
use compiler_support::authored as as_authored;
