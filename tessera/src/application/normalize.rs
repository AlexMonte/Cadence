use crate::domain::{
    AtomExpr, AtomExprKind, AtomModifier, AtomOperatorToken, AtomTile, ContainerId, ContainerKind,
    ContainerSurfaceTile, Diagnostic, DiagnosticCategory, DiagnosticKind, DiagnosticLocation,
    NormalizedContainer, NormalizedProgram, TesseraProgram, try_parse_note_value,
};

pub fn normalize_container(
    program: &TesseraProgram,
    container_id: &ContainerId,
) -> Result<NormalizedContainer, Vec<Diagnostic>> {
    let Some(container) = program.containers.get(container_id) else {
        return Err(vec![Diagnostic::new(
            DiagnosticCategory::Placement,
            DiagnosticKind::MissingContainer,
            "Container is missing from the program container table.",
            Some(DiagnosticLocation::ContainerStack {
                container: container_id.clone(),
                index: 0,
            }),
        )]);
    };

    let mut exprs = Vec::new();
    let mut diagnostics = Vec::new();
    let mut index = 0usize;

    while index < container.stack.len() {
        let expr = parse_atom_expr(
            program,
            container_id,
            &container.stack,
            &mut index,
            &mut diagnostics,
        );
        if let Some(expr) = expr {
            exprs.push(expr);
        }
    }

    if diagnostics.is_empty()
        && !matches!(
            container.kind,
            ContainerKind::Sequence | ContainerKind::Arrangement
        )
    {
        for (index, expr) in exprs.iter().enumerate() {
            if expr_contains_elongate(expr) {
                diagnostics.push(Diagnostic::new(
                    DiagnosticCategory::LocalGrammar,
                    DiagnosticKind::InvalidModifierArgument,
                    "Elongate is valid inside Sequence as a slot weight, or Arrangement as a duration in cycles.",
                    Some(DiagnosticLocation::ContainerStack {
                        container: container_id.clone(),
                        index,
                    }),
                ));
            }
        }
    }

    if diagnostics.is_empty() {
        Ok(
            NormalizedContainer::new(container_id.clone(), container.kind, exprs)
                .with_axis(container.axis),
        )
    } else {
        Err(diagnostics)
    }
}

fn parse_atom_expr(
    program: &TesseraProgram,
    container_id: &ContainerId,
    stack: &[ContainerSurfaceTile],
    index: &mut usize,
    diagnostics: &mut Vec<Diagnostic>,
) -> Option<AtomExpr> {
    let mut expr = parse_simple_expr(program, container_id, stack, index, diagnostics)?;
    while let Some(ContainerSurfaceTile::Atom(AtomTile::Operator(token))) = stack.get(*index) {
        let group_kind = match token {
            AtomOperatorToken::Choice => Some(AtomExprKind::Choice(Vec::new())),
            AtomOperatorToken::Parallel => Some(AtomExprKind::Parallel(Vec::new())),
            _ => None,
        };
        let Some(mut group_kind) = group_kind else {
            break;
        };
        *index += 1;
        let Some(rhs) = parse_simple_expr(program, container_id, stack, index, diagnostics) else {
            diagnostics.push(Diagnostic::new(
                DiagnosticCategory::LocalGrammar,
                DiagnosticKind::OperatorWithoutRightScalar,
                "Choice and parallel separators require an expression on their right.",
                Some(DiagnosticLocation::ContainerStack {
                    container: container_id.clone(),
                    index: index.saturating_sub(1),
                }),
            ));
            break;
        };
        let mut members = vec![expr, rhs];
        while matches!(stack.get(*index), Some(ContainerSurfaceTile::Atom(AtomTile::Operator(next))) if next == token)
        {
            *index += 1;
            if let Some(member) =
                parse_simple_expr(program, container_id, stack, index, diagnostics)
            {
                members.push(member);
            } else {
                break;
            }
        }
        while matches!(
            stack.get(*index),
            Some(ContainerSurfaceTile::Atom(AtomTile::Note(_)))
                | Some(ContainerSurfaceTile::Atom(AtomTile::Sound(_)))
                | Some(ContainerSurfaceTile::Atom(AtomTile::Scalar(_)))
                | Some(ContainerSurfaceTile::Atom(AtomTile::Rest))
                | Some(ContainerSurfaceTile::NestedContainer(_))
        ) {
            if let Some(member) =
                parse_simple_expr(program, container_id, stack, index, diagnostics)
            {
                members.push(member);
            } else {
                break;
            }
        }
        group_kind = match group_kind {
            AtomExprKind::Choice(_) => AtomExprKind::Choice(members),
            AtomExprKind::Parallel(_) => AtomExprKind::Parallel(members),
            _ => unreachable!(),
        };
        expr = AtomExpr {
            source_node: None,
            kind: group_kind,
            modifiers: Vec::new(),
        };
    }
    Some(expr)
}

