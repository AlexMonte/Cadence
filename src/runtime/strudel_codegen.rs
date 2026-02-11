//! Deterministic conversion from project model scope graphs to runtime code.

use std::fmt;

use crate::core::{ModelNode, NodeKind, Project, ScopeId, SequencerPatternData};
use crate::runtime::music_ir::{
    PatternExpr, PatternOp, ScopeProgram, StepGridExpr, TransformExpr, TransformOp,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CodegenError {
    ScopeNotFound(ScopeId),
    MissingOutputNode,
    MissingPatternNode,
    InvalidPatternParam,
    UnsupportedTransform(String),
}

impl fmt::Display for CodegenError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ScopeNotFound(scope) => write!(f, "scope not found: {scope}"),
            Self::MissingOutputNode => write!(f, "no Output node found in scope"),
            Self::MissingPatternNode => write!(f, "no Pattern node found in scope"),
            Self::InvalidPatternParam => write!(f, "invalid pattern parameter"),
            Self::UnsupportedTransform(name) => write!(f, "unsupported transform '{name}'"),
        }
    }
}

impl std::error::Error for CodegenError {}

pub fn generate_scope_code(project: &Project, scope_id: ScopeId) -> Result<String, CodegenError> {
    let program = generate_scope_ir(project, scope_id)?;
    render_scope_program(&program)
}

pub fn generate_scope_ir(
    project: &Project,
    scope_id: ScopeId,
) -> Result<ScopeProgram, CodegenError> {
    let scope = project
        .model
        .scopes
        .get(&scope_id)
        .ok_or(CodegenError::ScopeNotFound(scope_id))?;

    let ordered_nodes: Vec<&ModelNode> = scope
        .node_ids
        .iter()
        .filter_map(|node_id| project.model.nodes.get(node_id))
        .collect();

    if !ordered_nodes
        .iter()
        .any(|node| matches!(node.kind, NodeKind::Output))
    {
        return Err(CodegenError::MissingOutputNode);
    }

    let pattern_node = ordered_nodes
        .iter()
        .copied()
        .find(|node| matches!(node.kind, NodeKind::Pattern { .. }))
        .ok_or(CodegenError::MissingPatternNode)?;

    let pattern = PatternOp {
        source_node: pattern_node.id,
        expr: pattern_ir(pattern_node)?,
    };
    let mut transforms = Vec::new();

    for node in ordered_nodes {
        if let NodeKind::Transform { ref transform_type } = node.kind {
            transforms.push(TransformOp {
                source_node: node.id,
                expr: transform_ir(transform_type, node)?,
            });
        }
    }

    Ok(ScopeProgram {
        scope_id,
        pattern,
        transforms,
    })
}

fn render_scope_program(program: &ScopeProgram) -> Result<String, CodegenError> {
    let mut expr = render_pattern(&program.pattern.expr);
    for transform in &program.transforms {
        expr = render_transform(&expr, &transform.expr);
    }
    Ok(format!("let main = {expr};\nmain\n"))
}

fn pattern_ir(node: &ModelNode) -> Result<PatternExpr, CodegenError> {
    if let Some(expr) = sample_grid_ir(node) {
        return Ok(expr);
    }

    let NodeKind::Pattern { pattern_type } = &node.kind else {
        return Err(CodegenError::MissingPatternNode);
    };

    let pattern = string_param(node, "pattern")
        .map(str::trim)
        .filter(|value| !value.is_empty());

    let out = match (pattern_type.as_str(), pattern) {
        ("seq", Some(value)) => {
            let normalized = normalize_sequence_pattern(value);
            if normalized.is_empty() {
                return Err(CodegenError::InvalidPatternParam);
            }
            PatternExpr::Sequence { notes: normalized }
        }
        ("sample", Some(value)) => PatternExpr::Sample {
            value: value.to_string(),
        },
        (_, Some(value)) => PatternExpr::Mini {
            value: value.to_string(),
        },
        ("seq", None) => PatternExpr::Sequence {
            notes: "0 1 2 3".to_string(),
        },
        ("sample", None) => PatternExpr::Sample {
            value: "bd".to_string(),
        },
        (_, None) => PatternExpr::Sample {
            value: "bd".to_string(),
        },
    };

    Ok(out)
}

