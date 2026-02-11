//! Sequencer pattern payload normalization for sample-pattern nodes.

use serde_json::json;

use crate::core::{ModelNode, NodeKind};

pub const PARAM_SAMPLE: &str = "sample";
pub const PARAM_ROW_SAMPLES: &str = "row_samples";
pub const PARAM_PATTERN: &str = "pattern";
pub const PARAM_GRID_ROWS: &str = "grid_rows";
pub const PARAM_GRID_COLS: &str = "grid_cols";
pub const PARAM_GRID_BITS: &str = "grid_bits";
pub const PARAM_GRID_VELOCITIES: &str = "grid_velocities";
pub const PARAM_GRID_ACCENTS: &str = "grid_accents";
pub const PARAM_GRID_PROBABILITIES: &str = "grid_probabilities";

pub const GRID_DIM_MIN: u8 = 1;
pub const GRID_DIM_MAX: u8 = 16;
pub const GRID_DIM_DEFAULT_ROWS: u8 = 4;
pub const GRID_DIM_DEFAULT_COLS: u8 = 4;
pub const DEFAULT_SAMPLE: &str = "bd";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SequencerPatternError {
    NonPatternNode,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SequencerPatternData {
    pub rows: u8,
    pub cols: u8,
    pub bits: Vec<bool>,
    pub row_samples: Vec<String>,
    pub default_sample: String,
    pub velocities: Vec<f32>,
    pub accents: Vec<bool>,
    pub probabilities: Vec<f32>,
}

impl Default for SequencerPatternData {
    fn default() -> Self {
        let rows = GRID_DIM_DEFAULT_ROWS;
        let cols = GRID_DIM_DEFAULT_COLS;
        SequencerPatternData {
            rows,
            cols,
            bits: vec![false; rows as usize * cols as usize],
            row_samples: vec![DEFAULT_SAMPLE.to_string(); rows as usize],
            default_sample: DEFAULT_SAMPLE.to_string(),
            velocities: Vec::new(),
            accents: Vec::new(),
            probabilities: Vec::new(),
        }
    }
}

impl SequencerPatternData {
    pub fn from_model_node(node: &ModelNode) -> Option<Self> {
        Self::from_node_params(node)
    }

    pub fn from_node_params(node: &ModelNode) -> Option<Self> {
        let NodeKind::Pattern { pattern_type } = &node.kind else {
            return None;
        };
        if pattern_type != "sample" {
            return None;
        }

        let pattern = node
            .params
            .get(PARAM_PATTERN)
            .and_then(|value| value.as_str())
            .map(str::trim)
            .filter(|value| !value.is_empty());

        let has_grid_payload = node.params.contains_key(PARAM_GRID_BITS)
            || node.params.contains_key(PARAM_GRID_ROWS)
            || node.params.contains_key(PARAM_GRID_COLS)
            || node.params.contains_key(PARAM_ROW_SAMPLES)
            || node.params.contains_key(PARAM_GRID_VELOCITIES)
            || node.params.contains_key(PARAM_GRID_ACCENTS)
            || node.params.contains_key(PARAM_GRID_PROBABILITIES);

        if !has_grid_payload && pattern.is_none() {
            return None;
        }

        let mut data = SequencerPatternData {
            rows: read_grid_dim(node, PARAM_GRID_ROWS, GRID_DIM_DEFAULT_ROWS),
            cols: read_grid_dim(node, PARAM_GRID_COLS, GRID_DIM_DEFAULT_COLS),
            bits: Vec::new(),
            row_samples: read_row_samples(node),
            default_sample: read_default_sample(node),
            velocities: read_f32_array(node, PARAM_GRID_VELOCITIES),
            accents: read_bool_array(node, PARAM_GRID_ACCENTS),
            probabilities: read_f32_array(node, PARAM_GRID_PROBABILITIES),
        };
        data.clamp_normalize();

        let total = data.rows as usize * data.cols as usize;
        data.bits = if let Some(bits) = node
            .params
            .get(PARAM_GRID_BITS)
            .and_then(|value| value.as_str())
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            deserialize_bits(bits, total)
        } else if let Some(pattern) = pattern {
            deserialize_pattern_fallback(pattern, data.cols as usize, total)
        } else {
            vec![false; total]
        };

        Some(data)
    }

    pub fn write_to_node(&self, node: &mut ModelNode) {
        let NodeKind::Pattern { pattern_type } = &mut node.kind else {
            return;
        };
        *pattern_type = "sample".to_string();

        let mut data = self.clone();
        data.clamp_normalize();

        node.params
            .insert(PARAM_SAMPLE.to_string(), json!(data.default_sample.clone()));
        node.params.insert(
            PARAM_ROW_SAMPLES.to_string(),
            json!(data.row_samples.clone()),
        );
        node.params
            .insert(PARAM_GRID_ROWS.to_string(), json!(data.rows as u64));
        node.params
            .insert(PARAM_GRID_COLS.to_string(), json!(data.cols as u64));
        node.params.insert(
            PARAM_GRID_BITS.to_string(),
            json!(serialize_bits(&data.bits)),
        );
        node.params.insert(
            PARAM_PATTERN.to_string(),
            json!(data.pattern_preview_string()),
        );
        if !data.velocities.is_empty() {
            node.params.insert(
                PARAM_GRID_VELOCITIES.to_string(),
                json!(data.velocities.clone()),
            );
        } else {
            node.params.remove(PARAM_GRID_VELOCITIES);
        }
        if !data.accents.is_empty() {
            node.params
                .insert(PARAM_GRID_ACCENTS.to_string(), json!(data.accents.clone()));
        } else {
            node.params.remove(PARAM_GRID_ACCENTS);
        }
        if !data.probabilities.is_empty() {
            node.params.insert(
                PARAM_GRID_PROBABILITIES.to_string(),
                json!(data.probabilities.clone()),
            );
        } else {
            node.params.remove(PARAM_GRID_PROBABILITIES);
        }
    }

    pub fn pattern_preview_string(&self) -> String {
        let mut data = self.clone();
        data.clamp_normalize();

        let rows = data.rows as usize;
        let cols = data.cols as usize;
        let mut lines = Vec::with_capacity(rows);

        for row in 0..rows {
            let sample = data
                .row_samples
                .get(row)
                .cloned()
                .unwrap_or_else(|| data.default_sample.clone());
            let mut tokens = Vec::with_capacity(cols);
            for col in 0..cols {
                let idx = row * cols + col;
                let active = data.bits.get(idx).copied().unwrap_or(false);
                if active {
                    tokens.push(sample.clone());
                } else {
                    tokens.push("~".to_string());
                }
            }
            lines.push(tokens.join(" "));
        }
        lines.join(" | ")
    }

    fn clamp_normalize(&mut self) {
        self.rows = self.rows.clamp(GRID_DIM_MIN, GRID_DIM_MAX);
        self.cols = self.cols.clamp(GRID_DIM_MIN, GRID_DIM_MAX);

        if self.default_sample.trim().is_empty() {
            self.default_sample = DEFAULT_SAMPLE.to_string();
        }

        if self.row_samples.is_empty() {
            self.row_samples.push(self.default_sample.clone());
        }
        for sample in &mut self.row_samples {
            if sample.trim().is_empty() {
                *sample = self.default_sample.clone();
            }
        }
        while self.row_samples.len() < self.rows as usize {
            let fallback = self
                .row_samples
                .last()
                .cloned()
                .unwrap_or_else(|| self.default_sample.clone());
            self.row_samples.push(fallback);
        }
        self.row_samples.truncate(self.rows as usize);

        let total = self.rows as usize * self.cols as usize;
        self.bits.resize(total, false);
        if !self.velocities.is_empty() {
            self.velocities.resize(total, 1.0);
            for velocity in &mut self.velocities {
                *velocity = normalize_unit_interval(*velocity, 1.0);
            }
        }
        if !self.accents.is_empty() {
            self.accents.resize(total, false);
        }
        if !self.probabilities.is_empty() {
            self.probabilities.resize(total, 1.0);
            for probability in &mut self.probabilities {
                *probability = normalize_unit_interval(*probability, 1.0);
            }
        }
    }
}

