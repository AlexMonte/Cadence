//! Immutable host pattern sources evaluated by the planner, never the audio callback.
use super::{
    control::{ControlKey, ControlModelError, ControlValue},
    moment::Moment,
    prelude::Time,
    span::TransportSpan,
};
use std::{fmt::Debug, sync::Arc};

/// A whole musical event and its ordered, constant control assignments.
/// Repeated keys use Cadence's normal merge rules. Hosts must provide stable
/// occurrence keys across overlapping queries, retaining the original whole span.
#[derive(Debug, Clone)]
pub struct QueryMoment {
    /// Original whole event, with optional authored provenance.
    pub moment: Moment,
    /// Stable occurrence discriminator within this source, independent of
    /// the optional authored provenance ID on the moment.
    pub instance_key: u64,
    /// Ordered assignments combined using each lane’s merge rule.
    pub controls: Vec<(ControlKey, ControlValue)>,
}
/// One absolute-time control segment returned by a host pattern.
#[derive(Debug, Clone)]
pub struct QueryControl {
    /// Whole interval over which this assignment applies.
    pub span: TransportSpan,
    /// Typed lane owned by the assignment.
    pub key: ControlKey,
    /// Validated lane value.
    pub value: ControlValue,
}
/// A deterministic, bounded query over an immutable musical pattern snapshot.
/// Return all whole events intersecting the requested window, including held
/// events. Query results must not depend on call order or earlier windows.
pub trait ScoreQuerySource: Debug + Send + Sync + std::panic::RefUnwindSafe {
    /// Return intersecting whole events for this absolute cycle window.
    fn query(&self, window: &TransportSpan) -> Result<Vec<QueryMoment>, ControlModelError>;
    /// Conservative work estimate, including nested patterns, before allocating.
    fn estimated_work(&self, window_cycles: f64) -> f64;
    /// Extent used by concatenation; repeating sources usually own one cycle.
    fn extent(&self) -> Time {
        Time::ONE
    }
}
/// The control counterpart of [`ScoreQuerySource`].
pub trait ControlQuerySource: Debug + Send + Sync + std::panic::RefUnwindSafe {
    /// Return intersecting whole control intervals for this cycle window.
    fn query(&self, window: &TransportSpan) -> Result<Vec<QueryControl>, ControlModelError>;
    /// Conservative planning work estimate, before any query allocation.
    fn estimated_work(&self, window_cycles: f64) -> f64;
    /// Extent used by concatenation; repeating sources usually own one cycle.
    fn extent(&self) -> Time {
        Time::ONE
    }
    /// Conservative by default so gaps correctly release owned runtime lanes.
    fn contains_key(&self, _key: &ControlKey) -> bool {
        true
    }
}
#[derive(Debug, Clone)]
/// Shared immutable musical query source; equality compares snapshot identity.
pub struct ScoreSource(pub Arc<dyn ScoreQuerySource>);
impl PartialEq for ScoreSource {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
}
#[derive(Debug, Clone)]
/// Shared immutable control query source; equality compares snapshot identity.
pub struct ControlSource(pub Arc<dyn ControlQuerySource>);
impl PartialEq for ControlSource {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
}