fn sample_grid_ir(node: &ModelNode) -> Option<PatternExpr> {
    let payload = SequencerPatternData::from_model_node(node)?;

    Some(PatternExpr::StepGrid(StepGridExpr {
        rows: payload.rows as usize,
        cols: payload.cols as usize,
        bits: payload.bits,
        default_sample: payload.default_sample,
        row_samples: payload.row_samples,
        velocities: optional_payload(payload.velocities),
        accents: optional_payload(payload.accents),
        probabilities: optional_payload(payload.probabilities),
    }))
}

fn optional_payload<T>(values: Vec<T>) -> Option<Vec<T>> {
    if values.is_empty() {
        None
    } else {
        Some(values)
    }
}

fn transform_ir(transform_type: &str, node: &ModelNode) -> Result<TransformExpr, CodegenError> {
    match transform_type {
        "speed" => Ok(TransformExpr::Speed(number_param(
            node,
            &["factor", "value", "amount", "speed"],
            1.0,
        ))),
        "gain" => Ok(TransformExpr::Gain(number_param(
            node,
            &["factor", "value", "amount", "gain"],
            1.0,
        ))),
        "pan" => Ok(TransformExpr::Pan(number_param(
            node,
            &["value", "amount", "pan"],
            0.0,
        ))),
        other => Err(CodegenError::UnsupportedTransform(other.to_string())),
    }
}

fn render_pattern(pattern: &PatternExpr) -> String {
    match pattern {
        PatternExpr::Sequence { notes } => format!("n(\"{}\")", escape_string(notes)),
        PatternExpr::Sample { value } => format!("s(\"{}\")", escape_string(value)),
        PatternExpr::Mini { value } => format!("mini(\"{}\")", escape_string(value)),
        PatternExpr::StepGrid(grid) => render_step_grid(grid),
    }
}

fn render_step_grid(grid: &StepGridExpr) -> String {
    let mut layers = Vec::new();

    for row in 0..grid.rows {
        layers.extend(render_step_row_layers(grid, row));
    }

    if layers.is_empty() {
        let silent_tokens = std::iter::repeat_n("_", grid.cols)
            .collect::<Vec<_>>()
            .join(" ");
        return render_step_lane(grid.default_sample.as_str(), &silent_tokens);
    }

    if layers.len() == 1 {
        layers
            .into_iter()
            .next()
            .unwrap_or_else(|| render_step_lane(grid.default_sample.as_str(), "_"))
    } else {
        format!("stack({})", layers.join(", "))
    }
}

