//! Read-only things to inspect when a project is silent. These are candidates,
//! not a claim that an unused or deliberately gated tile caused the silence.
use super::session::MusaicProject;
use crate::domain::{
    board::BoardSurfaceId,
    document::{AtomValue, DocumentNodeKind},
    instrument::InstrumentSource,
};
use tessera::prelude::{AtomModifier, NodeId};

#[derive(Debug, Clone)]
pub struct AudioCheck {
    pub node: NodeId,
    pub surface: BoardSurfaceId,
    pub detail: String,
}

pub struct AudioChecks {
    pub outputs: usize,
    pub issues: Vec<AudioCheck>,
}

pub fn inspect(project: &MusaicProject) -> AudioChecks {
    let inactive = super::compile::inactive_outputs(project);
    let mut outputs = 0;
    let issues = project
        .document
        .graph
        .nodes()
        .filter_map(|node| {
            if matches!(node.kind, DocumentNodeKind::Output(_)) {
                outputs += 1;
            }
            let detail = match &node.kind {
                DocumentNodeKind::Output(output) if inactive.contains(&node.id) => Some(format!(
                    "{} · no incoming route",
                    if output.name.is_empty() {
                        "Output"
                    } else {
                        &output.name
                    }
                )),
                DocumentNodeKind::Atom(atom) => match atom.atom {
                    AtomValue::Modifier(AtomModifier::Gate(false)) => Some("Gate is closed".into()),
                    AtomValue::Modifier(
                        AtomModifier::Gain(value) | AtomModifier::PostGain(value),
                    ) if value.is_zero() => Some("Gain is zero".into()),
                    _ => None,
                },
                DocumentNodeKind::Sound(sound) => {
                    let instrument = &sound.definition;
                    if matches!(instrument.source, InstrumentSource::Sample(sample) if project.samples.decoded(sample).is_none()) {
                        Some("Sound sample is unavailable · relink or choose a sound".into())
                    } else {
                        instrument
                            .sound
                            .gain
                            .filter(|gain| gain.is_zero())
                            .map(|_| "Sound volume is zero".into())
                    }
                }
                _ => None,
            }?;
            Some(AudioCheck {
                node: node.id.clone(),
                surface: project.document.graph.location_of(&node.id)?.surface,
                detail,
            })
        })
        .collect();
    AudioChecks { outputs, issues }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{
        document::{PlacementAddress, StackIndex, TileSpawnKind},
        instrument::InstrumentDefinition,
        project::samples::SampleId,
    };
    use tessera::prelude::{Rational, SpatialSide};
    #[test]
    fn checks_real_routes_owned_mutes_and_missing_instrument_sources_without_editing() {
        assert_eq!(inspect(&MusaicProject::new_empty()).outputs, 0);
        let mut starter = super::super::session::first_loop();
        assert_eq!(inspect(&starter.project).outputs, 1);
        assert!(inspect(&starter.project).issues.is_empty());
        let document = &mut starter.project.document;
        let surface = document.graph.container_surface(&starter.pattern).unwrap();
        document
            .graph
            .insert_tile(
                &mut document.surfaces,
                surface,
                PlacementAddress::StackIndex(StackIndex(7)),
                TileSpawnKind::Atom {
                    atom: AtomValue::Modifier(AtomModifier::Gate(false)),
                },
            )
            .unwrap();
        document
            .graph
            .insert_tile(
                &mut document.surfaces,
                surface,
                PlacementAddress::StackIndex(StackIndex(8)),
                TileSpawnKind::Atom {
                    atom: AtomValue::Modifier(AtomModifier::Gain(Rational::zero())),
                },
            )
            .unwrap();
        let instrument = document
            .graph
            .nodes()
            .find(|n| matches!(&n.kind, DocumentNodeKind::Sound(_)))
            .unwrap()
            .id
            .clone();
        starter
            .project
            .document
            .graph
            .set_sound_definition(
                &instrument,
                InstrumentDefinition {
                    source: InstrumentSource::Sample(SampleId(999)),
                    ..Default::default()
                },
            )
            .unwrap();
        let checks = inspect(&starter.project).issues;
        assert_eq!(checks.len(), 3);
        assert!(checks.iter().any(|c| c.detail == "Gate is closed"));
        assert!(checks.iter().any(|c| c.detail == "Gain is zero"));
        assert!(
            checks
                .iter()
                .any(|c| c.detail.starts_with("Sound sample is unavailable"))
        );
        for check in checks {
            assert_eq!(
                starter
                    .project
                    .document
                    .graph
                    .location_of(&check.node)
                    .unwrap()
                    .surface,
                check.surface
            );
        }
        // The compiler's own inactive-output policy is the source for this check.
        let output = starter
            .project
            .document
            .graph
            .nodes()
            .find(|n| matches!(n.kind, DocumentNodeKind::Output(_)))
            .unwrap()
            .id
            .clone();
        let mut program =
            crate::domain::document::export_document_program(&starter.project.document).unwrap();
        let binding = program.root_surface.bindings.get_mut(&output).unwrap();
        for side in binding.inputs.values_mut() {
            *side = SpatialSide::Off;
        }
        starter.project.document.replace_connections_from(&program);
        assert!(
            inspect(&starter.project)
                .issues
                .iter()
                .any(|c| c.node == output && c.detail.contains("no incoming route"))
        );
    }
}
