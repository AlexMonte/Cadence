//! Strudel-style flow arrangements: timed segments of composed flow refs.
//!
//! Strudel's `stack(...)` maps to [`FlowComposer::Parallel`], not vertical atom stacking.

use serde::{Deserialize, Serialize};

use super::pattern_ir::{PatternNodeIr, Rational, TimedPatternIr};
use super::program::NodeId;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum FlowRef {
    Container { node: NodeId },
    Transform { node: NodeId },
    Arrangement { node: NodeId },
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct FlowList {
    pub items: Vec<FlowRef>,
}

impl FlowList {
    pub fn new(items: Vec<FlowRef>) -> Self {
        Self { items }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "value", rename_all = "snake_case")]
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
pub struct ArrangementSegment {
    pub duration: Rational,
    pub composer: FlowComposer,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct Arrangement {
    pub segments: Vec<ArrangementSegment>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ArrangementReject {
    EmptyArrangement,
    EmptyFlowList,
    InvalidDuration(Rational),
    InvalidTotalDuration,
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
            let timed = TimedPatternIr::new(segment.duration, 1, inner);
            timed
                .validate()
                .map_err(|_| ArrangementReject::InvalidDuration(segment.duration))?;
            children.push(timed);
        }
        TimedPatternIr::total_duration(&children)
            .map_err(|_| ArrangementReject::InvalidTotalDuration)?;
        Ok(PatternNodeIr::arrange(children))
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::PatternStream;

    #[test]
    fn every_composer_round_trips_without_colliding_reference_tags() {
        let refs = [
            FlowRef::Container {
                node: NodeId::new("notes"),
            },
            FlowRef::Transform {
                node: NodeId::new("gain"),
            },
            FlowRef::Arrangement {
                node: NodeId::new("song"),
            },
        ];
        let mut composers: Vec<_> = refs.iter().cloned().map(FlowComposer::Ref).collect();
        composers.extend([
            FlowComposer::Parallel(FlowList::new(refs.to_vec())),
            FlowComposer::Polymeter(FlowList::new(refs.to_vec())),
            FlowComposer::Layer {
                base: refs[0].clone(),
                branches: refs[1..].to_vec(),
            },
        ]);
        for composer in composers {
            let json = serde_json::to_string(&composer).unwrap();
            assert_eq!(
                serde_json::from_str::<FlowComposer>(&json).unwrap(),
                composer
            );
            assert_eq!(
                serde_json::from_value::<FlowComposer>(serde_json::to_value(&composer).unwrap())
                    .unwrap(),
                composer
            );
        }
    }

    fn note_pattern(label: &str) -> PatternNodeIr {
        use crate::domain::{CycleDuration, CycleSpan, CycleTime, EventValue, PatternEvent};
        PatternNodeIr::cycle_event_stream(PatternStream::new(vec![PatternEvent::new(
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
        assert_eq!(lowered.duration().0, Rational::from_integer(10));
        for cycle in 0..20 {
            let stream = lowered.query(crate::domain::CycleSpan::new(
                crate::domain::CycleTime(Rational::from_integer(cycle)),
                crate::domain::CycleDuration(Rational::one()),
            ));
            let labels = stream
                .events
                .iter()
                .filter_map(|event| match &event.value {
                    crate::domain::EventValue::Note { value, .. }
                    | crate::domain::EventValue::Sound { value } => Some(value.as_str()),
                    _ => None,
                })
                .collect::<Vec<_>>();
            assert!(labels.contains(&"m1") && labels.contains(&"dr"));
            assert_eq!(labels.contains(&"chord"), cycle % 10 >= 2);
            assert_eq!(labels.len(), if cycle % 10 < 2 { 2 } else { 3 });
        }
    }
}
