//! Recursive source and control score trees.

use std::sync::{
    Arc,
    atomic::{AtomicU64, Ordering},
};

use crate::domain::{
    control::ControlTrack,
    mosaic::Mosaic,
    prelude::Time,
    space::{Axis, Point3},
    voice::Voice,
};

static SCORE_NODE_ID_GENERATOR: AtomicU64 = AtomicU64::new(1);
static CONTROL_SCORE_NODE_ID_GENERATOR: AtomicU64 = AtomicU64::new(1);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
/// Opaque identity for a score node.
pub struct ScoreNodeId(u64);

impl ScoreNodeId {
    fn next() -> Self {
        Self(SCORE_NODE_ID_GENERATOR.fetch_add(1, Ordering::Relaxed))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
/// Opaque identity for a control-score node.
pub struct ControlScoreNodeId(u64);

impl ControlScoreNodeId {
    fn next() -> Self {
        Self(CONTROL_SCORE_NODE_ID_GENERATOR.fetch_add(1, Ordering::Relaxed))
    }
}

#[derive(Debug, Clone)]
/// Recursive source tree describing what musical material should exist.
///
/// `Score` is the canonical source-level authoring/query shape exposed by the
/// infrastructure layer.
pub struct Score(Arc<ScoreNode>);

#[derive(Debug, Clone, PartialEq)]
struct ScoreNode {
    id: ScoreNodeId,
    kind: ScoreKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
/// Deterministic keep/drop policy for projected source events.
pub struct DegradePolicy {
    keep_probability: Time,
    seed: u64,
}

impl DegradePolicy {
    /// Creates a deterministic degrade policy.
    ///
    /// # Panics
    ///
    /// Panics if `keep_probability` is outside `[0, 1]`.
    #[must_use]
    pub fn new(keep_probability: Time, seed: u64) -> Self {
        assert!(
            keep_probability >= Time::ZERO && keep_probability <= Time::ONE,
            "degrade keep probability must be within [0, 1]"
        );

        Self {
            keep_probability,
            seed,
        }
    }

    /// Returns the event keep probability.
    #[must_use]
    pub fn keep_probability(self) -> Time {
        self.keep_probability
    }

    /// Returns the deterministic seed.
    #[must_use]
    pub fn seed(self) -> u64 {
        self.seed
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
/// Duplicate identity policy for projected lifecycle cleanup.
pub enum DeduplicateKey {
    /// Lifecycles are duplicates only if their lifecycle identity matches.
    Lifecycle,
    /// Lifecycles are duplicates when they occupy the same whole span and carry
    /// the same source intent.
    WholeSpanAndIntent,
    /// Lifecycles are duplicates when they start at the same transport time and
    /// carry the same source intent.
    StartAndIntent,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
/// Winner policy used when duplicate lifecycles are found.
pub enum DeduplicateWinner {
    /// Keep the first lifecycle after stable projection ordering.
    First,
    /// Keep the last lifecycle after stable projection ordering.
    Last,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
/// Deterministic duplicate-removal policy.
pub struct DeduplicatePolicy {
    key: DeduplicateKey,
    winner: DeduplicateWinner,
}

impl DeduplicatePolicy {
    /// Creates a duplicate-removal policy.
    #[must_use]
    pub fn new(key: DeduplicateKey, winner: DeduplicateWinner) -> Self {
        Self { key, winner }
    }

    /// Keeps the first lifecycle with the same whole span and intent.
    #[must_use]
    pub fn whole_span_and_intent() -> Self {
        Self::new(DeduplicateKey::WholeSpanAndIntent, DeduplicateWinner::First)
    }

    /// Returns the duplicate key policy.
    #[must_use]
    pub fn key(self) -> DeduplicateKey {
        self.key
    }

    /// Returns the duplicate winner policy.
    #[must_use]
    pub fn winner(self) -> DeduplicateWinner {
        self.winner
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
/// Conflict rule used by priority merge.
pub enum ConflictPolicy {
    /// Lower-priority lifecycles conflict when they begin at the same time and
    /// carry the same source intent.
    SameWholeStartAndIntent,
    /// Lower-priority lifecycles conflict when their whole spans match exactly
    /// and they carry the same source intent.
    SameWholeSpanAndIntent,
    /// Lower-priority lifecycles conflict when their whole spans overlap.
    WholeSpanOverlap,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
/// Merge policy where earlier children win conflicts against later children.
pub struct PriorityMergePolicy {
    conflict: ConflictPolicy,
}

impl PriorityMergePolicy {
    /// Creates a priority-merge policy.
    #[must_use]
    pub fn new(conflict: ConflictPolicy) -> Self {
        Self { conflict }
    }

    /// Returns the conflict policy.
    #[must_use]
    pub fn conflict(self) -> ConflictPolicy {
        self.conflict
    }
}

#[derive(Debug, Clone, PartialEq)]
/// One weighted score option used by cycle-stable weighted choice.
pub struct WeightedScore {
    score: Score,
    weight: Time,
}

impl WeightedScore {
    /// Creates a weighted score option.
    ///
    /// # Panics
    ///
    /// Panics if `weight <= 0`.
    #[must_use]
    pub fn new(score: Score, weight: Time) -> Self {
        assert!(
            weight > Time::ZERO,
            "weighted score weight must be positive"
        );
        Self { score, weight }
    }

    /// Returns the child score.
    #[must_use]
    pub fn score(&self) -> &Score {
        &self.score
    }

    /// Returns the positive weight.
    #[must_use]
    pub fn weight(&self) -> Time {
        self.weight
    }
}

#[derive(Debug, Clone, PartialEq)]
/// One weighted control-score option used by cycle-stable weighted choice.
pub struct WeightedControlScore {
    score: ControlScore,
    weight: Time,
}

impl WeightedControlScore {
    /// Creates a weighted control-score option.
    ///
    /// # Panics
    ///
    /// Panics if `weight <= 0`.
    #[must_use]
    pub fn new(score: ControlScore, weight: Time) -> Self {
        assert!(
            weight > Time::ZERO,
            "weighted control score weight must be positive"
        );
        Self { score, weight }
    }

    /// Returns the child control score.
    #[must_use]
    pub fn score(&self) -> &ControlScore {
        &self.score
    }

    /// Returns the positive weight.
    #[must_use]
    pub fn weight(&self) -> Time {
        self.weight
    }
}

#[derive(Debug, Clone, PartialEq)]
/// Public variants of the source score tree.
pub enum ScoreKind {
    /// Single repeating voice leaf.
    Voice(Voice),
    /// Already-projected transport-time material.
    Mosaic(Mosaic),
    /// Simultaneous children.
    Merge(Vec<Score>),
    /// Append children in transport-time order. Child *n+1* starts after child *n* ends.
    Concat(Vec<Score>),
    /// Route cycles across children over time.
    CycleRoute(Vec<Score>),
    /// Assign specific cycle slots to children.
    CycleSlots(Vec<Score>),
    /// Scale child time by `rate`.
    TimeScale {
        /// Child score being transformed.
        inner: Score,
        /// Positive time scale rate.
        rate: Time,
    },
    /// Shift child material in transport time.
    Shift {
        /// Child score being transformed.
        inner: Score,
        /// Exact transport-time offset.
        offset: Time,
    },
    /// Reflect child material inside one cycle.
    ReflectCycle {
        /// Child score being transformed.
        inner: Score,
    },
    /// Translate the spatial position of child material by a fixed offset.
    ///
    /// This is the spatial twin of [`ScoreKind::Shift`]: it moves material
    /// through space, not time, so it leaves spans (and the time-based
    /// scheduler) untouched and only rewrites each projected moment's position.
    SpaceShift {
        /// Child score being transformed.
        inner: Score,
        /// Exact lattice offset.
        offset: Point3,
    },
    /// Scale the spatial position of child material component-wise.
    SpaceScale {
        /// Child score being transformed.
        inner: Score,
        /// Per-axis scale factor.
        factor: Point3,
    },
    /// Reflect the spatial position of child material across one axis.
    SpaceReflect {
        /// Child score being transformed.
        inner: Score,
        /// Axis the position is mirrored across.
        axis: Axis,
    },
    /// Deterministically keeps or drops whole projected event lifecycles.
    ///
    /// Degrade is an event-existence transform. Its keep/drop decision is
    /// based on whole-event lifecycle identity, not query-local visible
    /// fragments.
    Degrade {
        /// Child score being filtered.
        inner: Score,
        /// Deterministic keep/drop policy.
        policy: DegradePolicy,
    },
    /// Removes duplicate whole projected lifecycles according to a stable key.
    Deduplicate {
        /// Child score being cleaned.
        inner: Score,
        /// Duplicate-removal policy.
        policy: DeduplicatePolicy,
    },
    /// Merges children in priority order.
    ///
    /// Earlier children win conflicts against later children.
    PriorityMerge {
        /// Priority-ordered children.
        children: Vec<Score>,
        /// Conflict policy.
        policy: PriorityMergePolicy,
    },
    /// Selects one child per absolute cycle using deterministic weighted choice.
    WeightedChoice {
        /// Weighted child options.
        options: Vec<WeightedScore>,
        /// Deterministic seed.
        seed: u64,
    },
    /// Clips source event visibility to open gate-control spans.
    ///
    /// The source event keeps its original whole-event identity. Only the
    /// visible span is clipped.
    MaskClip {
        /// Source score being clipped.
        source: Score,
        /// Control score whose `ControlKey::Gate == true` spans define open
        /// visibility regions.
        mask: ControlScore,
    },
    /// Applies a control score to a source score.
    WithControls {
        /// Source material receiving the controls.
        source: Score,
        /// Control tree projected alongside the source.
        controls: ControlScore,
    },
}

#[derive(Debug, Clone)]
/// Recursive control tree parallel to [`Score`].
pub struct ControlScore(Arc<ControlScoreNode>);

#[derive(Debug, Clone, PartialEq)]
struct ControlScoreNode {
    id: ControlScoreNodeId,
    kind: ControlScoreKind,
}

#[derive(Debug, Clone, PartialEq)]
/// Public variants of the control-score tree.
pub enum ControlScoreKind {
    /// Single repeating control track leaf.
    Track(ControlTrack),
    /// Simultaneous control children.
    Merge(Vec<ControlScore>),
    /// Append control children in transport-time order.
    Concat(Vec<ControlScore>),
    /// Route cycles across control children over time.
    CycleRoute(Vec<ControlScore>),
    /// Assign specific cycle slots to control children.
    CycleSlots(Vec<ControlScore>),
    /// Scale child control time by `rate`.
    TimeScale {
        /// Child control score being transformed.
        inner: ControlScore,
        /// Positive time scale rate.
        rate: Time,
    },
    /// Shift child controls in transport time.
    Shift {
        /// Child control score being transformed.
        inner: ControlScore,
        /// Exact transport-time offset.
        offset: Time,
    },
    /// Reflect child controls inside one cycle.
    ReflectCycle {
        /// Child control score being transformed.
        inner: ControlScore,
    },
    /// Merges control children in priority order.
    ///
    /// Earlier children win conflicts against later children. Conflict policies
    /// that mention "intent" treat [`crate::domain::control::ControlKey`] as the
    /// control-lane identity.
    PriorityMerge {
        /// Priority-ordered control children (earlier wins).
        children: Vec<ControlScore>,
        /// Conflict rule used when later children collide with earlier ones.
        policy: PriorityMergePolicy,
    },
    /// Selects one control child per absolute cycle using deterministic weighted
    /// choice.
    WeightedChoice {
        /// Weighted control child options.
        options: Vec<WeightedControlScore>,
        /// Deterministic seed.
        seed: u64,
    },
    /// Clips source control visibility to open gate-control spans.
    ///
    /// The source control keeps its original whole-span identity. Only the
    /// visible span is clipped. The mask itself is not emitted.
    MaskClip {
        /// Source control score being clipped.
        source: ControlScore,
        /// Control score whose `ControlKey::Gate == true` spans define open
        /// visibility regions.
        mask: ControlScore,
    },
}

impl Score {
    /// Returns an empty score.
    #[must_use]
    pub fn empty() -> Self {
        Self::merge(Vec::new())
    }

    /// Wraps one voice as a score leaf.
    #[must_use]
    pub fn voice(voice: Voice) -> Self {
        Self::new(ScoreKind::Voice(voice))
    }

    /// Wraps already-projected material as a score leaf.
    #[must_use]
    pub fn mosaic(mosaic: Mosaic) -> Self {
        Self::new(ScoreKind::Mosaic(mosaic))
    }

    /// Creates a simultaneous merge of child scores.
    #[must_use]
    pub fn merge(children: Vec<Score>) -> Self {
        Self::new(ScoreKind::Merge(children))
    }

    /// Appends child scores sequentially in transport-time order.
    #[must_use]
    pub fn concat(children: Vec<Score>) -> Self {
        match children.len() {
            0 => Self::empty(),
            1 => children.into_iter().next().expect("checked length"),
            _ => Self::new(ScoreKind::Concat(children)),
        }
    }

    /// Creates a cycle-routing score.
    #[must_use]
    pub fn cycle_route(children: Vec<Score>) -> Self {
        Self::new(ScoreKind::CycleRoute(children))
    }

    /// Creates a cycle-slot score.
    #[must_use]
    pub fn cycle_slots(children: Vec<Score>) -> Self {
        Self::new(ScoreKind::CycleSlots(children))
    }

    /// Scales a child score in time.
    ///
    /// # Panics
    ///
    /// Panics if `rate <= 0`.
    #[must_use]
    pub fn time_scale(inner: Score, rate: Time) -> Self {
        assert!(rate > Time::ZERO, "time scale rate must be positive");
        Self::new(ScoreKind::TimeScale { inner, rate })
    }

    /// Shifts a child score in transport time.
    #[must_use]
    pub fn shift(inner: Score, offset: Time) -> Self {
        Self::new(ScoreKind::Shift { inner, offset })
    }

    /// Reflects a child score inside one cycle.
    #[must_use]
    pub fn reflect_cycle(inner: Score) -> Self {
        Self::new(ScoreKind::ReflectCycle { inner })
    }

    /// Translates the spatial position of a child score by `offset`.
    #[must_use]
    pub fn space_shift(inner: Score, offset: Point3) -> Self {
        Self::new(ScoreKind::SpaceShift { inner, offset })
    }

    /// Scales the spatial position of a child score component-wise by `factor`.
    #[must_use]
    pub fn space_scale(inner: Score, factor: Point3) -> Self {
        Self::new(ScoreKind::SpaceScale { inner, factor })
    }

    /// Reflects the spatial position of a child score across `axis`.
    #[must_use]
    pub fn space_reflect(inner: Score, axis: Axis) -> Self {
        Self::new(ScoreKind::SpaceReflect { inner, axis })
    }

    /// Deterministically keeps or drops whole projected event lifecycles.
    ///
    /// This is not a control transform. Use [`Score::with_controls`] for gain,
    /// attack, pan, playback-rate, pitch, filters, selectors, and other
    /// parameter modulation.
    ///
    /// `degrade` is stable across query slicing because projection keys are
    /// based on whole-event lifecycle identity.
    #[must_use]
    pub fn degrade(inner: Score, policy: DegradePolicy) -> Self {
        Self::new(ScoreKind::Degrade { inner, policy })
    }

    /// Removes duplicate whole projected lifecycles according to a stable key.
    #[must_use]
    pub fn deduplicate(inner: Score, policy: DeduplicatePolicy) -> Self {
        Self::new(ScoreKind::Deduplicate { inner, policy })
    }

    /// Merges children in priority order.
    ///
    /// Earlier children win conflicts against later children.
    #[must_use]
    pub fn priority_merge(children: Vec<Score>, policy: PriorityMergePolicy) -> Self {
        Self::new(ScoreKind::PriorityMerge { children, policy })
    }

    /// Selects one child per absolute cycle using deterministic weighted
    /// choice.
    ///
    /// Empty option lists produce no events.
    #[must_use]
    pub fn weighted_choice(options: Vec<WeightedScore>, seed: u64) -> Self {
        Self::new(ScoreKind::WeightedChoice { options, seed })
    }

    /// Selects one child per absolute cycle using deterministic equal-weight
    /// choice.
    ///
    /// Empty child lists produce no events.
    #[must_use]
    pub fn seeded_choice(children: Vec<Score>, seed: u64) -> Self {
        Self::weighted_choice(
            children
                .into_iter()
                .map(|score| WeightedScore::new(score, Time::ONE))
                .collect(),
            seed,
        )
    }

    /// Clips source event visibility to open gate-control spans.
    ///
    /// This is not the same as [`Score::with_controls`]. `with_controls`
    /// decorates events with controls. `mask_clip` changes visible event spans.
    #[must_use]
    pub fn mask_clip(source: Score, mask: ControlScore) -> Self {
        Self::new(ScoreKind::MaskClip { source, mask })
    }

    /// Applies a control tree to a source tree.
    #[must_use]
    pub fn with_controls(source: Score, controls: ControlScore) -> Self {
        Self::new(ScoreKind::WithControls { source, controls })
    }

    #[must_use]
    fn new(kind: ScoreKind) -> Self {
        Self(Arc::new(ScoreNode {
            id: ScoreNodeId::next(),
            kind,
        }))
    }

    #[must_use]
    pub(crate) fn id(&self) -> ScoreNodeId {
        self.0.id
    }

    #[must_use]
    pub(crate) fn kind(&self) -> &ScoreKind {
        &self.0.kind
    }
}

impl PartialEq for Score {
    fn eq(&self, other: &Self) -> bool {
        self.kind() == other.kind()
    }
}

impl From<Voice> for Score {
    fn from(value: Voice) -> Self {
        Self::voice(value)
    }
}

impl From<Mosaic> for Score {
    fn from(value: Mosaic) -> Self {
        Self::mosaic(value)
    }
}

impl ControlScore {
    /// Wraps one control track as a control-score leaf.
    #[must_use]
    pub fn track(track: ControlTrack) -> Self {
        Self::new(ControlScoreKind::Track(track))
    }

    /// Creates a simultaneous merge of control children.
    #[must_use]
    pub fn merge(children: Vec<ControlScore>) -> Self {
        Self::new(ControlScoreKind::Merge(children))
    }

    /// Appends control children sequentially in transport-time order.
    #[must_use]
    pub fn concat(children: Vec<ControlScore>) -> Self {
        match children.len() {
            0 => Self::merge(Vec::new()),
            1 => children.into_iter().next().expect("checked length"),
            _ => Self::new(ControlScoreKind::Concat(children)),
        }
    }

    /// Creates a cycle-routing control tree.
    #[must_use]
    pub fn cycle_route(children: Vec<ControlScore>) -> Self {
        Self::new(ControlScoreKind::CycleRoute(children))
    }

    /// Creates a cycle-slot control tree.
    #[must_use]
    pub fn cycle_slots(children: Vec<ControlScore>) -> Self {
        Self::new(ControlScoreKind::CycleSlots(children))
    }

    /// Scales a child control tree in time.
    ///
    /// # Panics
    ///
    /// Panics if `rate <= 0`.
    #[must_use]
    pub fn time_scale(inner: ControlScore, rate: Time) -> Self {
        assert!(rate > Time::ZERO, "time scale rate must be positive");
        Self::new(ControlScoreKind::TimeScale { inner, rate })
    }

    /// Shifts a child control tree in transport time.
    #[must_use]
    pub fn shift(inner: ControlScore, offset: Time) -> Self {
        Self::new(ControlScoreKind::Shift { inner, offset })
    }

    /// Reflects a child control tree inside one cycle.
    #[must_use]
    pub fn reflect_cycle(inner: ControlScore) -> Self {
        Self::new(ControlScoreKind::ReflectCycle { inner })
    }

    /// Merges control children in priority order.
    ///
    /// Earlier children win conflicts against later children. Conflict policies
    /// that mention "intent" treat control key as the lane identity.
    #[must_use]
    pub fn priority_merge(children: Vec<ControlScore>, policy: PriorityMergePolicy) -> Self {
        Self::new(ControlScoreKind::PriorityMerge { children, policy })
    }

    /// Selects one control child per absolute cycle using deterministic weighted
    /// choice.
    ///
    /// Empty option lists produce no controls.
    #[must_use]
    pub fn weighted_choice(options: Vec<WeightedControlScore>, seed: u64) -> Self {
        Self::new(ControlScoreKind::WeightedChoice { options, seed })
    }

    /// Clips source control visibility to open gate-control spans.
    ///
    /// This is the control-tree dual of [`Score::mask_clip`]: whole-span identity
    /// is preserved and only visible spans are clipped to open gate regions.
    #[must_use]
    pub fn mask_clip(source: ControlScore, mask: ControlScore) -> Self {
        Self::new(ControlScoreKind::MaskClip { source, mask })
    }

    /// Returns an empty control score (no tiles).
    #[must_use]
    pub fn empty() -> Self {
        Self::merge(Vec::new())
    }

    #[must_use]
    fn new(kind: ControlScoreKind) -> Self {
        Self(Arc::new(ControlScoreNode {
            id: ControlScoreNodeId::next(),
            kind,
        }))
    }

    #[must_use]
    pub(crate) fn id(&self) -> ControlScoreNodeId {
        self.0.id
    }

    #[must_use]
    pub(crate) fn kind(&self) -> &ControlScoreKind {
        &self.0.kind
    }
}

impl PartialEq for ControlScore {
    fn eq(&self, other: &Self) -> bool {
        self.kind() == other.kind()
    }
}

impl From<ControlTrack> for ControlScore {
    fn from(value: ControlTrack) -> Self {
        Self::track(value)
    }
}