fn parse_simple_expr(
    _program: &TesseraProgram,
    container_id: &ContainerId,
    stack: &[ContainerSurfaceTile],
    index: &mut usize,
    diagnostics: &mut Vec<Diagnostic>,
) -> Option<AtomExpr> {
    use crate::domain::{StackCompound, StackPiece, StackReject};
    let start = *index;
    let mut pieces = Vec::new();
    let error = |kind, message: &str| {
        Diagnostic::new(
            DiagnosticCategory::LocalGrammar,
            kind,
            message,
            Some(DiagnosticLocation::ContainerStack {
                container: container_id.clone(),
                index: start,
            }),
        )
    };
    match stack.get(start)? {
        ContainerSurfaceTile::Atom(AtomTile::Note(note)) => {
            if !note.label.is_empty() && try_parse_note_value(&note.label).is_none() {
                diagnostics.push(error(DiagnosticKind::InvalidNote, "Note label must be one of a, b, c, d, e, f, g; accidentals have their own pitch role."));
                *index += 1;
                return None;
            }
            pieces.push(StackPiece::note(note.value.clone(), note.label.clone()));
            if let Some(octave) = note.octave {
                pieces.push(StackPiece::Octave(octave));
            }
            if let Some(accidental) = note.accidental {
                pieces.push(StackPiece::Accidental(accidental));
            }
        }
        ContainerSurfaceTile::Atom(AtomTile::Sound(value)) => {
            pieces.push(StackPiece::Sound(value.clone()));
        }
        ContainerSurfaceTile::Atom(AtomTile::Modifier(modifier))
            if modifier.effect_value().is_some() =>
        {
            pieces.push(StackPiece::Modifier(modifier.clone()));
        }
        ContainerSurfaceTile::Atom(AtomTile::Rest) => pieces.push(StackPiece::Rest),
        ContainerSurfaceTile::Atom(AtomTile::Scalar(value)) => {
            pieces.push(StackPiece::Scalar(value.value))
        }
        ContainerSurfaceTile::NestedContainer(id) => pieces.push(StackPiece::Nested(id.clone())),
        unexpected => {
            let kind = match unexpected {
                ContainerSurfaceTile::Transform => DiagnosticKind::TransformInsideContainer,
                ContainerSurfaceTile::Output => DiagnosticKind::OutputInsideContainer,
                _ => DiagnosticKind::OperatorWithoutLeftValue,
            };
            diagnostics.push(error(
                kind,
                "This tile needs a musical expression to own it.",
            ));
            *index += 1;
            return None;
        }
    }
    *index += 1;
    loop {
        match stack.get(*index) {
            Some(ContainerSurfaceTile::Atom(AtomTile::Accidental(value))) => {
                pieces.push(StackPiece::Accidental(*value));
                *index += 1;
            }
            Some(ContainerSurfaceTile::Atom(AtomTile::Octave(value))) => {
                pieces.push(StackPiece::Octave(*value));
                *index += 1;
            }
            Some(ContainerSurfaceTile::Atom(AtomTile::Modifier(modifier))) => {
                // Standalone complete effect values occupy successive time slots.
                // Once a note owns the group, ordinary modifier stacking remains unchanged.
                if matches!(pieces.first(), Some(StackPiece::Modifier(root)) if root.effect_value().is_some())
                    && modifier.effect_value().is_some()
                {
                    break;
                }
                pieces.push(StackPiece::Modifier(modifier.clone()));
                *index += 1;
            }
            Some(ContainerSurfaceTile::Atom(AtomTile::Operator(
                AtomOperatorToken::Choice | AtomOperatorToken::Parallel,
            ))) => break,
            Some(ContainerSurfaceTile::Atom(AtomTile::Operator(operator))) => {
                pieces.push(StackPiece::Operator(*operator));
                *index += 1;
                let count = match operator {
                    AtomOperatorToken::Euclid => 2,
                    AtomOperatorToken::EuclidRot => 3,
                    _ => 1,
                };
                for _ in 0..count {
                    let Some(ContainerSurfaceTile::Atom(AtomTile::Scalar(value))) =
                        stack.get(*index)
                    else {
                        break;
                    };
                    pieces.push(StackPiece::Scalar(value.value));
                    *index += 1;
                }
            }
            _ => break,
        }
    }
    match StackCompound::from_layers(pieces).resolve() {
        Ok(mut expr) => {
            expr.source_node = _program
                .containers
                .get(container_id)
                .and_then(|container| container.source_nodes.get(&start))
                .cloned();
            Some(expr)
        }
        Err(reject) => {
            let (kind, message) = match reject {
                StackReject::MissingModifierArgument { .. } => (
                    DiagnosticKind::OperatorWithoutRightScalar,
                    "This modifier owns a missing operand; add its value without taking another group's number.",
                ),
                StackReject::UnassignedScalar { .. } | StackReject::DuplicateOctave => (
                    DiagnosticKind::InvalidModifierArgument,
                    "Numbers must be independent values or operands owned by a modifier; a note accepts at most one octave tile.",
                ),
                _ => (
                    DiagnosticKind::InvalidModifierArgument,
                    "This stack has an invalid pitch or modifier group.",
                ),
            };
            diagnostics.push(error(kind, message));
            None
        }
    }
}

