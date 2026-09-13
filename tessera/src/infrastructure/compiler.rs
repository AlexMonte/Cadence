use crate::application::{
    CompileContext, compile_container, compile_container_ir, compile_normalized_program,
    resolve_spatial_program, validate_and_normalize_program,
};
use crate::domain::{
    AuthoredTesseraProgram, ContainerId, CycleSpan, Diagnostic, DiagnosticCategory, DiagnosticKind,
    DiagnosticLocation, NormalizedProgram, TesseraProgram,
};

use super::{CompileReport, PreviewReport, ValidationReport};

#[derive(Debug, Clone, Default)]
pub struct TesseraCompiler;

impl TesseraCompiler {
    pub fn new() -> Self {
        Self
    }

    /// Inspect an endpoint while a board is still being authored. Unconnected
    /// outputs and unrelated unfinished containers do not prevent type discovery.
    pub fn authored_output_shape(
        &self,
        program: &AuthoredTesseraProgram,
        node: &crate::domain::NodeId,
        endpoint: &crate::domain::OutputEndpoint,
    ) -> Option<crate::domain::StreamShape> {
        self.authored_output_shape_inner(program, node, endpoint, &mut Default::default())
    }

    fn authored_output_shape_inner(
        &self,
        program: &AuthoredTesseraProgram,
        node: &crate::domain::NodeId,
        endpoint: &crate::domain::OutputEndpoint,
        visiting: &mut std::collections::BTreeSet<crate::domain::NodeId>,
    ) -> Option<crate::domain::StreamShape> {
        use crate::domain::{NormalizedProgram, RootSurfaceNodeKind};
        if !visiting.insert(node.clone()) {
            return Some(crate::domain::StreamShape::Any);
        }
        let kind = program.root_surface.nodes.get(node)?;
        if matches!(kind, RootSurfaceNodeKind::Transform(transform) if transform.kind == crate::domain::TransformKind::Wire)
        {
            if !matches!(endpoint, crate::domain::OutputEndpoint::Socket(port) if port.0 == "out") {
                return None;
            }
            let relations = &program.root_surface.explicit_relations;
            let mut sources = crate::application::relations::incoming_socket_sources(
                relations,
                node,
                &crate::domain::InputPort::new("main"),
            );
            sources.extend(crate::application::relations::incoming_chain_sources(
                relations, node,
            ));
            if sources.is_empty() {
                let input =
                    crate::domain::InputEndpoint::Socket(crate::domain::InputPort::new("main"));
                if let Some(side) = program
                    .root_surface
                    .bindings
                    .get(node)
                    .and_then(|bindings| bindings.inputs.get(&input))
                    .filter(|side| side.is_enabled())
                    && let Some(crate::domain::RootRelation::FlowsTo { from, .. }) =
                        crate::application::infer_input_relation(program, node, &input, *side)
                            .ok()
                            .flatten()
                {
                    sources.push(from);
                }
            }
            return match sources.first() {
                Some(source) => self.authored_output_shape_inner(
                    program,
                    &source.node,
                    &source.endpoint,
                    visiting,
                ),
                None => Some(crate::domain::StreamShape::Any),
            };
        }
        let signature = match kind {
            RootSurfaceNodeKind::Transform(value) => Some(&value.signature),
            RootSurfaceNodeKind::FlowControl(value) => Some(&value.signature),
            RootSurfaceNodeKind::Output(_) => return None,
            _ => None,
        };
        let declared = if let Some(signature) = signature {
            match endpoint {
                crate::domain::OutputEndpoint::Socket(port) => {
                    signature.output_socket(port).is_some()
                }
                crate::domain::OutputEndpoint::GroupMember { group, member } => {
                    signature.output_group(group).is_some()
                        && match kind {
                            RootSurfaceNodeKind::FlowControl(value) => value
                                .members
                                .outputs
                                .get(group)
                                .is_some_and(|members| members.contains(member)),
                            _ => true,
                        }
                }
            }
        } else {
            matches!(endpoint, crate::domain::OutputEndpoint::Socket(port) if port.0=="out")
        };
        if !declared {
            return None;
        }
        let mut normalized = NormalizedProgram {
            root_nodes: program.root_surface.nodes.clone(),
            containers: Default::default(),
            relations: Vec::new(),
        };
        if let RootSurfaceNodeKind::Container { container } = kind {
            let source = TesseraProgram {
                root_nodes: program.root_surface.nodes.clone(),
                containers: program.containers.clone(),
                relations: Vec::new(),
            };
            let mut pending = vec![(container.clone(), std::collections::BTreeSet::new())];
            let mut seen = std::collections::BTreeSet::new();
            while let Some((id, mut ancestors)) = pending.pop() {
                if !ancestors.insert(id.clone()) || ancestors.len() > 128 {
                    return None;
                }
                if !seen.insert(id.clone()) {
                    continue;
                }
                let authored = program.containers.get(&id)?;
                pending.extend(authored.stack.iter().filter_map(|tile| match tile {
                    crate::domain::ContainerSurfaceTile::NestedContainer(id) => {
                        Some((id.clone(), ancestors.clone()))
                    }
                    _ => None,
                }));
                normalized.containers.insert(
                    id.clone(),
                    crate::application::normalize::normalize_container(&source, &id).ok()?,
                );
            }
        }
        Some(
            crate::application::stream_shape::normalized_node_output_shape(
                &normalized,
                node,
                endpoint,
                &mut Default::default(),
            ),
        )
    }