fn read_default_sample(node: &ModelNode) -> String {
    node.params
        .get(PARAM_SAMPLE)
        .and_then(|value| value.as_str())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or(DEFAULT_SAMPLE)
        .to_string()
}

fn read_row_samples(node: &ModelNode) -> Vec<String> {
    node.params
        .get(PARAM_ROW_SAMPLES)
        .and_then(|value| value.as_array())
        .map(|array| {
            array
                .iter()
                .filter_map(|value| value.as_str())
                .map(ToString::to_string)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default()
}

fn read_grid_dim(node: &ModelNode, key: &str, default_value: u8) -> u8 {
    node.params
        .get(key)
        .and_then(|value| value.as_u64())
        .map(|value| {
            if value > u8::MAX as u64 {
                GRID_DIM_MAX
            } else {
                (value as u8).clamp(GRID_DIM_MIN, GRID_DIM_MAX)
            }
        })
        .unwrap_or(default_value)
}

fn read_f32_array(node: &ModelNode, key: &str) -> Vec<f32> {
    node.params
        .get(key)
        .and_then(|value| value.as_array())
        .map(|array| {
            array
                .iter()
                .filter_map(|value| value.as_f64())
                .map(|value| value as f32)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default()
}

fn read_bool_array(node: &ModelNode, key: &str) -> Vec<bool> {
    node.params
        .get(key)
        .and_then(|value| value.as_array())
        .map(|array| {
            array
                .iter()
                .filter_map(|value| value.as_bool())
                .collect::<Vec<_>>()
        })
        .unwrap_or_default()
}

fn normalize_unit_interval(value: f32, fallback: f32) -> f32 {
    if value.is_finite() {
        value.clamp(0.0, 1.0)
    } else {
        fallback
    }
}

fn serialize_bits(bits: &[bool]) -> String {
    bits.iter()
        .map(|active| if *active { '1' } else { '0' })
        .collect()
}

fn deserialize_bits(bits: &str, total: usize) -> Vec<bool> {
    let mut out = vec![false; total];
    for (idx, bit) in bits.chars().enumerate().take(total) {
        out[idx] = bit == '1';
    }
    out
}

fn deserialize_pattern_fallback(pattern: &str, cols: usize, total: usize) -> Vec<bool> {
    let mut out = vec![false; total];
    for (col, token) in pattern.split_whitespace().take(cols).enumerate() {
        out[col] = !matches!(token, "~" | "-" | "_");
    }
    out
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use serde_json::json;

    use crate::core::{ModelNode, NodeId, NodeKind, ScopeId};

    use super::*;

    fn sample_pattern_node() -> ModelNode {
        ModelNode {
            id: NodeId::new(),
            kind: NodeKind::Pattern {
                pattern_type: "sample".to_string(),
            },
            parent_scope: ScopeId::new(),
            params: HashMap::new(),
            input_ports: vec![],
            output_ports: vec![],
        }
    }

    #[test]
    fn from_node_params_defaults_and_clamps() {
        let mut node = sample_pattern_node();
        node.params
            .insert(PARAM_PATTERN.to_string(), json!("bd ~ sn ~"));
        node.params
            .insert(PARAM_GRID_ROWS.to_string(), json!(99u64));
        node.params.insert(PARAM_GRID_COLS.to_string(), json!(0u64));
        node.params.insert(PARAM_SAMPLE.to_string(), json!(""));

        let data = SequencerPatternData::from_node_params(&node).unwrap();
        assert_eq!(data.rows, GRID_DIM_MAX);
        assert_eq!(data.cols, GRID_DIM_MIN);
        assert_eq!(data.default_sample, DEFAULT_SAMPLE);
        assert_eq!(data.row_samples.len(), GRID_DIM_MAX as usize);
        assert_eq!(
            data.bits.len(),
            GRID_DIM_MAX as usize * GRID_DIM_MIN as usize
        );
        assert!(data.bits[0]);
        assert!(data.velocities.is_empty());
        assert!(data.accents.is_empty());
        assert!(data.probabilities.is_empty());
    }

    #[test]
    fn oversized_grid_dimensions_are_clamped_without_wrapping() {
        let mut node = sample_pattern_node();
        node.params
            .insert(PARAM_PATTERN.to_string(), json!("bd ~ sn ~"));
        node.params
            .insert(PARAM_GRID_ROWS.to_string(), json!(u64::MAX));
        node.params
            .insert(PARAM_GRID_COLS.to_string(), json!(u64::MAX));

        let data = SequencerPatternData::from_node_params(&node).unwrap();
        assert_eq!(data.rows, GRID_DIM_MAX);
        assert_eq!(data.cols, GRID_DIM_MAX);
    }

    #[test]
    fn row_samples_are_padded_and_truncated() {
        let mut padded = sample_pattern_node();
        padded
            .params
            .insert(PARAM_GRID_ROWS.to_string(), json!(3u64));
        padded
            .params
            .insert(PARAM_GRID_COLS.to_string(), json!(2u64));
        padded
            .params
            .insert(PARAM_GRID_BITS.to_string(), json!("000000"));
        padded
            .params
            .insert(PARAM_ROW_SAMPLES.to_string(), json!(["bd"]));

        let padded_data = SequencerPatternData::from_node_params(&padded).unwrap();
        assert_eq!(
            padded_data.row_samples,
            vec!["bd".to_string(), "bd".to_string(), "bd".to_string()]
        );

        let mut truncated = sample_pattern_node();
        truncated
            .params
            .insert(PARAM_GRID_ROWS.to_string(), json!(2u64));
        truncated
            .params
            .insert(PARAM_GRID_COLS.to_string(), json!(2u64));
        truncated
            .params
            .insert(PARAM_GRID_BITS.to_string(), json!("0000"));
        truncated
            .params
            .insert(PARAM_ROW_SAMPLES.to_string(), json!(["bd", "sn", "hh"]));

        let truncated_data = SequencerPatternData::from_node_params(&truncated).unwrap();
        assert_eq!(
            truncated_data.row_samples,
            vec!["bd".to_string(), "sn".to_string()]
        );
    }

    #[test]
    fn bits_resize_matches_rows_cols() {
        let mut node = sample_pattern_node();
        node.params.insert(PARAM_GRID_ROWS.to_string(), json!(2u64));
        node.params.insert(PARAM_GRID_COLS.to_string(), json!(4u64));
        node.params
            .insert(PARAM_GRID_BITS.to_string(), json!("101"));

        let data = SequencerPatternData::from_node_params(&node).unwrap();
        assert_eq!(data.bits.len(), 8);
        assert_eq!(
            data.bits,
            vec![true, false, true, false, false, false, false, false]
        );
    }

    #[test]
    fn pattern_fallback_populates_first_row_when_grid_bits_missing() {
        let mut node = sample_pattern_node();
        node.params.insert(PARAM_GRID_ROWS.to_string(), json!(1u64));
        node.params.insert(PARAM_GRID_COLS.to_string(), json!(4u64));
        node.params
            .insert(PARAM_PATTERN.to_string(), json!("bd ~ _ -"));

        let data = SequencerPatternData::from_node_params(&node).unwrap();
        assert_eq!(data.bits, vec![true, false, false, false]);
    }

    #[test]
    fn write_to_node_roundtrip_is_stable() {
        let mut node = sample_pattern_node();
        let data = SequencerPatternData {
            rows: 2,
            cols: 4,
            bits: vec![true, false, false, true, false, false, true, false],
            row_samples: vec!["bd".to_string(), "sn".to_string()],
            default_sample: "bd".to_string(),
            velocities: Vec::new(),
            accents: Vec::new(),
            probabilities: Vec::new(),
        };

        data.write_to_node(&mut node);

        let roundtripped = SequencerPatternData::from_model_node(&node).unwrap();
        assert_eq!(roundtripped, data);
        assert_eq!(node.params.get(PARAM_GRID_BITS), Some(&json!("10010010")));
        assert!(!node.params.contains_key(PARAM_GRID_VELOCITIES));
        assert!(!node.params.contains_key(PARAM_GRID_ACCENTS));
        assert!(!node.params.contains_key(PARAM_GRID_PROBABILITIES));
    }

    #[test]
    fn advanced_payload_is_clamped_and_resized() {
        let mut node = sample_pattern_node();
        node.params.insert(PARAM_GRID_ROWS.to_string(), json!(2u64));
        node.params.insert(PARAM_GRID_COLS.to_string(), json!(3u64));
        node.params
            .insert(PARAM_GRID_BITS.to_string(), json!("111000"));
        node.params.insert(
            PARAM_GRID_VELOCITIES.to_string(),
            json!([1.2, -0.4, 0.5, 0.2]),
        );
        node.params
            .insert(PARAM_GRID_ACCENTS.to_string(), json!([true]));
        node.params.insert(
            PARAM_GRID_PROBABILITIES.to_string(),
            json!([0.1, 1.1, -0.3]),
        );

        let data = SequencerPatternData::from_node_params(&node).unwrap();
        assert_eq!(data.velocities, vec![1.0, 0.0, 0.5, 0.2, 1.0, 1.0]);
        assert_eq!(data.accents, vec![true, false, false, false, false, false]);
        assert_eq!(data.probabilities, vec![0.1, 1.0, 0.0, 1.0, 1.0, 1.0]);

        let mut roundtrip_node = sample_pattern_node();
        data.write_to_node(&mut roundtrip_node);
        assert!(roundtrip_node.params.contains_key(PARAM_GRID_VELOCITIES));
        assert!(roundtrip_node.params.contains_key(PARAM_GRID_ACCENTS));
        assert!(roundtrip_node.params.contains_key(PARAM_GRID_PROBABILITIES));
    }

    #[test]
    fn write_to_node_removes_stale_optional_payload_keys() {
        let mut node = sample_pattern_node();
        node.params
            .insert(PARAM_GRID_VELOCITIES.to_string(), json!([0.5]));
        node.params
            .insert(PARAM_GRID_ACCENTS.to_string(), json!([true]));
        node.params
            .insert(PARAM_GRID_PROBABILITIES.to_string(), json!([0.8]));

        let data = SequencerPatternData {
            rows: 1,
            cols: 1,
            bits: vec![false],
            row_samples: vec!["bd".to_string()],
            default_sample: "bd".to_string(),
            velocities: Vec::new(),
            accents: Vec::new(),
            probabilities: Vec::new(),
        };
        data.write_to_node(&mut node);

        assert!(!node.params.contains_key(PARAM_GRID_VELOCITIES));
        assert!(!node.params.contains_key(PARAM_GRID_ACCENTS));
        assert!(!node.params.contains_key(PARAM_GRID_PROBABILITIES));
    }
}
