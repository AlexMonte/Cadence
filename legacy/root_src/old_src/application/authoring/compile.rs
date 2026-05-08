//! Cadence-level compile pipeline: Tessera analysis in, typed host IR out.

use tessera::Diagnostic;
use tessera::diagnostics::DiagnosticKind;

use crate::{
    adapter::{
        cadence_core::{PlaybackScore, PlaybackTime, merged_playback_score},
        tessera::{
            host_adapter::{analyze_tricks, normalize_project_piece_sides, runtime_engine},
            lowering::lower_target_graph,
        },
    },
    application::authoring::compile_support::has_error_diagnostics,
    application::tempo::parse_cps_expr,
    domain::{
        common::GridPos, preview::PreviewDocument, program::CadenceProgram,
        project::CadenceProjectDocument,
    },
};

#[derive(Debug, Clone)]
pub struct CompiledProject {
    #[allow(dead_code)]
    pub program: CadenceProgram,
    pub diagnostics: Vec<Diagnostic>,
    pub preview: PreviewDocument,
    pub can_render: bool,
    pub can_play: bool,
    pub sample_selectors: Vec<String>,
    pub cps: Option<PlaybackTime>,
    pub output_count: usize,
    pub playback_score: Option<PlaybackScore>,
}

pub fn compile_project(project: &CadenceProjectDocument) -> CompiledProject {
    let mut project = project.clone();
    normalize_project_piece_sides(&mut project);
    let mut tempo_diagnostics = Vec::new();
    let cps = match parse_cps_expr(project.init_stage.cps_expr.as_deref()) {
        Ok(parsed) => parsed.map(|tempo| tempo.cps),
        Err(reason) => {
            tempo_diagnostics.push(Diagnostic::error(
                DiagnosticKind::InvalidOperation {
                    reason: format!("invalid cps expression: {reason}"),
                },
                None,
            ));
            None
        }
    };

    let analyzed = runtime_engine(&project).analyze(project.runtime_graph());
    let lowered = lower_target_graph(
        &project,
        &crate::domain::project::CadenceGraphTarget::Runtime,
        &analyzed,
    );
    let preview = PreviewDocument {
        debug_text: render_debug_document(
            &project,
            lowered.output_debug.as_slice(),
            lowered.outputs.as_slice(),
        ),
        ..lowered.preview
    };
    let can_render = !lowered.program.outputs.is_empty();
    let playback_score = merged_playback_score(lowered.output_scores.as_slice());
    let mut diagnostics = lowered.diagnostics;
    diagnostics.extend(tempo_diagnostics);
    let can_play = playback_score.is_some() && !has_error_diagnostics(&diagnostics);

    CompiledProject {
        program: lowered.program,
        diagnostics,
        preview,
        can_render,
        can_play,
        sample_selectors: lowered.sample_selectors,
        cps,
        output_count: lowered.output_scores.len(),
        playback_score,
    }
}

fn trick_headers(project: &CadenceProjectDocument) -> Vec<String> {
    analyze_tricks(project)
        .into_iter()
        .map(|trick| {
            let params = trick
                .signature
                .inputs
                .iter()
                .map(|input| input.label.clone())
                .collect::<Vec<_>>();
            if params.is_empty() {
                format!("trick {}()", trick.name)
            } else {
                format!("trick {}({})", trick.name, params.join(", "))
            }
        })
        .collect()
}

fn render_debug_document(
    project: &CadenceProjectDocument,
    outputs: &[String],
    output_roots: &[GridPos],
) -> Option<String> {
    let mut sections = Vec::new();

    if let Some(cps_expr) = project.init_stage.cps_expr.as_deref().map(str::trim)
        && !cps_expr.is_empty()
    {
        sections.push(format!("cps = {cps_expr}"));
    }

    if !project.init_stage.sample_loads.is_empty() {
        sections.push(
            project
                .init_stage
                .sample_loads
                .iter()
                .map(|sample| format!("sample {} <- {}", sample.id, sample.source))
                .collect::<Vec<_>>()
                .join("\n"),
        );
    }

    let declarations = trick_headers(project);
    if !declarations.is_empty() {
        sections.push(declarations.join("\n"));
    }

    if !outputs.is_empty() {
        sections.push(outputs.join("\n"));
    } else if !output_roots.is_empty() {
        sections.push(
            output_roots
                .iter()
                .map(|root| format!("@({}, {}) output", root.col, root.row))
                .collect::<Vec<_>>()
                .join("\n"),
        );
    }

    (!sections.is_empty()).then(|| sections.join("\n\n"))
}
