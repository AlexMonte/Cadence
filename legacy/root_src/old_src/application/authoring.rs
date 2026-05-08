//! Graph authoring and preview use-cases.

pub(crate) mod compile;
pub(crate) mod compile_support;
pub(crate) mod ops;
pub(crate) mod terrance;
pub(crate) mod tile_pattern;

use crate::{
    adapter::{tessera, transport},
    infrastructure::dto,
};

pub fn graph_snapshot(
    state: &dto::SharedAppState,
    target: Option<dto::CadenceGraphTarget>,
) -> Result<dto::GraphSnapshotDto, String> {
    let target = target.map(tessera::graph_target_to_internal).transpose()?;
    let graph = ops::graph_snapshot(state, target)?;
    transport::translate(graph)
}

pub fn graph_piece_catalog(
    state: &dto::SharedAppState,
    target: Option<dto::CadenceGraphTarget>,
) -> Result<Vec<dto::PieceDef>, String> {
    let target = target.map(tessera::graph_target_to_internal).transpose()?;
    let defs = ops::graph_piece_catalog(state, target)?;
    tessera::encode_piece_catalog(defs)
}

pub fn compile_graph_preview(
    state: &dto::SharedAppState,
    target: Option<dto::CadenceGraphTarget>,
) -> Result<dto::GraphCompilePreviewDto, String> {
    let target = target.map(tessera::graph_target_to_internal).transpose()?;
    let preview = ops::graph_compile_preview(state, target)?;
    transport::translate(preview)
}

pub fn compile_project_preview(
    state: &dto::SharedAppState,
) -> Result<dto::ProjectCompilePreviewDto, String> {
    let preview = ops::project_compile_preview(state)?;
    transport::translate(preview)
}

pub fn graph_pick_target_param(
    state: &dto::SharedAppState,
    from: dto::GridPos,
    to_node: dto::GridPos,
    target: dto::CadenceGraphTarget,
    to_param: Option<String>,
) -> Result<dto::GraphPickTargetParamDto, String> {
    let from = tessera::grid_pos_to_tessera(from);
    let to_node = tessera::grid_pos_to_tessera(to_node);
    let target = tessera::graph_target_to_internal(target)?;
    let probe = ops::graph_pick_target_param(state, from, to_node, target, to_param)?;
    transport::translate(probe)
}

pub fn graph_pick_target_param_on_graph(
    state: &dto::SharedAppState,
    graph: dto::GraphSnapshotDto,
    from: dto::GridPos,
    to_node: dto::GridPos,
    target: dto::CadenceGraphTarget,
    to_param: Option<String>,
) -> Result<dto::GraphPickTargetParamDto, String> {
    let from = tessera::grid_pos_to_tessera(from);
    let to_node = tessera::grid_pos_to_tessera(to_node);
    let target = tessera::graph_target_to_internal(target)?;
    let probe =
        ops::graph_pick_target_param_on_graph(state, graph, from, to_node, target, to_param)?;
    transport::translate(probe)
}

pub fn apply_graph_ops(
    state: &dto::SharedAppState,
    ops: Vec<dto::GraphOp>,
    request_id: Option<String>,
    target: dto::CadenceGraphTarget,
) -> Result<dto::GraphApplyResultDto, Vec<dto::DiagnosticDto>> {
    let ops = tessera::graph_ops_to_internal(ops).map_err(dto::invalid_graph_op_diagnostic)?;
    let target =
        tessera::graph_target_to_internal(target).map_err(dto::invalid_graph_op_diagnostic)?;
    match ops::graph_apply_ops(state, ops, request_id, target) {
        Ok(result) => transport::translate(result).map_err(dto::invalid_graph_op_diagnostic),
        Err(error) => transport::translate(error).map_err(dto::invalid_graph_op_diagnostic),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        application::project::ops::project_new_internal, domain::script::ScriptInputContext,
        infrastructure::dto::SharedAppState, infrastructure::dto::StreamKind,
    };

    #[test]
    fn graph_piece_catalog_exposes_canonical_script_input_contexts() {
        let state = SharedAppState::new();
        {
            let mut store = state.store.lock().expect("app state lock");
            project_new_internal(&mut store, Some("Catalog".to_string())).expect("seed project");
        }

        let defs = graph_piece_catalog(&state, None).expect("catalog");

        let sound = defs
            .iter()
            .find(|piece| piece.id == "cadence.sound")
            .expect("sound piece");
        let sound_value = sound
            .params
            .iter()
            .find(|param| param.id == "value")
            .expect("sound value");
        let sound_pattern = sound
            .params
            .iter()
            .find(|param| param.id == "pattern")
            .expect("sound pattern");
        assert_eq!(
            sound_value.input_context,
            Some(ScriptInputContext::SourcePattern)
        );
        assert_eq!(sound_pattern.input_context, None);

        let note = defs
            .iter()
            .find(|piece| piece.id == "cadence.note")
            .expect("note piece");
        let note_value = note
            .params
            .iter()
            .find(|param| param.id == "value")
            .expect("note value");
        assert_eq!(
            note_value.input_context,
            Some(ScriptInputContext::NotePattern)
        );

        let mask = defs
            .iter()
            .find(|piece| piece.id == "cadence.mask")
            .expect("mask piece");
        let mask_by = mask
            .params
            .iter()
            .find(|param| param.id == "by")
            .expect("mask by");
        assert_eq!(
            mask_by.input_context,
            Some(ScriptInputContext::StructuralPattern)
        );
    }

    #[test]
    fn graph_piece_catalog_exposes_picker_metadata_for_new_input_tiles() {
        let state = SharedAppState::new();
        {
            let mut store = state.store.lock().expect("app state lock");
            project_new_internal(&mut store, Some("Catalog".to_string())).expect("seed project");
        }

        let defs = graph_piece_catalog(&state, None).expect("catalog");

        for piece_id in [
            "cadence.container.basic",
            "cadence.container.subdivide",
            "cadence.container.alternate",
            "cadence.container.parallel",
            "cadence.atom.note",
            "cadence.atom.scalar",
            "cadence.atom.rest",
            "cadence.atom.operator.elongation",
            "cadence.atom.operator.pitch_shift",
            "cadence.atom.operator.slow",
            "cadence.atom.operator.fast",
        ] {
            let piece = defs
                .iter()
                .find(|piece| piece.id == piece_id)
                .unwrap_or_else(|| panic!("missing Tessera piece {piece_id}"));
            assert_eq!(piece.category, "constant");
            assert_eq!(piece.stream_kind, Some(StreamKind::Pattern));
        }

        let control_input = defs
            .iter()
            .find(|piece| piece.id == "cadence.control_input")
            .expect("control input piece");
        assert_eq!(control_input.category, "constant");
        assert_eq!(control_input.stream_kind, Some(StreamKind::Control));

        let args_connector = defs
            .iter()
            .find(|piece| piece.id == "args_connector")
            .expect("args connector piece");
        assert_eq!(args_connector.category, "connector");
    }
}