fn row_tokens(bits: &[bool], row: usize, cols: usize) -> String {
    (0..cols)
        .map(|col| {
            if bits.get(row * cols + col).copied().unwrap_or(false) {
                "x"
            } else {
                "_"
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn render_step_row_layers(grid: &StepGridExpr, row: usize) -> Vec<String> {
    if !has_step_dynamics(grid) {
        if !is_row_active(&grid.bits, row, grid.cols) {
            return Vec::new();
        }
        let sample = step_row_sample(grid, row);
        let tokens = row_tokens(&grid.bits, row, grid.cols);
        return vec![render_step_lane(sample, &tokens)];
    }

    let sample = step_row_sample(grid, row);
    let mut regular = vec!["_"; grid.cols];
    let mut accent = vec!["_"; grid.cols];
    let mut regular_active = false;
    let mut accent_active = false;

    for col in 0..grid.cols {
        let idx = row * grid.cols + col;
        if !grid.bits.get(idx).copied().unwrap_or(false) {
            continue;
        }

        // Phase 3C is deterministic and codegen-neutral:
        // probability/velocity gate only when explicitly zeroed out.
        let velocity = step_value(&grid.velocities, idx);
        let probability = step_value(&grid.probabilities, idx);
        if velocity <= 0.0 || probability <= 0.0 {
            continue;
        }

        if step_flag(&grid.accents, idx) {
            accent[col] = "x";
            accent_active = true;
        } else {
            regular[col] = "x";
            regular_active = true;
        }
    }

    let mut lanes = Vec::new();
    if regular_active {
        lanes.push(render_step_lane(sample, &regular.join(" ")));
    }
    if accent_active {
        lanes.push(format!(
            "({}).gain({})",
            render_step_lane(sample, &accent.join(" ")),
            fmt_number(1.25)
        ));
    }
    lanes
}

fn has_step_dynamics(grid: &StepGridExpr) -> bool {
    grid.velocities.is_some() || grid.accents.is_some() || grid.probabilities.is_some()
}

fn step_row_sample(grid: &StepGridExpr, row: usize) -> &str {
    grid.row_samples
        .get(row)
        .map(String::as_str)
        .filter(|sample| !sample.trim().is_empty())
        .unwrap_or(grid.default_sample.as_str())
}

fn step_value(values: &Option<Vec<f32>>, idx: usize) -> f32 {
    values
        .as_ref()
        .and_then(|items| items.get(idx))
        .copied()
        .unwrap_or(1.0)
}

fn step_flag(values: &Option<Vec<bool>>, idx: usize) -> bool {
    values
        .as_ref()
        .and_then(|items| items.get(idx))
        .copied()
        .unwrap_or(false)
}

fn render_step_lane(sample: &str, tokens: &str) -> String {
    format!(
        "s(\"{}\").struct(\"<[{}]>\")",
        escape_string(sample),
        escape_string(tokens),
    )
}

fn is_row_active(bits: &[bool], row: usize, cols: usize) -> bool {
    (0..cols).any(|col| bits.get(row * cols + col).copied().unwrap_or(false))
}

fn render_transform(expr: &str, transform: &TransformExpr) -> String {
    match transform {
        TransformExpr::Speed(value) => format!("({expr}).speed({})", fmt_number(*value)),
        TransformExpr::Gain(value) => format!("({expr}).gain({})", fmt_number(*value)),
        TransformExpr::Pan(value) => format!("({expr}).pan({})", fmt_number(*value)),
    }
}

fn normalize_sequence_pattern(input: &str) -> String {
    input
        .replace(['[', ']', ','], " ")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn string_param<'a>(node: &'a ModelNode, key: &str) -> Option<&'a str> {
    node.params.get(key)?.as_str()
}

fn number_param(node: &ModelNode, keys: &[&str], default: f64) -> f64 {
    for key in keys {
        if let Some(value) = node.params.get(*key)
            && let Some(number) = value.as_f64()
        {
            return number;
        }
    }
    default
}

fn fmt_number(value: f64) -> String {
    let mut out = format!("{value:.6}");
    while out.contains('.') && out.ends_with('0') {
        out.pop();
    }
    if out.ends_with('.') {
        out.push('0');
    }
    if !out.contains('.') {
        out.push_str(".0");
    }
    out
}

fn escape_string(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use crate::core::{ModelNode, NodeId, NodeKind, Project};

    use super::*;

    fn make_node(kind: NodeKind, parent_scope: ScopeId) -> ModelNode {
        ModelNode {
            id: NodeId::new(),
            kind,
            parent_scope,
            params: HashMap::new(),
            input_ports: vec![],
            output_ports: vec![],
        }
    }

    #[test]
    fn generates_pattern_transform_output_pipeline() {
        let mut project = Project::new("codegen".to_string());
        let scope = project.root_scope();

        let mut pattern = make_node(
            NodeKind::Pattern {
                pattern_type: "seq".to_string(),
            },
            scope,
        );
        pattern
            .params
            .insert("pattern".to_string(), serde_json::json!("[1, 2, 3, 4]"));
        let pattern_id = pattern.id;

        let mut speed = make_node(
            NodeKind::Transform {
                transform_type: "speed".to_string(),
            },
            scope,
        );
        speed
            .params
            .insert("factor".to_string(), serde_json::json!(2.0));
        let speed_id = speed.id;

        let output = make_node(NodeKind::Output, scope);
        let output_id = output.id;

        project.add_node(scope, pattern);
        project.add_node(scope, speed);
        project.add_node(scope, output);

        // Ensure stable node order for deterministic output.
        let scope_nodes = &project.model.scopes.get(&scope).unwrap().node_ids;
        assert_eq!(scope_nodes, &vec![pattern_id, speed_id, output_id]);

        let code = generate_scope_code(&project, scope).expect("codegen should succeed");
        assert_eq!(code, "let main = (n(\"1 2 3 4\")).speed(2.0);\nmain\n");
    }

    #[test]
    fn errors_when_output_missing() {
        let mut project = Project::new("missing-output".to_string());
        let scope = project.root_scope();
        let pattern = make_node(
            NodeKind::Pattern {
                pattern_type: "default".to_string(),
            },
            scope,
        );
        project.add_node(scope, pattern);

        let err = generate_scope_code(&project, scope).expect_err("missing output must fail");
        assert_eq!(err, CodegenError::MissingOutputNode);
    }

    #[test]
    fn errors_when_pattern_missing() {
        let mut project = Project::new("missing-pattern".to_string());
        let scope = project.root_scope();
        let output = make_node(NodeKind::Output, scope);
        project.add_node(scope, output);

        let err = generate_scope_code(&project, scope).expect_err("missing pattern must fail");
        assert_eq!(err, CodegenError::MissingPatternNode);
    }

    #[test]
    fn errors_when_transform_is_unsupported() {
        let mut project = Project::new("unsupported-transform".to_string());
        let scope = project.root_scope();
        let pattern = make_node(
            NodeKind::Pattern {
                pattern_type: "default".to_string(),
            },
            scope,
        );
        let transform = make_node(
            NodeKind::Transform {
                transform_type: "warp".to_string(),
            },
            scope,
        );
        let output = make_node(NodeKind::Output, scope);

        project.add_node(scope, pattern);
        project.add_node(scope, transform);
        project.add_node(scope, output);

        let err =
            generate_scope_code(&project, scope).expect_err("unsupported transform must fail");
        assert_eq!(err, CodegenError::UnsupportedTransform("warp".to_string()));
    }

    #[test]
    fn output_is_deterministic_for_same_model() {
        let mut project = Project::new("determinism".to_string());
        let scope = project.root_scope();
        let mut pattern = make_node(
            NodeKind::Pattern {
                pattern_type: "default".to_string(),
            },
            scope,
        );
        pattern
            .params
            .insert("pattern".to_string(), serde_json::json!("bd sn"));
        let output = make_node(NodeKind::Output, scope);

        project.add_node(scope, pattern);
        project.add_node(scope, output);

        let a = generate_scope_code(&project, scope).unwrap();
        let b = generate_scope_code(&project, scope).unwrap();
        assert_eq!(a, b);
    }

    #[test]
    fn scope_ir_is_deterministic_for_same_model() {
        let mut project = Project::new("ir-determinism".to_string());
        let scope = project.root_scope();
        let mut pattern = make_node(
            NodeKind::Pattern {
                pattern_type: "sample".to_string(),
            },
            scope,
        );
        pattern
            .params
            .insert("pattern".to_string(), serde_json::json!("bd ~ sn ~"));
        let output = make_node(NodeKind::Output, scope);
        project.add_node(scope, pattern);
        project.add_node(scope, output);

        let a = generate_scope_ir(&project, scope).unwrap();
        let b = generate_scope_ir(&project, scope).unwrap();
        assert_eq!(a, b);
    }

    #[test]
    fn scope_ir_preserves_source_node_ids() {
        let mut project = Project::new("ir-source-ids".to_string());
        let scope = project.root_scope();

        let pattern = make_node(
            NodeKind::Pattern {
                pattern_type: "seq".to_string(),
            },
            scope,
        );
        let pattern_id = pattern.id;
        let transform = make_node(
            NodeKind::Transform {
                transform_type: "gain".to_string(),
            },
            scope,
        );
        let transform_id = transform.id;
        let output = make_node(NodeKind::Output, scope);

        project.add_node(scope, pattern);
        project.add_node(scope, transform);
        project.add_node(scope, output);

        let ir = generate_scope_ir(&project, scope).unwrap();
        assert_eq!(ir.scope_id, scope);
        assert_eq!(ir.pattern.source_node, pattern_id);
        assert_eq!(ir.transforms.len(), 1);
        assert_eq!(ir.transforms[0].source_node, transform_id);
    }

    #[test]
    fn generates_stack_from_grid_bits_and_row_samples() {
        let mut project = Project::new("grid-stack".to_string());
        let scope = project.root_scope();

        let mut pattern = make_node(
            NodeKind::Pattern {
                pattern_type: "sample".to_string(),
            },
            scope,
        );
        pattern
            .params
            .insert("grid_rows".to_string(), serde_json::json!(2));
        pattern
            .params
            .insert("grid_cols".to_string(), serde_json::json!(4));
        pattern
            .params
            .insert("grid_bits".to_string(), serde_json::json!("10010010"));
        pattern
            .params
            .insert("row_samples".to_string(), serde_json::json!(["bd", "sn"]));
        pattern
            .params
            .insert("sample".to_string(), serde_json::json!("bd"));

        let output = make_node(NodeKind::Output, scope);
        project.add_node(scope, pattern);
        project.add_node(scope, output);

        let code = generate_scope_code(&project, scope).expect("grid codegen should succeed");
        assert_eq!(
            code,
            "let main = stack(s(\"bd\").struct(\"<[x _ _ x]>\"), s(\"sn\").struct(\"<[_ _ x _]>\"));\nmain\n"
        );
    }

    #[test]
    fn generates_single_step_lane_struct_for_one_active_row() {
        let mut project = Project::new("grid-single-row".to_string());
        let scope = project.root_scope();

        let mut pattern = make_node(
            NodeKind::Pattern {
                pattern_type: "sample".to_string(),
            },
            scope,
        );
        pattern
            .params
            .insert("grid_rows".to_string(), serde_json::json!(1));
        pattern
            .params
            .insert("grid_cols".to_string(), serde_json::json!(4));
        pattern
            .params
            .insert("grid_bits".to_string(), serde_json::json!("1001"));
        pattern
            .params
            .insert("row_samples".to_string(), serde_json::json!(["bd"]));
        pattern
            .params
            .insert("sample".to_string(), serde_json::json!("bd"));

        let output = make_node(NodeKind::Output, scope);
        project.add_node(scope, pattern);
        project.add_node(scope, output);

        let code = generate_scope_code(&project, scope).expect("single lane codegen should work");
        assert_eq!(
            code,
            "let main = s(\"bd\").struct(\"<[x _ _ x]>\");\nmain\n"
        );
    }

    #[test]
    fn all_silent_step_grid_uses_default_sample_struct_lane() {
        let mut project = Project::new("grid-all-silent".to_string());
        let scope = project.root_scope();

        let mut pattern = make_node(
            NodeKind::Pattern {
                pattern_type: "sample".to_string(),
            },
            scope,
        );
        pattern
            .params
            .insert("grid_rows".to_string(), serde_json::json!(2));
        pattern
            .params
            .insert("grid_cols".to_string(), serde_json::json!(4));
        pattern
            .params
            .insert("grid_bits".to_string(), serde_json::json!("00000000"));
        pattern
            .params
            .insert("row_samples".to_string(), serde_json::json!(["bd", "sn"]));
        pattern
            .params
            .insert("sample".to_string(), serde_json::json!("bd"));

        let output = make_node(NodeKind::Output, scope);
        project.add_node(scope, pattern);
        project.add_node(scope, output);

        let code = generate_scope_code(&project, scope).expect("silent lane codegen should work");
        assert_eq!(
            code,
            "let main = s(\"bd\").struct(\"<[_ _ _ _]>\");\nmain\n"
        );
    }

    #[test]
    fn step_grid_ir_carries_optional_typed_step_payload() {
        let mut project = Project::new("grid-typed-payload".to_string());
        let scope = project.root_scope();

        let mut pattern = make_node(
            NodeKind::Pattern {
                pattern_type: "sample".to_string(),
            },
            scope,
        );
        pattern
            .params
            .insert("grid_rows".to_string(), serde_json::json!(2));
        pattern
            .params
            .insert("grid_cols".to_string(), serde_json::json!(2));
        pattern
            .params
            .insert("grid_bits".to_string(), serde_json::json!("1100"));
        pattern
            .params
            .insert("row_samples".to_string(), serde_json::json!(["bd", "sn"]));
        pattern
            .params
            .insert("sample".to_string(), serde_json::json!("bd"));
        pattern.params.insert(
            "grid_velocities".to_string(),
            serde_json::json!([1.0, 0.5, 0.25, 0.75]),
        );
        pattern.params.insert(
            "grid_accents".to_string(),
            serde_json::json!([true, false, true, false]),
        );
        pattern.params.insert(
            "grid_probabilities".to_string(),
            serde_json::json!([1.0, 0.6, 0.4, 0.9]),
        );

        let output = make_node(NodeKind::Output, scope);
        project.add_node(scope, pattern);
        project.add_node(scope, output);

        let ir = generate_scope_ir(&project, scope).expect("IR generation should succeed");
        let PatternExpr::StepGrid(step) = ir.pattern.expr else {
            panic!("expected StepGrid IR pattern");
        };

        let velocities = step.velocities.expect("velocities should be present");
        assert_eq!(velocities.len(), 4);
        assert!((velocities[0] - 1.0).abs() < f32::EPSILON);
        assert!((velocities[1] - 0.5).abs() < f32::EPSILON);
        assert!((velocities[2] - 0.25).abs() < f32::EPSILON);
        assert!((velocities[3] - 0.75).abs() < f32::EPSILON);
        assert_eq!(step.accents, Some(vec![true, false, true, false]));
        let probabilities = step.probabilities.expect("probabilities should be present");
        assert_eq!(probabilities.len(), 4);
        assert!((probabilities[0] - 1.0).abs() < f32::EPSILON);
        assert!((probabilities[1] - 0.6).abs() < f32::EPSILON);
        assert!((probabilities[2] - 0.4).abs() < f32::EPSILON);
        assert!((probabilities[3] - 0.9).abs() < f32::EPSILON);

        let code = generate_scope_code(&project, scope).expect("codegen should still succeed");
        assert_eq!(
            code,
            "let main = stack(s(\"bd\").struct(\"<[_ x]>\"), (s(\"bd\").struct(\"<[x _]>\"))\
.gain(1.25));\nmain\n"
        );
    }

    #[test]
    fn step_grid_dynamics_zero_velocity_and_probability_mute_steps() {
        let mut project = Project::new("grid-dynamics-mute".to_string());
        let scope = project.root_scope();

        let mut pattern = make_node(
            NodeKind::Pattern {
                pattern_type: "sample".to_string(),
            },
            scope,
        );
        pattern
            .params
            .insert("grid_rows".to_string(), serde_json::json!(1));
        pattern
            .params
            .insert("grid_cols".to_string(), serde_json::json!(4));
        pattern
            .params
            .insert("grid_bits".to_string(), serde_json::json!("1111"));
        pattern
            .params
            .insert("sample".to_string(), serde_json::json!("bd"));
        pattern.params.insert(
            "grid_velocities".to_string(),
            serde_json::json!([1.0, 0.0, 1.0, 1.0]),
        );
        pattern.params.insert(
            "grid_probabilities".to_string(),
            serde_json::json!([1.0, 1.0, 0.0, 1.0]),
        );

        let output = make_node(NodeKind::Output, scope);
        project.add_node(scope, pattern);
        project.add_node(scope, output);

        let code = generate_scope_code(&project, scope).expect("codegen should still succeed");
        assert_eq!(
            code,
            "let main = s(\"bd\").struct(\"<[x _ _ x]>\");\nmain\n"
        );
    }
}
