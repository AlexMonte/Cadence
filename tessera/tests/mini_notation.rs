use tessera::prelude::*;

fn expand(
    pattern: MiniPattern,
    program: &mut TesseraProgram,
    serial: &mut usize,
) -> Vec<ContainerSurfaceTile> {
    let mut stack = match pattern.kind {
        MiniPatternKind::Rest => vec![ContainerSurfaceTile::Atom(AtomTile::Rest)],
        MiniPatternKind::Value(value) => {
            vec![ContainerSurfaceTile::Atom(AtomTile::Scalar(ScalarAtom {
                value: parse_pattern_number(&value).unwrap(),
            }))]
        }
        MiniPatternKind::Group(kind, children) => {
            *serial += 1;
            let id = ContainerId::new(format!("mini-{serial}"));
            let stack = children
                .into_iter()
                .flat_map(|c| expand(c, program, serial))
                .collect();
            program
                .containers
                .insert(id.clone(), Container::new(kind, stack));
            vec![ContainerSurfaceTile::NestedContainer(id)]
        }
    };
    stack.extend(
        pattern
            .modifiers
            .into_iter()
            .map(|m| ContainerSurfaceTile::Atom(AtomTile::Modifier(m))),
    );
    stack
}
fn compile(text: &str) -> PatternNodeIr {
    let mut p = TesseraProgram::default();
    let mut stack = expand(parse_mini_notation(text).unwrap(), &mut p, &mut 0);
    stack.push(ContainerSurfaceTile::Atom(AtomTile::Modifier(
        AtomModifier::Scale(ScaleParameters::default()),
    )));
    let id = ContainerId::new("source");
    p.containers
        .insert(id.clone(), Container::new(ContainerKind::Sequence, stack));
    p.root_nodes.insert(
        NodeId::new("source"),
        RootSurfaceNodeKind::Container { container: id },
    );
    p.root_nodes.insert(
        NodeId::new("out"),
        RootSurfaceNodeKind::Output(OutputNode::default()),
    );
    p.relations.push(RootRelation::FlowsTo {
        from: StreamSource::node(NodeId::new("source")),
        to: StreamTarget::OutputInput {
            node: NodeId::new("out"),
            endpoint: InputEndpoint::GroupMember {
                group: PortGroupId::new("inputs"),
                member: PortMemberId::new("main"),
            },
        },
    });
    TesseraCompiler::new()
        .compile_authored(&authored(&p))
        .unwrap()
        .ir
        .outputs
        .remove(0)
        .root
}
#[test]
fn chord_progression_preserves_alternate_clock_and_simultaneous_degrees() {
    let tree = compile(
        "<[[0,2,4,6] ~!3] ~ ~ ~ [[-1,0,2,4] ~!3] ~ ~ ~ [[1,3,5,7] ~!3] ~ ~ ~ [[-2,0,1,3] ~!3] ~ [[-2,-1,1,3] ~!3] ~>",
    );
    for cycle in 0..32 {
        let query = tree.query(CycleSpan::new(
            CycleTime(Rational::from_integer(cycle)),
            CycleDuration(Rational::one()),
        ));
        let notes: Vec<_> = query
            .events
            .iter()
            .filter(|e| !matches!(e.value, EventValue::Rest))
            .collect();
        assert_eq!(
            notes.len(),
            if [0, 4, 8, 12, 14].contains(&(cycle % 16)) {
                4
            } else {
                0
            },
            "cycle {cycle}"
        );
        if !notes.is_empty() {
            assert!(
                notes
                    .iter()
                    .all(|e| e.span.duration.0 == Rational::new(1, 4))
            );
        }
    }
}
#[test]
fn repeat_differs_from_speed_and_rest_alias_is_identical() {
    assert_eq!(
        compile("0 - 1")
            .query(CycleSpan::new(
                CycleTime(Rational::zero()),
                CycleDuration(Rational::one())
            ))
            .events
            .len(),
        3
    );
    let a = compile("0!3 1");
    let b = compile("0*3 1");
    let window = CycleSpan::new(CycleTime(Rational::zero()), CycleDuration(Rational::one()));
    let a = a.query(window).events;
    let b = b.query(window).events;
    assert_eq!(a[0].span.duration.0, Rational::new(1, 4));
    assert_eq!(b[0].span.duration.0, Rational::new(1, 6));
}
#[test]
fn rejects_invalid_and_excessive_notation_without_panicking() {
    for text in [
        "", "[]", "[0", "0]", "<0]", "0,", ",0", "0!1.5", "0/0", "0*1025", "0?", "0(3,8)", "0**2",
        "0*.",
    ] {
        assert!(parse_mini_notation(text).is_err(), "{text}");
    }
    assert!(parse_mini_notation(&format!("{}0{}", "[".repeat(70), "]".repeat(70))).is_err());
    assert_eq!(parse_pattern_number(".005"), Ok(Rational::new(1, 200)));
    assert_eq!(parse_pattern_number("-1.25"), Ok(Rational::new(-5, 4)));
}
#[test]
fn drum_names_and_variants_remain_host_owned_symbols() {
    let p = parse_mini_notation("[bd,cr] - sd hh:6 akailinn_sd 9000_hh").unwrap();
    assert!(matches!(p.kind,MiniPatternKind::Group(ContainerKind::Sequence, ref v) if v.len()==6));
}
#[path = "support/compiler.rs"]
mod compiler_support;
use compiler_support::authored;
