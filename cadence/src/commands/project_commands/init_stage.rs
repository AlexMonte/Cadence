use super::*;

pub(super) fn default_trick_graph(name: &str) -> Graph {
    Graph {
        nodes: BTreeMap::new(),
        edges: BTreeMap::new(),
        name: name.to_string(),
        cols: STANDARD_GRAPH_COLS,
        rows: STANDARD_GRAPH_ROWS,
    }
}

pub(super) fn init_stage_snapshot(project: &CadenceProjectDocument) -> InitStageSnapshotDto {
    InitStageSnapshotDto::from(&project.init_stage)
}

pub(super) fn apply_init_stage_ops(
    project: &mut CadenceProjectDocument,
    ops: &[InitStageOp],
) -> AppResult<bool> {
    let mut changed = false;

    for op in ops {
        match op {
            InitStageOp::SetCps { expr } => {
                let normalized = expr
                    .as_deref()
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                    .map(ToOwned::to_owned);
                if project.init_stage.cps_expr != normalized {
                    project.init_stage.cps_expr = normalized;
                    changed = true;
                }
            }
            InitStageOp::SampleLoadUpsert {
                id,
                source,
                aliases,
            } => {
                let trimmed_id = id.trim();
                if trimmed_id.is_empty() {
                    return Err(AppError::InvalidInput(
                        "sample load id cannot be empty".into(),
                    ));
                }
                let trimmed_source = source.trim();
                if trimmed_source.is_empty() {
                    return Err(AppError::InvalidInput(
                        "sample load source cannot be empty".into(),
                    ));
                }
                let next = crate::model::CadenceSampleLoad {
                    id: trimmed_id.to_string(),
                    source: trimmed_source.to_string(),
                    aliases: aliases.clone(),
                };
                if let Some(existing) = project
                    .init_stage
                    .sample_loads
                    .iter_mut()
                    .find(|sample| sample.id == next.id)
                {
                    if *existing != next {
                        *existing = next;
                        changed = true;
                    }
                } else {
                    project.init_stage.sample_loads.push(next);
                    changed = true;
                }
            }
            InitStageOp::SampleLoadRemove { id } => {
                let before = project.init_stage.sample_loads.len();
                project
                    .init_stage
                    .sample_loads
                    .retain(|sample| sample.id != *id);
                changed |= project.init_stage.sample_loads.len() != before;
            }
            InitStageOp::TrickCreate { id, name, graph } => {
                let trimmed_id = id.trim();
                if trimmed_id.is_empty() {
                    return Err(AppError::InvalidInput("trick id cannot be empty".into()));
                }
                let trimmed_name = name.trim();
                if trimmed_name.is_empty() {
                    return Err(AppError::InvalidInput("trick name cannot be empty".into()));
                }
                if project
                    .init_stage
                    .tricks
                    .iter()
                    .any(|trick| trick.id == trimmed_id)
                {
                    return Err(AppError::InvalidInput(format!(
                        "trick id '{}' already exists",
                        trimmed_id
                    )));
                }
                project
                    .init_stage
                    .tricks
                    .push(crate::model::CadenceTrickDef {
                        id: trimmed_id.to_string(),
                        name: trimmed_name.to_string(),
                        graph: graph
                            .clone()
                            .unwrap_or_else(|| default_trick_graph(trimmed_name)),
                    });
                changed = true;
            }
            InitStageOp::TrickRename { id, name } => {
                let trimmed_name = name.trim();
                if trimmed_name.is_empty() {
                    return Err(AppError::InvalidInput("trick name cannot be empty".into()));
                }
                let trick = project
                    .init_stage
                    .tricks
                    .iter_mut()
                    .find(|trick| trick.id == *id)
                    .ok_or_else(|| AppError::InvalidInput(format!("unknown trick '{}'", id)))?;
                if trick.name != trimmed_name {
                    trick.name = trimmed_name.to_string();
                    if trick.graph.name.trim().is_empty() {
                        trick.graph.name = trimmed_name.to_string();
                    }
                    changed = true;
                }
            }
            InitStageOp::TrickDelete { id } => {
                let before = project.init_stage.tricks.len();
                project.init_stage.tricks.retain(|trick| trick.id != *id);
                changed |= project.init_stage.tricks.len() != before;
            }
        }
    }

    Ok(changed)
}