    /// Uses the same stream compatibility rule as compilation.
    pub fn stream_shape_compatible(
        source: crate::domain::StreamShape,
        target: crate::domain::StreamShape,
    ) -> bool {
        crate::application::stream_shape::stream_shape_compatible(source, target)
    }

    /// Compile one named pattern without evaluating it at a particular cycle.
    /// The recursive tree retains alternation and modifier scope for host queries.
    pub fn compile_normalized_container_ir(
        &self,
        program: &NormalizedProgram,
        container_id: &ContainerId,
    ) -> Result<crate::domain::PatternNodeIr, Vec<Diagnostic>> {
        let Some(container) = program.containers.get(container_id) else {
            return Err(vec![Diagnostic::new(
                DiagnosticCategory::Placement,
                DiagnosticKind::MissingContainer,
                "Pattern target is missing from normalized program.",
                Some(DiagnosticLocation::ContainerStack {
                    container: container_id.clone(),
                    index: 0,
                }),
            )]);
        };
        compile_container_ir(program, container, &mut CompileContext::default())
    }

    pub fn preview_authored_container(
        &self,
        authored: &AuthoredTesseraProgram,
        container_id: ContainerId,
        span: CycleSpan,
        cycle_index: usize,
    ) -> Result<PreviewReport, Vec<Diagnostic>> {
        let resolved = resolve_spatial_program(authored)?;
        let normalized_program = validate_and_normalize_program(&resolved)?;
        let Some(container) = normalized_program.containers.get(&container_id) else {
            return Err(vec![Diagnostic::new(
                DiagnosticCategory::Placement,
                DiagnosticKind::MissingContainer,
                "Container preview target is missing from normalized program.",
                Some(DiagnosticLocation::ContainerStack {
                    container: container_id,
                    index: 0,
                }),
            )]);
        };
        let mut ctx = CompileContext::new(cycle_index);
        let stream = compile_container(&normalized_program, container, span, &mut ctx)?;
        Ok(PreviewReport { stream })
    }

    pub fn resolve(
        &self,
        authored: &AuthoredTesseraProgram,
    ) -> Result<TesseraProgram, Vec<Diagnostic>> {
        resolve_spatial_program(authored)
    }

    pub fn validate_authored(&self, authored: &AuthoredTesseraProgram) -> ValidationReport {
        match self.resolve(authored) {
            Ok(resolved) => match validate_and_normalize_program(&resolved) {
                Ok(normalized) => ValidationReport::valid(normalized),
                Err(diagnostics) => ValidationReport::invalid(diagnostics),
            },
            Err(diagnostics) => ValidationReport::invalid(diagnostics),
        }
    }

    pub fn compile_authored(
        &self,
        authored: &AuthoredTesseraProgram,
    ) -> Result<CompileReport, Vec<Diagnostic>> {
        let resolved = self.resolve(authored)?;
        let normalized = validate_and_normalize_program(&resolved)?;
        let ir = compile_normalized_program(&normalized, &mut CompileContext::default())?;
        Ok(CompileReport { normalized, ir })
    }
}
