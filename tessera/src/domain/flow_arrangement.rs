//! Strudel-style flow arrangements: timed segments of composed flow refs.
//!
//! Strudel's `stack(...)` maps to [`FlowComposer::Parallel`], not vertical atom stacking.

use serde::{Deserialize, Serialize};

use super::pattern_ir::{PatternNodeIr, Rational};
use super::program::NodeId;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
#[cfg_attr(feature = "bevy", derive(bevy_reflect::Reflect))]
pub enum FlowRef {
    Container { node: NodeId },
    Transform { node: NodeId },
    Arrangement { node: NodeId },
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[cfg_attr(feature = "bevy", derive(bevy_reflect::Reflect))]
pub struct FlowList {
    pub items: Vec<FlowRef>,
}

impl FlowList {
    pub fn new(items: Vec<FlowRef>) -> Self {
        Self { items }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
#[cfg_attr(feature = "bevy", derive(bevy_reflect::Reflect))]
pub enum FlowComposer {
    Ref(FlowRef),
    /// Strudel `stack(...)` — simultaneous merge.
    Parallel(FlowList),
    /// Strudel `s_polymeter(...)`.
    Polymeter(FlowList),
    /// Strudel `.layer(x => ..., x => ...)`.
    Layer {
        base: FlowRef,
        branches: Vec<FlowRef>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "bevy", derive(bevy_reflect::Reflect))]
pub struct ArrangementSegment {
    pub duration: Rational,
    pub composer: FlowComposer,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[cfg_attr(feature = "bevy", derive(bevy_reflect::Reflect))]
pub struct Arrangement {
    pub segments: Vec<ArrangementSegment>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
#[cfg_attr(feature = "bevy", derive(bevy_reflect::Reflect))]
pub enum ArrangementReject {
    EmptyArrangement,
    EmptyFlowList,
    InvalidDuration(Rational),
    UnknownFlowRef(FlowRef),
}

pub struct LowerCtx<'a, F> {
    pub resolve: &'a F,
}

impl Arrangement {
    pub fn lower<F>(&self, ctx: &LowerCtx<'_, F>) -> Result<PatternNodeIr, ArrangementReject>
    where
        F: Fn(&FlowRef) -> Result<PatternNodeIr, ArrangementReject>,
    {
        if self.segments.is_empty() {
            return Err(ArrangementReject::EmptyArrangement);
        }

        let mut children = Vec::with_capacity(self.segments.len());
        for segment in &self.segments {
            let inner = lower_composer(&segment.composer, ctx)?;
            children.push(stretch_to_duration(inner, segment.duration)?);
        }
        Ok(PatternNodeIr::concat(children))
    }
}

fn lower_composer<F>(
    composer: &FlowComposer,
    ctx: &LowerCtx<'_, F>,
) -> Result<PatternNodeIr, ArrangementReject>
where
    F: Fn(&FlowRef) -> Result<PatternNodeIr, ArrangementReject>,
{
    match composer {
        FlowComposer::Ref(reference) => (ctx.resolve)(reference),
        FlowComposer::Parallel(list) | FlowComposer::Polymeter(list) => {
            let children = lower_flow_list(list, ctx)?;
            Ok(PatternNodeIr::merge(children))
        }
        FlowComposer::Layer { base, branches } => {
            let mut children = vec![(ctx.resolve)(base)?];
            for branch in branches {
                children.push((ctx.resolve)(branch)?);
            }
            Ok(PatternNodeIr::merge(children))
        }
    }
}

fn lower_flow_list<F>(
    list: &FlowList,
    ctx: &LowerCtx<'_, F>,
) -> Result<Vec<PatternNodeIr>, ArrangementReject>
where
    F: Fn(&FlowRef) -> Result<PatternNodeIr, ArrangementReject>,
{
    if list.items.is_empty() {
        return Err(ArrangementReject::EmptyFlowList);
    }
    list.items
        .iter()
        .map(|reference| (ctx.resolve)(reference))
        .collect()
}

fn stretch_to_duration(
    inner: PatternNodeIr,
    duration: Rational,
) -> Result<PatternNodeIr, ArrangementReject> {
    if duration <= Rational::zero() || duration.denominator != 1 {
        return Err(ArrangementReject::InvalidDuration(duration));
    }
    let cycles = duration.numerator as usize;
    match cycles {
        0 => Err(ArrangementReject::InvalidDuration(duration)),
        1 => Ok(inner),
        n => Ok(PatternNodeIr::concat(
            std::iter::repeat_n(inner, n).collect(),
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::PatternStream;

    fn note_pattern(label: &str) -> PatternNodeIr {
        use crate::domain::{CycleDuration, CycleSpan, CycleTime, EventValue, PatternEvent};
        PatternNodeIr::event_stream(PatternStream::new(vec![PatternEvent::new(
            CycleSpan {
                start: CycleTime(Rational::zero()),
                duration: CycleDuration(Rational::one()),
            },
            EventValue::Note {
                value: label.to_string(),
                octave: None,
            },
        )]))
    }

    fn resolve_ref(reference: &FlowRef) -> Result<PatternNodeIr, ArrangementReject> {
        match reference {
            FlowRef::Container { node } => Ok(note_pattern(node.0.as_str())),
            other => Err(ArrangementReject::UnknownFlowRef(other.clone())),
        }
    }

    #[test]
    fn arrangement_parallel_segments_concat_and_merge() {
        let arrangement = Arrangement {
            segments: vec![
                ArrangementSegment {
                    duration: Rational::from_integer(2),
                    composer: FlowComposer::Parallel(FlowList::new(vec![
                        FlowRef::Container {
                            node: NodeId::new("m1"),
                        },
                        FlowRef::Container {
                            node: NodeId::new("dr"),
                        },
                    ])),
                },
                ArrangementSegment {
                    duration: Rational::from_integer(8),
                    composer: FlowComposer::Parallel(FlowList::new(vec![
                        FlowRef::Container {
                            node: NodeId::new("m1"),
                        },
                        FlowRef::Container {
                            node: NodeId::new("dr"),
                        },
                        FlowRef::Container {
                            node: NodeId::new("chord"),
                        },
                    ])),
                },
            ],
        };

        let ctx = LowerCtx {
            resolve: &resolve_ref,
        };
        let lowered = arrangement.lower(&ctx).expect("arrangement lowers");
        match lowered {
            PatternNodeIr::Concat { children } => {
                assert_eq!(children.len(), 2);
                assert!(matches!(children[0], PatternNodeIr::Concat { .. }));
                assert!(matches!(children[1], PatternNodeIr::Concat { .. }));
                match &children[0] {
                    PatternNodeIr::Concat { children: inner } => {
                        assert_eq!(inner.len(), 2);
                        assert!(matches!(inner[0], PatternNodeIr::Merge { .. }));
                        assert!(matches!(inner[1], PatternNodeIr::Merge { .. }));
                    }
                    other => panic!("expected stretched parallel segment, got {other:?}"),
                }
                match &children[1] {
                    PatternNodeIr::Concat { children: inner } => {
                        assert_eq!(inner.len(), 8);
                        assert!(matches!(inner[0], PatternNodeIr::Merge { .. }));
                    }
                    other => panic!("expected stretched parallel segment, got {other:?}"),
                }
            }
            other => panic!("expected concat arrangement, got {other:?}"),
        }
    }
}
