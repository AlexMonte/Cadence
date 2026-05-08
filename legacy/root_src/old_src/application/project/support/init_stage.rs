use super::*;
use crate::application::tempo::parse_cps_expr;

fn normalize_aliases(aliases: &BTreeMap<String, String>) -> AppResult<BTreeMap<String, String>> {
    let mut normalized = BTreeMap::new();
    for (key, value) in aliases {
        let key = key.trim();
        let value = value.trim();
        if key.is_empty() {
            return Err(AppError::InvalidInput(
                "sample alias key cannot be empty".into(),
            ));
        }
        if value.is_empty() {
            return Err(AppError::InvalidInput(
                "sample alias value cannot be empty".into(),
            ));
        }
        normalized.insert(key.to_string(), value.to_string());
    }
    Ok(normalized)
}

pub(super) fn default_trick_graph(name: &str) -> Graph {
    Graph {
        nodes: BTreeMap::new(),
        edges: BTreeMap::new(),
        name: name.to_string(),
        cols: STANDARD_GRAPH_COLS,
        rows: STANDARD_GRAPH_ROWS,
    }
}

pub(super) fn init_stage_snapshot(project: &CadenceProjectDocument) -> InitStageSnapshot {
    InitStageSnapshot::from(project)
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
                parse_cps_expr(normalized.as_deref()).map_err(|reason| {
                    AppError::InvalidInput(format!("invalid cps expression: {reason}"))
                })?;
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
                let aliases = normalize_aliases(aliases)?;
                let next = crate::domain::project::CadenceSampleLoad {
                    id: trimmed_id.to_string(),
                    source: trimmed_source.to_string(),
                    aliases,
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
                if project.tricks().iter().any(|trick| trick.id == trimmed_id) {
                    return Err(AppError::InvalidInput(format!(
                        "trick id '{}' already exists",
                        trimmed_id
                    )));
                }
                project
                    .tricks_mut()
                    .push(crate::domain::project::CadenceTrickWorkspace {
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
                    .trick_mut(id)
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
                let before = project.tricks().len();
                project.tricks_mut().retain(|trick| trick.id != *id);
                changed |= project.tricks().len() != before;
            }
        }
    }

    validate_sample_loads(project.init_stage.sample_loads.as_slice())?;
    Ok(changed)
}