fn expr_contains_elongate(expr: &AtomExpr) -> bool {
    if expr
        .modifiers
        .iter()
        .any(|modifier| matches!(modifier, AtomModifier::Elongate(_)))
    {
        return true;
    }
    match &expr.kind {
        AtomExprKind::Choice(branches) | AtomExprKind::Parallel(branches) => {
            branches.iter().any(expr_contains_elongate)
        }
        AtomExprKind::Value(_) => false,
    }
}

pub fn normalize_program(program: &TesseraProgram) -> Result<NormalizedProgram, Vec<Diagnostic>> {
    let mut containers = std::collections::BTreeMap::new();
    let mut diagnostics = Vec::new();
    for container_id in program.containers.keys() {
        match normalize_container(program, container_id) {
            Ok(container) => {
                containers.insert(container_id.clone(), container);
            }
            Err(mut errs) => diagnostics.append(&mut errs),
        }
    }
    if diagnostics.is_empty() {
        Ok(NormalizedProgram {
            root_nodes: program.root_nodes.clone(),
            containers,
            relations: program.relations.clone(),
        })
    } else {
        Err(diagnostics)
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use crate::domain::stack_compound_from_tiles;
    use crate::domain::{AtomExprKind, Container, ContainerKind, NoteAtom, ScalarAtom};

    use super::*;

    #[test]
    fn normalizes_broad_modifier_subset() {
        let container_id = ContainerId::new("pattern");
        let mut containers = BTreeMap::new();
        containers.insert(
            container_id.clone(),
            Container::new(
                ContainerKind::Sequence,
                vec![
                    ContainerSurfaceTile::Atom(AtomTile::Note(NoteAtom::new("e"))),
                    ContainerSurfaceTile::Atom(AtomTile::Operator(AtomOperatorToken::Elongate)),
                    ContainerSurfaceTile::Atom(AtomTile::Scalar(ScalarAtom::integer(4))),
                    ContainerSurfaceTile::Atom(AtomTile::Operator(AtomOperatorToken::Slow)),
                    ContainerSurfaceTile::Atom(AtomTile::Scalar(ScalarAtom::integer(2))),
                    ContainerSurfaceTile::Atom(AtomTile::Operator(AtomOperatorToken::Replicate)),
                    ContainerSurfaceTile::Atom(AtomTile::Scalar(ScalarAtom::integer(3))),
                    ContainerSurfaceTile::Atom(AtomTile::Operator(AtomOperatorToken::Degrade)),
                    ContainerSurfaceTile::Atom(AtomTile::Operator(AtomOperatorToken::Choice)),
                    ContainerSurfaceTile::Atom(AtomTile::Note(NoteAtom::new("g"))),
                ],
            ),
        );

        let normalized = normalize_container(
            &TesseraProgram {
                root_nodes: BTreeMap::new(),
                containers,
                relations: vec![],
            },
            &container_id,
        )
        .expect("container should normalize");

        assert_eq!(normalized.exprs.len(), 1);
        match &normalized.exprs[0].kind {
            AtomExprKind::Choice(branches) => {
                assert_eq!(branches.len(), 2);
                assert_eq!(branches[0].modifiers.len(), 4);
            }
            other => panic!("expected choice expression, got {other:?}"),
        }
    }

    #[test]
    fn stack_compound_matches_normalize_for_stacked_modifiers() {
        let container_id = ContainerId::new("pattern");
        let stack = vec![
            ContainerSurfaceTile::Atom(AtomTile::Note(NoteAtom::new("e"))),
            ContainerSurfaceTile::Atom(AtomTile::Octave(2)),
            ContainerSurfaceTile::Atom(AtomTile::Operator(AtomOperatorToken::Elongate)),
            ContainerSurfaceTile::Atom(AtomTile::Scalar(ScalarAtom::integer(2))),
        ];
        let mut containers = BTreeMap::new();
        containers.insert(
            container_id.clone(),
            Container::new(ContainerKind::Sequence, stack.clone()),
        );

        let normalized = normalize_container(
            &TesseraProgram {
                root_nodes: BTreeMap::new(),
                containers,
                relations: vec![],
            },
            &container_id,
        )
        .expect("container should normalize");

        let via_stack = stack_compound_from_tiles(&stack, 0)
            .resolve()
            .expect("stack compound resolves");
        assert_eq!(normalized.exprs, vec![via_stack]);
    }
}
