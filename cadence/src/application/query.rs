use std::hash::{Hash, Hasher};

use crate::application::audio::VoiceInstanceId;
use crate::domain::{
    arrangement::{Timed, TimedControlScore, TimedScore},
    control::{
        ControlKey, ControlMap, ControlMerge, ControlModelError, ControlTile, ControlTiming,
        ControlTrack, ControlValue, SignedUnitValue, UnitValue,
    },
    intent::Intent,
    moment::Moment,
    prelude::Time,
    projection::ProjectedMoment,
    score::{
        ConflictPolicy, ControlScore, ControlScoreKind, ControlScoreNodeId, DeduplicateKey,
        DeduplicatePolicy, DeduplicateWinner, DegradePolicy, PriorityMergePolicy, Score, ScoreKind,
        ScoreNodeId, WeightedControlScore, WeightedScore,
    },
    space::SpatialMotion,
    span::TransportSpan,
    voice::{Repeat, Voice},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) struct LeafEventId(u64);

impl LeafEventId {
    #[must_use]
    pub(crate) fn new(value: u64) -> Self {
        Self(value)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) struct QueryKey {
    owner: ScoreNodeId,
    leaf_event: LeafEventId,
    whole: TransportSpan,
}

impl QueryKey {
    #[must_use]
    pub(crate) fn new(owner: ScoreNodeId, leaf_event: LeafEventId, whole: TransportSpan) -> Self {
        Self {
            owner,
            leaf_event,
            whole,
        }
    }

    #[must_use]
    pub(crate) fn whole(self) -> TransportSpan {
        self.whole
    }

    #[must_use]
    fn with_owner(self, owner: ScoreNodeId, whole: TransportSpan) -> Self {
        Self {
            owner,
            leaf_event: self.leaf_event,
            whole,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
struct LifecycleKey {
    voice_id: VoiceInstanceId,
    voice_whole: TransportSpan,
}

/// How one evaluated moment participates in voice scheduling.
#[derive(Debug, Clone, PartialEq)]
pub enum EvaluatedEventKind {
    /// Begins or backfills a voice instance for one lifecycle span.
    StartVoice {
        /// Full transport span of the underlying note or sample lifecycle.
        voice_whole: TransportSpan,
        /// Stable identifier shared by later updates for the same lifecycle.
        voice_id: VoiceInstanceId,
    },
    /// Applies runtime control changes to an existing voice instance.
    UpdateVoiceControls {
        /// Lifecycle span the update targets.
        voice_whole: TransportSpan,
        /// Stable identifier matching the lifecycle's [`StartVoice`].
        voice_id: VoiceInstanceId,
    },
}

/// One evaluated scheduling moment with lifecycle metadata.
#[derive(Debug, Clone, PartialEq)]
pub struct EvaluatedEvent {
    key: QueryKey,
    projected: ProjectedMoment,
    kind: EvaluatedEventKind,
}

impl EvaluatedEvent {
    #[must_use]
    pub(crate) fn new(key: QueryKey, projected: ProjectedMoment) -> Self {
        let voice_whole = projected.whole();
        let voice_id = voice_instance_id(key, voice_whole);

        Self {
            key,
            projected,
            kind: EvaluatedEventKind::StartVoice {
                voice_whole,
                voice_id,
            },
        }
    }

    #[must_use]
    pub(crate) fn new_with_kind(
        key: QueryKey,
        projected: ProjectedMoment,
        kind: EvaluatedEventKind,
    ) -> Self {
        Self {
            key,
            projected,
            kind,
        }
    }

    #[must_use]
    pub(crate) fn key(&self) -> QueryKey {
        self.key
    }

    /// Returns the projected moment for this evaluation row.
    #[must_use]
    pub fn projected(&self) -> &ProjectedMoment {
        &self.projected
    }

    /// Consumes the row and returns the projected moment.
    #[must_use]
    pub fn into_projected(self) -> ProjectedMoment {
        self.projected
    }

    /// Returns whether this row starts a voice or applies a runtime update.
    #[must_use]
    pub fn kind(&self) -> &EvaluatedEventKind {
        &self.kind
    }

    #[must_use]
    fn with_projected(self, owner: ScoreNodeId, projected: ProjectedMoment) -> Self {
        let key = self.key.with_owner(owner, projected.whole());
        let voice_whole = projected.whole();
        let voice_id = voice_instance_id(key, voice_whole);
        let kind = match self.kind {
            EvaluatedEventKind::StartVoice { .. } => EvaluatedEventKind::StartVoice {
                voice_whole,
                voice_id,
            },
            // A time or ownership transform must carry updates to the same
            // lifecycle as its start, including when only an update is visible.
            EvaluatedEventKind::UpdateVoiceControls { .. } => {
                EvaluatedEventKind::UpdateVoiceControls {
                    voice_whole,
                    voice_id,
                }
            }
        };

        Self {
            key,
            projected,
            kind,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
struct LifecycleGroup {
    key: LifecycleKey,
    events: Vec<EvaluatedEvent>,
}

impl LifecycleGroup {
    fn representative(&self) -> &EvaluatedEvent {
        self.events
            .iter()
            .find(|event| matches!(event.kind(), EvaluatedEventKind::StartVoice { .. }))
            .unwrap_or_else(|| {
                self.events
                    .first()
                    .expect("lifecycle group must contain at least one event")
            })
    }

    fn whole(&self) -> TransportSpan {
        self.key.voice_whole
    }

    fn intent(&self) -> &Intent {
        self.representative().projected().intent()
    }
}

#[derive(Debug, Clone, PartialEq)]
struct EvaluatedControl {
    owner: ControlScoreNodeId,
    whole: TransportSpan,
    visible: TransportSpan,
    key: ControlKey,
    value: ControlValue,
}

impl EvaluatedControl {
    #[must_use]
    fn new(
        owner: ControlScoreNodeId,
        whole: TransportSpan,
        visible: TransportSpan,
        key: ControlKey,
        value: ControlValue,
    ) -> Self {
        Self {
            owner,
            whole,
            visible,
            key,
            value,
        }
    }

    #[must_use]
    fn remap(
        &self,
        owner: ControlScoreNodeId,
        whole: TransportSpan,
        visible: TransportSpan,
        value: ControlValue,
    ) -> Self {
        Self {
            owner,
            whole,
            visible,
            key: self.key.clone(),
            value,
        }
    }
}

pub(crate) fn evaluate_score(
    score: &Score,
    window: &TransportSpan,
) -> Result<Vec<EvaluatedEvent>, ControlModelError> {
    let mut events = evaluate_score_unsorted(score, window)?;
    sort_evaluated_events(&mut events);
    Ok(events)
}

fn evaluate_score_unsorted(
    score: &Score,
    window: &TransportSpan,
) -> Result<Vec<EvaluatedEvent>, ControlModelError> {
    match score.kind() {
        ScoreKind::QuerySource(source) => {
            let work = source
                .0
                .estimated_work((window.end() - window.start()).value());
            if !work.is_finite() || work < 0.0 || work > 16_384.0 {
                return Err(ControlModelError::QuerySource(
                    "query exceeds its preparation budget".into(),
                ));
            }
            let moments = source.0.query(window)?;
            if moments.len() > 16_384 {
                return Err(ControlModelError::QuerySource(
                    "too many events in one window".into(),
                ));
            }
            let mut result = Vec::new();
            let mut seen = std::collections::BTreeSet::new();
            for item in moments {
                let Some(visible) = item.moment.span().intersection(window) else {
                    continue;
                };
                let whole = item.moment.span();
                if !seen.insert((item.instance_key, whole)) {
                    return Err(ControlModelError::QuerySource(
                        "duplicate event identity in one window".into(),
                    ));
                }
                let mut controls = ControlMap::new();
                for (key, value) in item.controls {
                    value.validate_for(&key)?;
                    validate_control_support_for_intent(&key, item.moment.intent())?;
                    let value = sample_onset_control(&key, &value, whole.start());
                    merge_into_control_map(&mut controls, key, value)?;
                }
                result.push(EvaluatedEvent::new(
                    QueryKey::new(score.id(), LeafEventId::new(item.instance_key), whole),
                    ProjectedMoment::new(item.moment, visible, controls),
                ));
            }
            Ok(result)
        }
        ScoreKind::Voice(voice) => Ok(evaluate_voice_leaf(score.id(), voice, window)),
        ScoreKind::Events(events) => Ok(evaluate_events_leaf(score.id(), events, window)),
        ScoreKind::Merge(children) => {
            let mut events = Vec::new();
            for child in children {
                events.extend(evaluate_score(child, window)?);
            }
            Ok(events)
        }
        ScoreKind::Concat(children) => evaluate_score_concat(score.id(), children, window),
        ScoreKind::Arrange { segments, period } => {
            evaluate_score_arrangement(score.id(), segments, *period, window)
        }
        ScoreKind::CycleRoute(children) => evaluate_score_cycle_route(children, window),
        ScoreKind::CycleSlots(children) => evaluate_score_cycle_slots(children, window),
        ScoreKind::WeightedCycleSlots(children) => evaluate_score_weighted_slots(
            &children
                .iter()
                .map(|child| (child.score(), child.weight()))
                .collect::<Vec<_>>(),
            window,
        ),
        ScoreKind::TimeScale { inner, rate } => {
            let scaled_window =
                TransportSpan::new(window.start() * *rate, window.end() * *rate).unwrap();

            Ok(evaluate_score(inner, &scaled_window)?
                .into_iter()
                .map(|event| {
                    let projected = remap_projected(
                        event.projected(),
                        scale_span(event.projected().whole(), Time::ONE / *rate),
                        scale_span(event.projected().visible(), Time::ONE / *rate),
                        event.projected().controls().clone(),
                    );

                    event.with_projected(score.id(), projected)
                })
                .collect())
        }
        ScoreKind::Shift { inner, offset } => {
            let shifted_window =
                TransportSpan::new(window.start() - *offset, window.end() - *offset).unwrap();

            Ok(evaluate_score(inner, &shifted_window)?
                .into_iter()
                .map(|event| {
                    let projected = remap_projected(
                        event.projected(),
                        translate_span(event.projected().whole(), *offset),
                        translate_span(event.projected().visible(), *offset),
                        event.projected().controls().clone(),
                    );

                    event.with_projected(score.id(), projected)
                })
                .collect())
        }
        ScoreKind::ReflectCycle { inner } => evaluate_reflected_score(score.id(), inner, window),
        ScoreKind::SpaceShift { inner, offset } => {
            let offset = *offset;
            Ok(map_event_positions(
                evaluate_score(inner, window)?,
                |motion| motion.translated(offset),
            ))
        }
        ScoreKind::SpaceScale { inner, factor } => {
            let factor = *factor;
            Ok(map_event_positions(
                evaluate_score(inner, window)?,
                |motion| motion.scaled(factor),
            ))
        }
        ScoreKind::SpaceReflect { inner, axis } => {
            let axis = *axis;
            Ok(map_event_positions(
                evaluate_score(inner, window)?,
                |motion| motion.reflected(axis),
            ))
        }
        ScoreKind::Degrade { inner, policy } => Ok(filter_degraded_events(
            evaluate_score(inner, window)?,
            *policy,
        )),
        ScoreKind::Deduplicate { inner, policy } => {
            Ok(deduplicate_events(evaluate_score(inner, window)?, *policy))
        }
        ScoreKind::PriorityMerge { children, policy } => {
            evaluate_priority_merge(children, window, *policy)
        }
        ScoreKind::WeightedChoice { options, seed } => {
            evaluate_weighted_choice(options, *seed, window)
        }
        ScoreKind::MaskClip { source, mask } => Ok(mask_clip_events(
            score.id(),
            evaluate_score(source, window)?,
            evaluate_control_score(mask, window)?,
        )),
        ScoreKind::WithControls { source, controls } => {
            apply_controls(score.id(), source, controls, window)
        }
    }
}

fn group_lifecycles(events: Vec<EvaluatedEvent>) -> Vec<LifecycleGroup> {
    let mut groups = Vec::<LifecycleGroup>::new();
    let mut group_index = std::collections::BTreeMap::<LifecycleKey, usize>::new();

    for event in events {
        let key = lifecycle_key(&event);
        let index = *group_index.entry(key).or_insert_with(|| {
            let index = groups.len();
            groups.push(LifecycleGroup {
                key,
                events: Vec::new(),
            });
            index
        });
        // An interior query can contain updates only. Both row kinds carry
        // complete lifecycle identity, and only the scheduler may promote an
        // unseen update to a backfill start. Keep projection rows unchanged.
        groups[index].events.push(event);
    }

    groups
}

fn flatten_lifecycle_groups(groups: Vec<LifecycleGroup>) -> Vec<EvaluatedEvent> {
    groups.into_iter().flat_map(|group| group.events).collect()
}

fn filter_degraded_events(
    events: Vec<EvaluatedEvent>,
    policy: DegradePolicy,
) -> Vec<EvaluatedEvent> {
    let keep_probability = policy.keep_probability();
    if keep_probability <= Time::ZERO {
        return Vec::new();
    }
    if keep_probability >= Time::ONE {
        return events;
    }

    let groups = group_lifecycles(events);
    flatten_lifecycle_groups(
        groups
            .into_iter()
            .filter(|group| {
                if let Some(value) = group
                    .representative()
                    .projected()
                    .as_moment()
                    .value_identity()
                {
                    let mut hasher = StableHasher::default();
                    hasher.write(value.as_str().as_bytes());
                    let span = group.whole();
                    for part in [
                        span.start().numerator(),
                        span.start().denominator(),
                        span.end().numerator(),
                        span.end().denominator(),
                    ] {
                        hasher.write(&part.to_le_bytes());
                    }
                    seeded_roll_unit(policy.seed(), hasher.finish()) < policy.keep_probability()
                } else {
                    lifecycle_survives_degrade(group.key, policy)
                }
            })
            .collect(),
    )
}

fn lifecycle_key(event: &EvaluatedEvent) -> LifecycleKey {
    match event.kind() {
        EvaluatedEventKind::StartVoice {
            voice_whole,
            voice_id,
        }
        | EvaluatedEventKind::UpdateVoiceControls {
            voice_whole,
            voice_id,
        } => LifecycleKey {
            voice_id: *voice_id,
            voice_whole: *voice_whole,
        },
    }
}

fn lifecycle_survives_degrade(key: LifecycleKey, policy: DegradePolicy) -> bool {
    seeded_roll_unit(policy.seed(), stable_hash(&key)) < policy.keep_probability()
}

fn deduplicate_events(
    events: Vec<EvaluatedEvent>,
    policy: DeduplicatePolicy,
) -> Vec<EvaluatedEvent> {
    let groups = group_lifecycles(events);
    let kept = match policy.winner() {
        DeduplicateWinner::First => deduplicate_groups_keep_first(groups, policy.key()),
        DeduplicateWinner::Last => deduplicate_groups_keep_last(groups, policy.key()),
    };
    flatten_lifecycle_groups(kept)
}

fn deduplicate_groups_keep_first(
    groups: Vec<LifecycleGroup>,
    key_policy: DeduplicateKey,
) -> Vec<LifecycleGroup> {
    let mut seen = std::collections::BTreeSet::new();
    let mut kept = Vec::new();
    for group in groups {
        let key = duplicate_group_key(&group, key_policy);
        if seen.insert(key) {
            kept.push(group);
        }
    }
    kept
}

fn deduplicate_groups_keep_last(
    groups: Vec<LifecycleGroup>,
    key_policy: DeduplicateKey,
) -> Vec<LifecycleGroup> {
    let mut seen = std::collections::BTreeSet::new();
    let mut kept = Vec::new();
    for group in groups.into_iter().rev() {
        let key = duplicate_group_key(&group, key_policy);
        if seen.insert(key) {
            kept.push(group);
        }
    }
    kept.reverse();
    kept
}

fn duplicate_group_key(group: &LifecycleGroup, key_policy: DeduplicateKey) -> u64 {
    match key_policy {
        DeduplicateKey::Lifecycle => stable_hash(&group.key),
        DeduplicateKey::WholeSpanAndIntent => stable_hash(&(group.whole(), group.intent())),
        DeduplicateKey::StartAndIntent => stable_hash(&(group.whole().start(), group.intent())),
        DeduplicateKey::WholeSpanAndValue => {
            stable_hash(&(group.whole(), musical_value_key(group)))
        }
        DeduplicateKey::StartAndValue => {
            stable_hash(&(group.whole().start(), musical_value_key(group)))
        }
    }
}

fn musical_value_key(group: &LifecycleGroup) -> u64 {
    match group
        .representative()
        .projected()
        .as_moment()
        .value_identity()
    {
        Some(value) => stable_hash(&(true, value)),
        None => stable_hash(&(false, group.intent())),
    }
}

fn evaluate_priority_merge(
    children: &[Score],
    window: &TransportSpan,
    policy: PriorityMergePolicy,
) -> Result<Vec<EvaluatedEvent>, ControlModelError> {
    let mut accepted = Vec::<LifecycleGroup>::new();
    for child in children {
        let candidate_groups = group_lifecycles(evaluate_score(child, window)?);
        for candidate in candidate_groups {
            if !accepted
                .iter()
                .any(|existing| lifecycle_groups_conflict(existing, &candidate, policy.conflict()))
            {
                accepted.push(candidate);
            }
        }
    }

    Ok(flatten_lifecycle_groups(accepted))
}

fn lifecycle_groups_conflict(
    left: &LifecycleGroup,
    right: &LifecycleGroup,
    policy: ConflictPolicy,
) -> bool {
    match policy {
        ConflictPolicy::SameWholeStartAndIntent => {
            left.whole().start() == right.whole().start() && left.intent() == right.intent()
        }
        ConflictPolicy::SameWholeSpanAndIntent => {
            left.whole() == right.whole() && left.intent() == right.intent()
        }
        ConflictPolicy::WholeSpanOverlap => left.whole().intersects(&right.whole()),
        ConflictPolicy::SameWholeStartAndValue => {
            left.whole().start() == right.whole().start()
                && musical_value_key(left) == musical_value_key(right)
        }
        ConflictPolicy::SameWholeSpanAndValue => {
            left.whole() == right.whole() && musical_value_key(left) == musical_value_key(right)
        }
    }
}

fn evaluate_weighted_choice(
    options: &[WeightedScore],
    seed: u64,
    window: &TransportSpan,
) -> Result<Vec<EvaluatedEvent>, ControlModelError> {
    if options.is_empty() {
        return Ok(Vec::new());
    }

    let mut events = Vec::new();
    for cycle_window in split_by_cycle(window) {
        let cycle_index = cycle_window.start().floor();
        let Some(selected) = choose_weighted_option(options, seed, cycle_index) else {
            continue;
        };
        events.extend(evaluate_score(selected.score(), &cycle_window)?);
    }
    Ok(events)
}

fn choose_weighted_option(
    options: &[WeightedScore],
    seed: u64,
    cycle_index: i64,
) -> Option<&WeightedScore> {
    let total_weight = options
        .iter()
        .fold(Time::ZERO, |total, option| total + option.weight());
    if total_weight <= Time::ZERO {
        return None;
    }

    let roll = seeded_roll_below(seed, choice_cycle_key(cycle_index), total_weight);
    let mut cursor = Time::ZERO;
    for option in options {
        cursor = cursor + option.weight();
        if roll < cursor {
            return Some(option);
        }
    }

    options.last()
}

fn mask_clip_events(
    owner: ScoreNodeId,
    events: Vec<EvaluatedEvent>,
    mask_controls: Vec<EvaluatedControl>,
) -> Vec<EvaluatedEvent> {
    let open_spans = mask_controls
        .iter()
        .filter_map(gate_open_span)
        .collect::<Vec<_>>();
    if open_spans.is_empty() {
        return Vec::new();
    }

    let groups = group_lifecycles(events);
    let mut clipped_groups = Vec::new();
    for group in groups {
        let mut clipped_events = Vec::new();
        for event in group.events {
            for open_span in &open_spans {
                let Some(visible) = event.projected().visible().intersection(open_span) else {
                    continue;
                };

                let projected = remap_projected(
                    event.projected(),
                    event.projected().whole(),
                    visible,
                    event.projected().controls().clone(),
                );
                clipped_events.push(event.clone().with_projected(owner, projected));
            }
        }
        if !clipped_events.is_empty() {
            clipped_groups.push(LifecycleGroup {
                key: group.key,
                events: clipped_events,
            });
        }
    }

    flatten_lifecycle_groups(clipped_groups)
}

fn mask_clip_controls(
    owner: ControlScoreNodeId,
    source: Vec<EvaluatedControl>,
    mask_controls: Vec<EvaluatedControl>,
) -> Vec<EvaluatedControl> {
    let open_spans = mask_controls
        .iter()
        .filter_map(gate_open_span)
        .collect::<Vec<_>>();
    if open_spans.is_empty() {
        return Vec::new();
    }

    let mut clipped = Vec::new();
    for control in source {
        for open_span in &open_spans {
            let Some(visible) = control.visible.intersection(open_span) else {
                continue;
            };
            clipped.push(control.remap(owner, control.whole, visible, control.value.clone()));
        }
    }
    clipped
}

fn evaluate_control_priority_merge(
    children: &[ControlScore],
    window: &TransportSpan,
    policy: PriorityMergePolicy,
) -> Result<Vec<EvaluatedControl>, ControlModelError> {
    let mut accepted = Vec::<EvaluatedControl>::new();
    for child in children {
        for candidate in evaluate_control_score(child, window)? {
            if !accepted
                .iter()
                .any(|existing| control_tiles_conflict(existing, &candidate, policy.conflict()))
            {
                accepted.push(candidate);
            }
        }
    }
    Ok(accepted)
}

fn control_tiles_conflict(
    left: &EvaluatedControl,
    right: &EvaluatedControl,
    policy: ConflictPolicy,
) -> bool {
    if left.key != right.key {
        return false;
    }
    match policy {
        ConflictPolicy::SameWholeStartAndIntent => left.whole.start() == right.whole.start(),
        ConflictPolicy::SameWholeSpanAndIntent => left.whole == right.whole,
        ConflictPolicy::SameWholeStartAndValue => {
            left.whole.start() == right.whole.start() && left.value == right.value
        }
        ConflictPolicy::SameWholeSpanAndValue => {
            left.whole == right.whole && left.value == right.value
        }
        ConflictPolicy::WholeSpanOverlap => left.whole.intersects(&right.whole),
    }
}

fn evaluate_control_weighted_choice(
    options: &[WeightedControlScore],
    seed: u64,
    window: &TransportSpan,
) -> Result<Vec<EvaluatedControl>, ControlModelError> {
    if options.is_empty() {
        return Ok(Vec::new());
    }

    let mut controls = Vec::new();
    for cycle_window in split_by_cycle(window) {
        let cycle_index = cycle_window.start().floor();
        let Some(selected) = choose_weighted_control_option(options, seed, cycle_index) else {
            continue;
        };
        controls.extend(evaluate_control_score(selected.score(), &cycle_window)?);
    }
    Ok(controls)
}

fn choose_weighted_control_option(
    options: &[WeightedControlScore],
    seed: u64,
    cycle_index: i64,
) -> Option<&WeightedControlScore> {
    let total_weight = options
        .iter()
        .fold(Time::ZERO, |total, option| total + option.weight());
    if total_weight <= Time::ZERO {
        return None;
    }

    let roll = seeded_roll_below(seed, choice_cycle_key(cycle_index), total_weight);
    let mut cursor = Time::ZERO;
    for option in options {
        cursor = cursor + option.weight();
        if roll < cursor {
            return Some(option);
        }
    }

    options.last()
}

fn gate_open_span(control: &EvaluatedControl) -> Option<TransportSpan> {
    if control.key != ControlKey::Gate {
        return None;
    }
    match control.value {
        ControlValue::Bool(true) => Some(control.whole),
        ControlValue::Bool(false) => None,
        _ => None,
    }
}

#[derive(Debug, Clone, Copy)]
struct StableHasher {
    state: u64,
}

impl Default for StableHasher {
    fn default() -> Self {
        Self {
            state: 0xCBF2_9CE4_8422_2325,
        }
    }
}

impl Hasher for StableHasher {
    fn finish(&self) -> u64 {
        self.state
    }

    fn write(&mut self, bytes: &[u8]) {
        for byte in bytes {
            self.state ^= u64::from(*byte);
            self.state = self.state.wrapping_mul(0x0000_0100_0000_01B3);
        }
    }
}

fn stable_hash<T: Hash>(value: &T) -> u64 {
    let mut hasher = StableHasher::default();
    value.hash(&mut hasher);
    hasher.finish()
}

fn seeded_roll_unit(seed: u64, key: u64) -> Time {
    const DENOMINATOR: i64 = 1_000_000;
    let value = splitmix64(seed ^ key);
    Time::new((value % DENOMINATOR as u64) as i64, DENOMINATOR)
}

fn choice_cycle_key(cycle: i64) -> u64 {
    let mut hasher = StableHasher::default();
    hasher.write(&cycle.to_le_bytes());
    hasher.finish()
}

fn seeded_roll_below(seed: u64, key: u64, upper_bound: Time) -> Time {
    seeded_roll_unit(seed, key) * upper_bound
}

fn splitmix64(mut value: u64) -> u64 {
    value = value.wrapping_add(0x9E37_79B9_7F4A_7C15);
    value = (value ^ (value >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    value = (value ^ (value >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    value ^ (value >> 31)
}

fn evaluate_control_score(
    score: &ControlScore,
    window: &TransportSpan,
) -> Result<Vec<EvaluatedControl>, ControlModelError> {
    let mut controls = evaluate_control_score_unsorted(score, window)?;
    sort_controls(&mut controls);
    Ok(controls)
}

fn evaluate_control_score_unsorted(
    score: &ControlScore,
    window: &TransportSpan,
) -> Result<Vec<EvaluatedControl>, ControlModelError> {
    Ok(match score.kind() {
        ControlScoreKind::QuerySource(source) => {
            let work = source
                .0
                .estimated_work((window.end() - window.start()).value());
            if !work.is_finite() || work < 0.0 || work > 16_384.0 {
                return Err(ControlModelError::QuerySource(
                    "control query exceeds its preparation budget".into(),
                ));
            }
            let values = source.0.query(window)?;
            if values.len() > 16_384 {
                return Err(ControlModelError::QuerySource(
                    "too many control segments in one window".into(),
                ));
            }
            let mut result = Vec::new();
            for item in values {
                item.value.validate_for(&item.key)?;
                if let Some(visible) = item.span.intersection(window) {
                    result.push(EvaluatedControl::new(
                        score.id(),
                        item.span,
                        visible,
                        item.key,
                        item.value,
                    ));
                }
            }
            result
        }
        ControlScoreKind::Track(track) => evaluate_control_track_leaf(score.id(), track, window),
        ControlScoreKind::Merge(children) => {
            let mut controls = Vec::new();
            for child in children {
                controls.extend(evaluate_control_score(child, window)?);
            }
            controls
        }
        ControlScoreKind::Concat(children) => {
            evaluate_control_score_concat(score.id(), children, window)?
        }
        ControlScoreKind::Arrange { segments, period } => {
            evaluate_control_arrangement(score.id(), segments, *period, window)?
        }
        ControlScoreKind::CycleRoute(children) => evaluate_control_cycle_route(children, window)?,
        ControlScoreKind::CycleSlots(children) => evaluate_control_cycle_slots(children, window)?,
        ControlScoreKind::WeightedCycleSlots(children) => evaluate_control_weighted_slots(
            &children
                .iter()
                .map(|child| (child.score(), child.weight()))
                .collect::<Vec<_>>(),
            window,
        )?,
        ControlScoreKind::TimeScale { inner, rate } => {
            let scaled_window =
                TransportSpan::new(window.start() * *rate, window.end() * *rate).unwrap();

            evaluate_control_score(inner, &scaled_window)?
                .into_iter()
                .map(|control| {
                    control.remap(
                        score.id(),
                        scale_span(control.whole, Time::ONE / *rate),
                        scale_span(control.visible, Time::ONE / *rate),
                        control.value.clone(),
                    )
                })
                .collect()
        }
        ControlScoreKind::Shift { inner, offset } => {
            let shifted_window =
                TransportSpan::new(window.start() - *offset, window.end() - *offset).unwrap();

            evaluate_control_score(inner, &shifted_window)?
                .into_iter()
                .map(|control| {
                    control.remap(
                        score.id(),
                        translate_span(control.whole, *offset),
                        translate_span(control.visible, *offset),
                        control.value.clone(),
                    )
                })
                .collect()
        }
        ControlScoreKind::ReflectCycle { inner } => {
            evaluate_reflected_controls(score.id(), inner, window)?
        }
        ControlScoreKind::PriorityMerge { children, policy } => {
            evaluate_control_priority_merge(children, window, *policy)?
        }
        ControlScoreKind::WeightedChoice { options, seed } => {
            evaluate_control_weighted_choice(options, *seed, window)?
        }
        ControlScoreKind::MaskClip { source, mask } => mask_clip_controls(
            score.id(),
            evaluate_control_score(source, window)?,
            evaluate_control_score(mask, window)?,
        ),
    })
}

fn score_sequence_origin(score: &Score) -> Time {
    match score.kind() {
        ScoreKind::QuerySource(_) => Time::ZERO,
        ScoreKind::Voice(voice) => voice
            .tiles()
            .iter()
            .map(|tile| tile.phase().start())
            .min()
            .unwrap_or(Time::ZERO),
        ScoreKind::Events(events) => events
            .first()
            .map(|event| event.span().start())
            .unwrap_or(Time::ZERO),
        ScoreKind::Concat(_) | ScoreKind::WeightedCycleSlots(_) | ScoreKind::Arrange { .. } => {
            Time::ZERO
        }
        ScoreKind::Merge(children)
        | ScoreKind::CycleRoute(children)
        | ScoreKind::CycleSlots(children) => children
            .iter()
            .map(score_sequence_origin)
            .min()
            .unwrap_or(Time::ZERO),
        ScoreKind::TimeScale { inner, rate } => score_sequence_origin(inner) / *rate,
        ScoreKind::Shift { inner, offset } => score_sequence_origin(inner) + *offset,
        ScoreKind::ReflectCycle { inner } => score_sequence_origin(inner),
        ScoreKind::SpaceShift { inner, .. }
        | ScoreKind::SpaceScale { inner, .. }
        | ScoreKind::SpaceReflect { inner, .. }
        | ScoreKind::Degrade { inner, .. }
        | ScoreKind::Deduplicate { inner, .. } => score_sequence_origin(inner),
        ScoreKind::PriorityMerge { children, .. } => children
            .iter()
            .map(score_sequence_origin)
            .min()
            .unwrap_or(Time::ZERO),
        ScoreKind::WeightedChoice { options, .. } => options
            .iter()
            .map(|option| score_sequence_origin(option.score()))
            .min()
            .unwrap_or(Time::ZERO),
        ScoreKind::MaskClip { source, .. } | ScoreKind::WithControls { source, .. } => {
            score_sequence_origin(source)
        }
    }
}

fn score_sequencing_extent(score: &Score) -> Time {
    match score.kind() {
        ScoreKind::QuerySource(source) => source.0.extent(),
        ScoreKind::Arrange { period, .. } => *period,
        ScoreKind::WeightedCycleSlots(_) => Time::ONE,
        ScoreKind::Voice(voice) => voice.period(),
        ScoreKind::Events(events) => {
            let Some(first) = events.first() else {
                return Time::ZERO;
            };
            let start = first.span().start();
            events
                .iter()
                .map(|event| event.span().end())
                .max()
                .unwrap_or(start)
                - start
        }
        ScoreKind::Concat(children) => children
            .iter()
            .map(score_sequencing_extent)
            .fold(Time::ZERO, |total, extent| total + extent),
        ScoreKind::Merge(children) => {
            let origin = children
                .iter()
                .map(score_sequence_origin)
                .min()
                .unwrap_or(Time::ZERO);
            let end = children
                .iter()
                .map(|child| score_sequence_origin(child) + score_sequencing_extent(child))
                .max()
                .unwrap_or(origin);
            end - origin
        }
        ScoreKind::CycleRoute(children) | ScoreKind::CycleSlots(children) => children
            .iter()
            .map(score_sequencing_extent)
            .max()
            .unwrap_or(Time::ZERO),
        ScoreKind::TimeScale { inner, rate } => score_sequencing_extent(inner) / *rate,
        ScoreKind::Shift { inner, .. }
        | ScoreKind::ReflectCycle { inner }
        | ScoreKind::SpaceShift { inner, .. }
        | ScoreKind::SpaceScale { inner, .. }
        | ScoreKind::SpaceReflect { inner, .. }
        | ScoreKind::Degrade { inner, .. }
        | ScoreKind::Deduplicate { inner, .. } => score_sequencing_extent(inner),
        ScoreKind::PriorityMerge { children, .. } => children
            .iter()
            .map(score_sequencing_extent)
            .max()
            .unwrap_or(Time::ZERO),
        ScoreKind::WeightedChoice { options, .. } => options
            .iter()
            .map(|option| score_sequencing_extent(option.score()))
            .max()
            .unwrap_or(Time::ZERO),
        ScoreKind::MaskClip { source, .. } | ScoreKind::WithControls { source, .. } => {
            score_sequencing_extent(source)
        }
    }
}

fn control_sequencing_extent(score: &ControlScore) -> Time {
    match score.kind() {
        ControlScoreKind::QuerySource(source) => source.0.extent(),
        ControlScoreKind::Arrange { period, .. } => *period,
        ControlScoreKind::WeightedCycleSlots(_) => Time::ONE,
        ControlScoreKind::Track(track) => track.period(),
        ControlScoreKind::Concat(children) => children
            .iter()
            .map(control_sequencing_extent)
            .fold(Time::ZERO, |total, extent| total + extent),
        ControlScoreKind::Merge(children)
        | ControlScoreKind::CycleRoute(children)
        | ControlScoreKind::CycleSlots(children) => children
            .iter()
            .map(control_sequencing_extent)
            .max()
            .unwrap_or(Time::ZERO),
        ControlScoreKind::TimeScale { inner, rate } => control_sequencing_extent(inner) / *rate,
        ControlScoreKind::Shift { inner, .. }
        | ControlScoreKind::ReflectCycle { inner }
        | ControlScoreKind::MaskClip { source: inner, .. } => control_sequencing_extent(inner),
        ControlScoreKind::PriorityMerge { children, .. } => children
            .iter()
            .map(control_sequencing_extent)
            .max()
            .unwrap_or(Time::ZERO),
        ControlScoreKind::WeightedChoice { options, .. } => options
            .iter()
            .map(|option| control_sequencing_extent(option.score()))
            .max()
            .unwrap_or(Time::ZERO),
    }
}

const MAX_ARRANGEMENT_QUERY_WORK: usize = 16_384;

fn checked_arrangement_time(num: i128, den: i128) -> Result<Time, ControlModelError> {
    let (mut a, mut b) = (num.abs(), den);
    while b != 0 {
        (a, b) = (b, a % b);
    }
    let numerator = i64::try_from(num / a).map_err(|_| ControlModelError::ArrangementQueryLimit)?;
    let denominator =
        i64::try_from(den / a).map_err(|_| ControlModelError::ArrangementQueryLimit)?;
    Ok(Time::new(numerator, denominator))
}

fn arrangement_add(a: Time, b: Time) -> Result<Time, ControlModelError> {
    checked_arrangement_time(
        i128::from(a.numerator()) * i128::from(b.denominator())
            + i128::from(b.numerator()) * i128::from(a.denominator()),
        i128::from(a.denominator()) * i128::from(b.denominator()),
    )
}

fn arrangement_shift(
    span: TransportSpan,
    offset: Time,
) -> Result<TransportSpan, ControlModelError> {
    Ok(TransportSpan::new(
        arrangement_add(span.start(), offset)?,
        arrangement_add(span.end(), offset)?,
    )
    .unwrap())
}

fn arrangement_floor_ratio(a: Time, b: Time) -> Result<i64, ControlModelError> {
    let num = i128::from(a.numerator()) * i128::from(b.denominator());
    let den = i128::from(a.denominator()) * i128::from(b.numerator());
    i64::try_from(num.div_euclid(den)).map_err(|_| ControlModelError::ArrangementQueryLimit)
}

fn arrangement_occurrences<T>(
    segments: &[Timed<T>],
    period: Time,
    window: &TransportSpan,
    mut visit: impl FnMut(usize, &Timed<T>, Time, TransportSpan) -> Result<(), ControlModelError>,
) -> Result<(), ControlModelError> {
    // Bound work before iterating, including empty arrangements and direct seeks.
    if window.start().value().abs() > 1_000_000_000.0
        || window.end().value().abs() > 1_000_000_000.0
    {
        return Err(ControlModelError::ArrangementQueryLimit);
    }
    let first = arrangement_floor_ratio(window.start(), period)?;
    let last = arrangement_floor_ratio(window.end(), period)?;
    if (i128::from(last) - i128::from(first) + 1) * segments.len() as i128
        > MAX_ARRANGEMENT_QUERY_WORK as i128
    {
        return Err(ControlModelError::ArrangementQueryLimit);
    }
    let mut work = 0;
    for cycle in first..=last {
        let cycle_start = checked_arrangement_time(
            i128::from(period.numerator()) * i128::from(cycle),
            i128::from(period.denominator()),
        )?;
        let mut offset = Time::ZERO;
        for (index, segment) in segments.iter().enumerate() {
            let group_start = arrangement_add(cycle_start, offset)?;
            let extent = segment.duration() * Time::whole_number(i64::from(segment.repeats()));
            offset = offset + extent;
            let group_end = arrangement_add(group_start, extent)?;
            let Some(overlap) =
                window.intersection(&TransportSpan::new(group_start, group_end).unwrap())
            else {
                continue;
            };
            let local_start = arrangement_add(overlap.start(), Time::ZERO - group_start)?;
            let mut repeat =
                arrangement_floor_ratio(local_start, segment.duration())?.max(0) as u64;
            while repeat < u64::from(segment.repeats()) {
                let start = arrangement_add(
                    group_start,
                    segment.duration() * Time::whole_number(repeat as i64),
                )?;
                if start >= overlap.end() {
                    break;
                }
                let end = arrangement_add(start, segment.duration())?;
                let occurrence = TransportSpan::new(start, end).unwrap();
                if let Some(visible) = overlap.intersection(&occurrence) {
                    work += 1;
                    if work > MAX_ARRANGEMENT_QUERY_WORK {
                        return Err(ControlModelError::ArrangementQueryLimit);
                    }
                    visit(
                        index,
                        segment,
                        start,
                        arrangement_shift(visible, Time::ZERO - start)?,
                    )?;
                }
                repeat += 1;
            }
        }
    }
    Ok(())
}

fn arrangement_control(
    value: &ControlValue,
    original: TransportSpan,
    clipped: TransportSpan,
    offset: Time,
) -> ControlValue {
    match slice_control_value(value, original, clipped) {
        ControlValue::Signal(signal) => {
            ControlValue::Signal(signal.with_phase(signal.phase() - signal.rate() * offset))
        }
        ControlValue::Semitones(control) => {
            ControlValue::Semitones(control.clone().shifted(offset))
        }
        value => value,
    }
}

fn evaluate_score_arrangement(
    owner: ScoreNodeId,
    segments: &[TimedScore],
    period: Time,
    window: &TransportSpan,
) -> Result<Vec<EvaluatedEvent>, ControlModelError> {
    let mut events = Vec::new();
    arrangement_occurrences(
        segments,
        period,
        window,
        |index, segment, offset, local_window| {
            let clip = TransportSpan::new(Time::ZERO, segment.duration()).unwrap();
            for event in evaluate_score(segment.source(), &local_window)? {
                let Some(whole) = event.projected().whole().intersection(&clip) else {
                    continue;
                };
                let Some(visible) = event.projected().visible().intersection(&whole) else {
                    continue;
                };
                let (voice_whole, old_voice_id, is_start) = match *event.kind() {
                    EvaluatedEventKind::StartVoice {
                        voice_whole,
                        voice_id,
                    } => (voice_whole, voice_id, true),
                    EvaluatedEventKind::UpdateVoiceControls {
                        voice_whole,
                        voice_id,
                    } => (voice_whole, voice_id, false),
                };
                let Some(voice_whole) = voice_whole.intersection(&clip) else {
                    continue;
                };
                let voice_whole = arrangement_shift(voice_whole, offset)?;
                let voice_id =
                    VoiceInstanceId::new(stable_hash(&(owner, index, offset, old_voice_id)) as i64);
                let kind = if is_start {
                    EvaluatedEventKind::StartVoice {
                        voice_whole,
                        voice_id,
                    }
                } else {
                    EvaluatedEventKind::UpdateVoiceControls {
                        voice_whole,
                        voice_id,
                    }
                };
                let controls = event
                    .projected()
                    .controls()
                    .iter()
                    .map(|(key, value)| {
                        (
                            key.clone(),
                            arrangement_control(value, event.projected().whole(), whole, offset),
                        )
                    })
                    .collect();
                let global_whole = arrangement_shift(whole, offset)?;
                let mut projected = remap_projected(
                    event.projected(),
                    global_whole,
                    arrangement_shift(visible, offset)?,
                    controls,
                );
                projected = ProjectedMoment::new(
                    projected
                        .as_moment()
                        .clone()
                        .with_position(projected.position().shifted_time(offset)),
                    projected.visible(),
                    projected.controls().clone(),
                );
                let key = QueryKey::new(
                    owner,
                    LeafEventId::new(stable_hash(&(index, event.key.owner, event.key.leaf_event))),
                    global_whole,
                );
                events.push(EvaluatedEvent::new_with_kind(key, projected, kind));
                if events.len() > MAX_ARRANGEMENT_QUERY_WORK {
                    return Err(ControlModelError::ArrangementQueryLimit);
                }
            }
            Ok(())
        },
    )?;
    Ok(events)
}

fn evaluate_control_arrangement(
    owner: ControlScoreNodeId,
    segments: &[TimedControlScore],
    period: Time,
    window: &TransportSpan,
) -> Result<Vec<EvaluatedControl>, ControlModelError> {
    let mut controls = Vec::new();
    arrangement_occurrences(
        segments,
        period,
        window,
        |_, segment, offset, local_window| {
            let clip = TransportSpan::new(Time::ZERO, segment.duration()).unwrap();
            for control in evaluate_control_score(segment.source(), &local_window)? {
                let Some(whole) = control.whole.intersection(&clip) else {
                    continue;
                };
                let Some(visible) = control.visible.intersection(&whole) else {
                    continue;
                };
                controls.push(control.remap(
                    owner,
                    arrangement_shift(whole, offset)?,
                    arrangement_shift(visible, offset)?,
                    arrangement_control(&control.value, control.whole, whole, offset),
                ));
                if controls.len() > MAX_ARRANGEMENT_QUERY_WORK {
                    return Err(ControlModelError::ArrangementQueryLimit);
                }
            }
            Ok(())
        },
    )?;
    Ok(controls)
}

fn evaluate_score_concat(
    owner: ScoreNodeId,
    children: &[Score],
    window: &TransportSpan,
) -> Result<Vec<EvaluatedEvent>, ControlModelError> {
    let mut events = Vec::new();
    let mut offset = Time::ZERO;
    for child in children {
        let extent = score_sequencing_extent(child);
        if extent > Time::ZERO {
            let segment = TransportSpan::new(offset, offset + extent).unwrap();
            if let Some(child_window) = window.intersection(&segment) {
                let origin = score_sequence_origin(child);
                let normalized = Score::shift(child.clone(), Time::ZERO - origin);
                let shifted_window =
                    TransportSpan::new(child_window.start() - offset, child_window.end() - offset)
                        .unwrap();
                events.extend(
                    evaluate_score(&normalized, &shifted_window)?
                        .into_iter()
                        .map(|event| {
                            let projected = remap_projected(
                                event.projected(),
                                translate_span(event.projected().whole(), offset),
                                translate_span(event.projected().visible(), offset),
                                event.projected().controls().clone(),
                            );
                            event.with_projected(owner, projected)
                        }),
                );
            }
        }
        offset = offset + extent;
    }
    Ok(events)
}

fn evaluate_control_score_concat(
    owner: ControlScoreNodeId,
    children: &[ControlScore],
    window: &TransportSpan,
) -> Result<Vec<EvaluatedControl>, ControlModelError> {
    let mut controls = Vec::new();
    let mut offset = Time::ZERO;
    for child in children {
        let extent = control_sequencing_extent(child);
        if extent > Time::ZERO {
            let segment = TransportSpan::new(offset, offset + extent).unwrap();
            if let Some(child_window) = window.intersection(&segment) {
                let shifted_window =
                    TransportSpan::new(child_window.start() - offset, child_window.end() - offset)
                        .unwrap();
                controls.extend(
                    evaluate_control_score(child, &shifted_window)?
                        .into_iter()
                        .map(|control| {
                            control.remap(
                                owner,
                                translate_span(control.whole, offset),
                                translate_span(control.visible, offset),
                                control.value.clone(),
                            )
                        }),
                );
            }
        }
        offset = offset + extent;
    }
    Ok(controls)
}

fn evaluate_score_cycle_route(
    children: &[Score],
    window: &TransportSpan,
) -> Result<Vec<EvaluatedEvent>, ControlModelError> {
    if children.is_empty() {
        return Ok(Vec::new());
    }

    let mut events = Vec::new();
    for cycle_window in split_by_cycle(window) {
        let cycle_index = cycle_window
            .start()
            .floor()
            .rem_euclid(children.len() as i64);
        events.extend(evaluate_score(
            &children[cycle_index as usize],
            &cycle_window,
        )?);
    }

    Ok(events)
}

fn evaluate_control_cycle_route(
    children: &[ControlScore],
    window: &TransportSpan,
) -> Result<Vec<EvaluatedControl>, ControlModelError> {
    if children.is_empty() {
        return Ok(Vec::new());
    }

    let mut controls = Vec::new();
    for cycle_window in split_by_cycle(window) {
        let cycle_index = cycle_window
            .start()
            .floor()
            .rem_euclid(children.len() as i64);
        controls.extend(evaluate_control_score(
            &children[cycle_index as usize],
            &cycle_window,
        )?);
    }
    Ok(controls)
}

fn evaluate_score_cycle_slots(
    children: &[Score],
    window: &TransportSpan,
) -> Result<Vec<EvaluatedEvent>, ControlModelError> {
    evaluate_score_weighted_slots(
        &children
            .iter()
            .map(|child| (child, Time::ONE))
            .collect::<Vec<_>>(),
        window,
    )
}

fn evaluate_score_weighted_slots(
    children: &[(&Score, Time)],
    window: &TransportSpan,
) -> Result<Vec<EvaluatedEvent>, ControlModelError> {
    if children.is_empty() {
        return Ok(Vec::new());
    }

    if children.len() == 1 {
        return evaluate_score(children[0].0, window);
    }
    let total = children
        .iter()
        .fold(Time::ZERO, |sum, (_, weight)| sum + *weight);
    let mut events = Vec::new();

    for cycle_window in split_by_cycle(window) {
        let cycle_start = cycle_start(cycle_window.start());

        let mut offset = Time::ZERO;
        for (child, weight) in children {
            let slot_width = *weight / total;
            let slot_start = cycle_start + offset;
            offset = offset + slot_width;
            let slot_end = slot_start + slot_width;
            let slot = TransportSpan::new(slot_start, slot_end).unwrap();
            let Some(visible_slot) = cycle_window.intersection(&slot) else {
                continue;
            };

            let inner_window = TransportSpan::new(
                cycle_start + (visible_slot.start() - slot_start) / slot_width,
                cycle_start + (visible_slot.end() - slot_start) / slot_width,
            )
            .unwrap();

            events.extend(
                evaluate_score(child, &inner_window)?
                    .into_iter()
                    .map(|event| {
                        let projected = remap_projected(
                            event.projected(),
                            map_span_into_slot(
                                event.projected().whole(),
                                cycle_start,
                                slot_start,
                                slot_width,
                            ),
                            map_span_into_slot(
                                event.projected().visible(),
                                cycle_start,
                                slot_start,
                                slot_width,
                            ),
                            event.projected().controls().clone(),
                        );

                        event.with_projected(child.id(), projected)
                    }),
            );
        }
    }

    Ok(events)
}

fn evaluate_control_cycle_slots(
    children: &[ControlScore],
    window: &TransportSpan,
) -> Result<Vec<EvaluatedControl>, ControlModelError> {
    evaluate_control_weighted_slots(
        &children
            .iter()
            .map(|child| (child, Time::ONE))
            .collect::<Vec<_>>(),
        window,
    )
}

fn evaluate_control_weighted_slots(
    children: &[(&ControlScore, Time)],
    window: &TransportSpan,
) -> Result<Vec<EvaluatedControl>, ControlModelError> {
    if children.is_empty() {
        return Ok(Vec::new());
    }

    if children.len() == 1 {
        return evaluate_control_score(children[0].0, window);
    }
    let total = children
        .iter()
        .fold(Time::ZERO, |sum, (_, weight)| sum + *weight);
    let mut controls = Vec::new();

    for cycle_window in split_by_cycle(window) {
        let cycle_start = cycle_start(cycle_window.start());

        let mut offset = Time::ZERO;
        for (child, weight) in children {
            let slot_width = *weight / total;
            let slot_start = cycle_start + offset;
            offset = offset + slot_width;
            let slot_end = slot_start + slot_width;
            let slot = TransportSpan::new(slot_start, slot_end).unwrap();
            let Some(visible_slot) = cycle_window.intersection(&slot) else {
                continue;
            };

            let inner_window = TransportSpan::new(
                cycle_start + (visible_slot.start() - slot_start) / slot_width,
                cycle_start + (visible_slot.end() - slot_start) / slot_width,
            )
            .unwrap();

            controls.extend(
                evaluate_control_score(child, &inner_window)?
                    .into_iter()
                    .map(|control| {
                        control.remap(
                            child.id(),
                            map_span_into_slot(control.whole, cycle_start, slot_start, slot_width),
                            map_span_into_slot(
                                control.visible,
                                cycle_start,
                                slot_start,
                                slot_width,
                            ),
                            control.value.clone(),
                        )
                    }),
            );
        }
    }

    Ok(controls)
}

fn evaluate_reflected_score(
    owner: ScoreNodeId,
    inner: &Score,
    window: &TransportSpan,
) -> Result<Vec<EvaluatedEvent>, ControlModelError> {
    let mut events = Vec::new();

    for cycle_window in split_by_cycle(window) {
        let cycle = cycle_span(cycle_window.start());
        let reflected_window = reflect_span(cycle_window, cycle);

        events.extend(
            evaluate_score(inner, &reflected_window)?
                .into_iter()
                .map(|event| {
                    let projected = remap_projected(
                        event.projected(),
                        reflect_span(event.projected().whole(), cycle),
                        reflect_span(event.projected().visible(), cycle),
                        reflect_controls(event.projected().controls()),
                    );

                    event.with_projected(owner, projected)
                }),
        );
    }

    Ok(events)
}

fn evaluate_reflected_controls(
    owner: ControlScoreNodeId,
    inner: &ControlScore,
    window: &TransportSpan,
) -> Result<Vec<EvaluatedControl>, ControlModelError> {
    let mut controls = Vec::new();

    for cycle_window in split_by_cycle(window) {
        let cycle = cycle_span(cycle_window.start());
        let reflected_window = reflect_span(cycle_window, cycle);

        controls.extend(
            evaluate_control_score(inner, &reflected_window)?
                .into_iter()
                .map(|control| {
                    control.remap(
                        owner,
                        reflect_span(control.whole, cycle),
                        reflect_span(control.visible, cycle),
                        reflect_control_value(&control.value),
                    )
                }),
        );
    }

    Ok(controls)
}

fn evaluate_voice_leaf(
    origin: ScoreNodeId,
    voice: &Voice,
    window: &TransportSpan,
) -> Vec<EvaluatedEvent> {
    if matches!(voice.repeat(), Repeat::Until(until) if until <= Time::ZERO) {
        return Vec::new();
    }

    let effective_window = match repeat_extent(voice.repeat()) {
        Some(extent) => match window.intersection(&extent) {
            Some(window) => window,
            None => return Vec::new(),
        },
        None => *window,
    };

    let first_repetition =
        first_repetition(voice.repeat(), voice.period(), effective_window.start());

    let mut projected = Vec::new();
    let mut repetition = first_repetition;

    while repeat_is_active(
        voice.repeat(),
        voice.period(),
        repetition,
        effective_window.end(),
    ) {
        let offset = voice_offset(repetition, voice.period());
        if offset >= effective_window.end() {
            break;
        }

        projected.extend(project_voice_repetition(
            origin,
            voice,
            offset,
            &effective_window,
        ));
        repetition += 1;
    }

    projected
}

fn evaluate_control_track_leaf(
    origin: ControlScoreNodeId,
    track: &ControlTrack,
    window: &TransportSpan,
) -> Vec<EvaluatedControl> {
    if matches!(track.repeat(), Repeat::Until(until) if until <= Time::ZERO) {
        return Vec::new();
    }

    let effective_window = match repeat_extent(track.repeat()) {
        Some(extent) => match window.intersection(&extent) {
            Some(window) => window,
            None => return Vec::new(),
        },
        None => *window,
    };

    let first_repetition =
        first_repetition(track.repeat(), track.period(), effective_window.start());

    let mut projected = Vec::new();
    let mut repetition = first_repetition;

    while repeat_is_active(
        track.repeat(),
        track.period(),
        repetition,
        effective_window.end(),
    ) {
        let offset = voice_offset(repetition, track.period());
        if offset >= effective_window.end() {
            break;
        }

        projected.extend(project_control_repetition(
            origin,
            track,
            offset,
            &effective_window,
        ));
        repetition += 1;
    }

    projected
}

fn evaluate_events_leaf(
    origin: ScoreNodeId,
    events: &[Moment],
    window: &TransportSpan,
) -> Vec<EvaluatedEvent> {
    events
        .iter()
        .enumerate()
        .filter_map(|(index, moment)| {
            let whole = moment.span();
            let visible = whole.intersection(window)?;
            let projected = ProjectedMoment::new(moment.clone(), visible, ControlMap::new());

            Some(EvaluatedEvent::new(
                QueryKey::new(
                    origin,
                    leaf_event_id(moment.id().map(|id| id.value()), index),
                    whole,
                ),
                projected,
            ))
        })
        .collect()
}

fn apply_controls(
    owner: ScoreNodeId,
    source: &Score,
    controls: &ControlScore,
    window: &TransportSpan,
) -> Result<Vec<EvaluatedEvent>, ControlModelError> {
    let source_events = evaluate_score(source, window)?;
    let control_spans = evaluate_control_score(controls, window)?;
    // A slow enclosing clock can make this visible window tiny while onset
    // recovery still queries a full source cycle per held event. Bound that
    // additional work before any widened query allocates control repetitions.
    let recovery_queries = source_events
        .iter()
        .filter(|event| event.projected().whole().start() < window.start())
        .count();
    super::preparation::validate_onset_control_recovery(controls, recovery_queries).map_err(
        |error| {
            ControlModelError::QuerySource(format!("held-note onset control recovery: {error}"))
        },
    )?;
    let owned_runtime_lanes: Vec<_> = [
        ControlKey::Transpose,
        ControlKey::Pan,
        ControlKey::PostGain,
        ControlKey::PitchBend,
        ControlKey::Expression,
    ]
    .into_iter()
    .filter(|key| control_score_contains_key(controls, key))
    .collect();
    let mut resolved = Vec::new();

    for event in source_events {
        // Seeking into a held note still samples its original onset, even if
        // the controlling tile has already ended before this query window.
        let onset = event.projected().whole().start();
        let onset_spans = if onset < window.start() {
            let recovery_end = onset
                .numerator()
                .checked_add(onset.denominator())
                .map(|numerator| Time::new(numerator, onset.denominator()))
                .ok_or_else(|| {
                    ControlModelError::QuerySource(
                        "held-note onset control recovery exceeds the supported time range".into(),
                    )
                })?;
            Some(evaluate_control_score(
                controls,
                &TransportSpan::new(onset, recovery_end).unwrap(),
            )?)
        } else {
            None
        };
        let overlapping_controls = control_spans
            .iter()
            .filter(|control| control.whole.intersects(&event.projected().whole()))
            .collect::<Vec<_>>();
        let onset_candidates: Vec<_> = onset_spans
            .as_ref()
            .map(|spans| spans.iter().collect())
            .unwrap_or_else(|| overlapping_controls.clone());
        let onset_controls = onset_candidates
            .iter()
            .copied()
            .filter(|control| control.key.spec().timing() == ControlTiming::Onset)
            .collect::<Vec<_>>();
        let lifecycle_controls = onset_candidates
            .iter()
            .copied()
            .filter(|control| control.key.spec().timing() == ControlTiming::VoiceLifecycle)
            .collect::<Vec<_>>();
        let segment_controls = overlapping_controls
            .iter()
            .copied()
            .filter(|control| control.key.spec().timing() == ControlTiming::SegmentSampled)
            .collect::<Vec<_>>();
        let runtime_controls = overlapping_controls
            .iter()
            .copied()
            .filter(|control| control.key.spec().timing() == ControlTiming::ContinuousRuntime)
            .collect::<Vec<_>>();

        let lifecycle_whole = event.projected().whole();
        let lifecycle_key = event.key().with_owner(owner, lifecycle_whole);
        let voice_id = voice_instance_id(lifecycle_key, lifecycle_whole);

        let segment_boundaries = partition_boundaries(lifecycle_whole, &segment_controls);
        let mut first_visible_segment = true;

        for (segment_index, boundary_window) in segment_boundaries.windows(2).enumerate() {
            let Some(segment_whole) = TransportSpan::new(boundary_window[0], boundary_window[1])
            else {
                continue;
            };
            let Some(visible) = segment_whole
                .intersection(&event.projected().whole())
                .and_then(|span| span.intersection(window))
            else {
                continue;
            };

            let mut controls_map = event.projected().controls().clone();
            apply_onset_controls(
                &mut controls_map,
                &onset_controls,
                event.projected().intent(),
                segment_whole.start(),
            )?;
            apply_onset_controls(
                &mut controls_map,
                &lifecycle_controls,
                event.projected().intent(),
                lifecycle_whole.start(),
            )?;
            apply_segment_controls(
                &mut controls_map,
                &segment_controls,
                event.projected().intent(),
                segment_whole,
            )?;

            let entry_time = visible.start();
            let runtime_snapshot = runtime_control_snapshot(
                &runtime_controls,
                event.projected().intent(),
                entry_time,
            )?;
            for (key, value) in runtime_snapshot.clone() {
                merge_into_control_map(&mut controls_map, key, value)?;
            }
            // A scheduler window can begin after this lane's last tile.
            // Restore its identity while retaining any inherited nested value.
            for key in &owned_runtime_lanes {
                controls_map
                    .entry(key.clone())
                    .or_insert_with(|| runtime_lane_identity(key));
            }

            let projected =
                remap_projected(event.projected(), lifecycle_whole, visible, controls_map);

            let emit_start = segment_index == 0 && visible.start() == lifecycle_whole.start();
            if first_visible_segment {
                first_visible_segment = false;
            }

            if emit_start {
                resolved.push(EvaluatedEvent::new_with_kind(
                    lifecycle_key,
                    projected,
                    EvaluatedEventKind::StartVoice {
                        voice_whole: lifecycle_whole,
                        voice_id,
                    },
                ));

                resolved.extend(runtime_update_events(RuntimeUpdateContext {
                    owner,
                    event: &event,
                    voice_whole: lifecycle_whole,
                    entry_time,
                    voice_id,
                    runtime_controls: &runtime_controls,
                    segment_controls: &segment_controls,
                    segment_boundaries: &segment_boundaries,
                    window,
                })?);
            } else {
                let update_key = event.key().with_owner(owner, segment_whole);
                resolved.push(EvaluatedEvent::new_with_kind(
                    update_key,
                    projected,
                    EvaluatedEventKind::UpdateVoiceControls {
                        voice_whole: lifecycle_whole,
                        voice_id,
                    },
                ));
            }
        }
    }

    Ok(resolved)
}

fn control_score_contains_key(score: &ControlScore, key: &ControlKey) -> bool {
    match score.kind() {
        ControlScoreKind::QuerySource(source) => source.0.contains_key(key),
        ControlScoreKind::Arrange { segments, .. } => segments
            .iter()
            .any(|segment| control_score_contains_key(segment.source(), key)),
        ControlScoreKind::Track(track) => track.tiles().iter().any(|tile| tile.key() == key),
        ControlScoreKind::Merge(children)
        | ControlScoreKind::Concat(children)
        | ControlScoreKind::CycleRoute(children)
        | ControlScoreKind::CycleSlots(children)
        | ControlScoreKind::PriorityMerge { children, .. } => children
            .iter()
            .any(|child| control_score_contains_key(child, key)),
        ControlScoreKind::WeightedCycleSlots(children)
        | ControlScoreKind::WeightedChoice {
            options: children, ..
        } => children
            .iter()
            .any(|child| control_score_contains_key(child.score(), key)),
        ControlScoreKind::TimeScale { inner, .. }
        | ControlScoreKind::Shift { inner, .. }
        | ControlScoreKind::ReflectCycle { inner }
        | ControlScoreKind::MaskClip { source: inner, .. } => {
            control_score_contains_key(inner, key)
        }
    }
}

fn partition_boundaries(base: TransportSpan, controls: &[&EvaluatedControl]) -> Vec<Time> {
    let mut boundaries = vec![base.start(), base.end()];

    for control in controls {
        let Some(overlap) = control.whole.intersection(&base) else {
            continue;
        };
        boundaries.push(overlap.start());
        boundaries.push(overlap.end());
    }

    boundaries.sort();
    boundaries.dedup();
    boundaries
}

fn apply_onset_controls(
    controls_map: &mut ControlMap,
    controls: &[&EvaluatedControl],
    intent: &Intent,
    at: Time,
) -> Result<(), ControlModelError> {
    for control in controls
        .iter()
        .copied()
        .filter(|control| span_contains_time(control.whole, at))
    {
        validate_control_support_for_intent(&control.key, intent)?;
        let value = sample_onset_control(&control.key, &control.value, at);
        merge_into_control_map(controls_map, control.key.clone(), value)?;
    }

    Ok(())
}

/// A velocity signal chooses one intensity for the entire note. Resolve it
/// before outer timing transforms and before combining other velocity owners,
/// so rate, reverse, arrangement, query windows and rendering block sizes
/// cannot change the value of an existing onset.
fn sample_onset_control(key: &ControlKey, value: &ControlValue, at: Time) -> ControlValue {
    match (key, value) {
        (ControlKey::Velocity, ControlValue::Signal(signal)) => {
            ControlValue::Unipolar(UnitValue::new(signal.eval_at(at).clamp(0.0, 1.0)).unwrap())
        }
        _ => value.clone(),
    }
}

fn apply_segment_controls(
    controls_map: &mut ControlMap,
    controls: &[&EvaluatedControl],
    intent: &Intent,
    segment_whole: TransportSpan,
) -> Result<(), ControlModelError> {
    for control in controls
        .iter()
        .copied()
        .filter(|control| control.whole.contains(&segment_whole))
    {
        validate_control_support_for_intent(&control.key, intent)?;
        let value = slice_control_value(&control.value, control.whole, segment_whole);
        merge_into_control_map(controls_map, control.key.clone(), value)?;
    }

    Ok(())
}

fn runtime_control_snapshot(
    controls: &[&EvaluatedControl],
    intent: &Intent,
    at: Time,
) -> Result<ControlMap, ControlModelError> {
    let mut controls_map = ControlMap::new();

    for control in controls
        .iter()
        .copied()
        .filter(|control| span_contains_time(control.whole, at))
    {
        validate_control_support_for_intent(&control.key, intent)?;
        let value = if control.key == ControlKey::Transpose {
            match control.value.clone() {
                ControlValue::Ramp { from, to } => {
                    ControlValue::Semitones(crate::domain::control::SemitoneControl::signal(
                        crate::domain::signal::Signal::linear_ramp(
                            control.whole.start(),
                            control.whole.end(),
                            from,
                            to,
                        ),
                    )?)
                }
                value => value,
            }
        } else {
            control.value.clone()
        };
        merge_into_control_map(&mut controls_map, control.key.clone(), value)?;
    }

    Ok(controls_map)
}

struct RuntimeUpdateContext<'a> {
    owner: ScoreNodeId,
    event: &'a EvaluatedEvent,
    voice_whole: TransportSpan,
    entry_time: Time,
    voice_id: VoiceInstanceId,
    runtime_controls: &'a [&'a EvaluatedControl],
    segment_controls: &'a [&'a EvaluatedControl],
    segment_boundaries: &'a [Time],
    window: &'a TransportSpan,
}

fn runtime_update_events(
    context: RuntimeUpdateContext<'_>,
) -> Result<Vec<EvaluatedEvent>, ControlModelError> {
    let RuntimeUpdateContext {
        owner,
        event,
        voice_whole,
        entry_time,
        voice_id,
        runtime_controls,
        segment_controls,
        segment_boundaries,
        window,
    } = context;
    let mut resolved = Vec::new();
    let boundaries = partition_boundaries(voice_whole, runtime_controls);
    let mut previous_snapshot =
        runtime_control_snapshot(runtime_controls, event.projected().intent(), entry_time)?;

    for boundary_window in boundaries.windows(2) {
        let Some(update_whole) = TransportSpan::new(boundary_window[0], boundary_window[1]) else {
            continue;
        };
        if update_whole.start() < entry_time {
            continue;
        }

        let snapshot = runtime_control_snapshot(
            runtime_controls,
            event.projected().intent(),
            update_whole.start(),
        )?;
        if update_whole.start() != voice_whole.start()
            && segment_boundaries.contains(&update_whole.start())
        {
            previous_snapshot = snapshot;
            continue;
        }
        if snapshot == previous_snapshot {
            continue;
        }

        let Some(visible) = update_whole
            .intersection(&event.projected().whole())
            .and_then(|span| span.intersection(window))
        else {
            previous_snapshot = snapshot;
            continue;
        };

        // Runtime updates carry the active segment settings too. Otherwise a
        // pan/expression boundary would look like the end of an active filter
        // or send when the audio renderer restores omitted segment lanes.
        let mut update_controls = event.projected().controls().clone();
        apply_onset_controls(
            &mut update_controls,
            segment_controls,
            event.projected().intent(),
            update_whole.start(),
        )?;
        for (key, value) in snapshot.clone() {
            merge_into_control_map(&mut update_controls, key, value)?;
        }
        for key in [
            ControlKey::Transpose,
            ControlKey::Pan,
            ControlKey::PostGain,
            ControlKey::PitchBend,
            ControlKey::Expression,
        ] {
            if previous_snapshot.contains_key(&key)
                && !snapshot.contains_key(&key)
                && !update_controls.contains_key(&key)
            {
                update_controls.insert(key.clone(), runtime_lane_identity(&key));
            }
        }
        let projected = remap_projected(event.projected(), voice_whole, visible, update_controls);
        let key = event.key().with_owner(owner, update_whole);
        resolved.push(EvaluatedEvent::new_with_kind(
            key,
            projected,
            EvaluatedEventKind::UpdateVoiceControls {
                voice_whole,
                voice_id,
            },
        ));
        previous_snapshot = snapshot;
    }

    Ok(resolved)
}

fn merge_into_control_map(
    controls_map: &mut ControlMap,
    key: ControlKey,
    value: ControlValue,
) -> Result<(), ControlModelError> {
    if let Some(existing) = controls_map.get(&key).cloned() {
        let merged = merge_control_values(&key, existing, value)?;
        controls_map.insert(key, merged);
    } else {
        controls_map.insert(key, value);
    }

    Ok(())
}

fn runtime_lane_identity(key: &ControlKey) -> ControlValue {
    match key {
        ControlKey::Transpose => {
            ControlValue::Semitones(crate::domain::control::SemitoneControl::default())
        }
        ControlKey::Pan => ControlValue::Pan(crate::domain::control::PanControl::default()),
        ControlKey::PostGain => ControlValue::Scalar(1.0),
        ControlKey::PitchBend => {
            ControlValue::Bipolar(crate::domain::control::SignedUnitValue::new(0.0).unwrap())
        }
        ControlKey::Expression => {
            ControlValue::Unipolar(crate::domain::control::UnitValue::new(1.0).unwrap())
        }
        _ => unreachable!(
            "only additive and multiplicative runtime lanes have reset identities here"
        ),
    }
}

fn span_contains_time(span: TransportSpan, time: Time) -> bool {
    span.start() <= time && time < span.end()
}

fn validate_control_support_for_intent(
    key: &ControlKey,
    intent: &Intent,
) -> Result<(), ControlModelError> {
    let supported = match intent {
        Intent::Sample(_) => key.spec().support().supports_sample(),
        Intent::Synth(_) => key.spec().support().supports_synth(),
        _ => false,
    };

    if supported {
        Ok(())
    } else {
        Err(ControlModelError::UnsupportedControlForSource {
            key: key.canonical_name(),
            source_kind: intent_source_kind(intent),
        })
    }
}

fn intent_source_kind(intent: &Intent) -> &'static str {
    match intent {
        Intent::Sample(_) => "sample playback",
        Intent::Synth(_) => "synth playback",
        _ => "non-sample/synth intent",
    }
}

fn merge_control_values(
    key: &ControlKey,
    existing: ControlValue,
    incoming: ControlValue,
) -> Result<ControlValue, ControlModelError> {
    match key.spec().merge() {
        ControlMerge::Override => Err(ControlModelError::ConflictingOverrideControls {
            key: key.canonical_name(),
        }),
        ControlMerge::Multiply => multiply_control_values(key, existing, incoming),
        ControlMerge::Add => add_control_values(key, existing, incoming),
    }
}

fn multiply_control_values(
    key: &ControlKey,
    left: ControlValue,
    right: ControlValue,
) -> Result<ControlValue, ControlModelError> {
    match key {
        ControlKey::Gain => multiply_gain_values(key, left, right),
        ControlKey::Velocity | ControlKey::Expression => Ok(ControlValue::Unipolar(
            UnitValue::new(unit_scalar(&left, key)? * unit_scalar(&right, key)?).unwrap(),
        )),
        ControlKey::PostGain => Ok(ControlValue::Scalar(
            non_negative_scalar(&left, key)? * non_negative_scalar(&right, key)?,
        )),
        _ => Err(ControlModelError::InvalidControlValue {
            key: key.canonical_name(),
            found: left.kind_label(),
        }),
    }
}

fn multiply_gain_values(
    key: &ControlKey,
    left: ControlValue,
    right: ControlValue,
) -> Result<ControlValue, ControlModelError> {
    match (left, right) {
        (ControlValue::Signal(signal), value) | (value, ControlValue::Signal(signal)) => {
            let multiplier = non_negative_scalar(&value, key)?;
            Ok(ControlValue::Signal(
                signal
                    .with_bias(signal.bias() * multiplier)
                    .with_depth(signal.depth() * multiplier),
            ))
        }
        (
            ControlValue::Ramp { from, to },
            ControlValue::Ramp {
                from: rhs_from,
                to: rhs_to,
            },
        ) => Ok(ControlValue::Ramp {
            from: from * rhs_from,
            to: to * rhs_to,
        }),
        (ControlValue::Ramp { from, to }, rhs) => {
            let multiplier = non_negative_scalar(&rhs, key)?;
            Ok(ControlValue::Ramp {
                from: from * multiplier,
                to: to * multiplier,
            })
        }
        (lhs, ControlValue::Ramp { from, to }) => {
            let multiplier = non_negative_scalar(&lhs, key)?;
            Ok(ControlValue::Ramp {
                from: from * multiplier,
                to: to * multiplier,
            })
        }
        (lhs, rhs) => Ok(ControlValue::Scalar(
            non_negative_scalar(&lhs, key)? * non_negative_scalar(&rhs, key)?,
        )),
    }
}

fn add_control_values(
    key: &ControlKey,
    left: ControlValue,
    right: ControlValue,
) -> Result<ControlValue, ControlModelError> {
    match key {
        ControlKey::Pitch => Ok(ControlValue::Scalar(
            finite_scalar(&left, key)? + finite_scalar(&right, key)?,
        )),
        ControlKey::Transpose => Ok(ControlValue::Semitones(
            semitone_control(&left)?.combine(semitone_control(&right)?)?,
        )),
        ControlKey::Pan => Ok(ControlValue::Pan(
            pan_control(&left)?.add(pan_control(&right)?)?,
        )),
        ControlKey::PitchBend => Ok(ControlValue::Bipolar(
            SignedUnitValue::new(
                (signed_unit_scalar(&left, key)? + signed_unit_scalar(&right, key)?)
                    .clamp(-1.0, 1.0),
            )
            .unwrap(),
        )),
        _ => Err(ControlModelError::InvalidControlValue {
            key: key.canonical_name(),
            found: left.kind_label(),
        }),
    }
}

fn semitone_control(
    value: &ControlValue,
) -> Result<crate::domain::control::SemitoneControl, ControlModelError> {
    use crate::domain::control::SemitoneControl;
    match value {
        ControlValue::Scalar(value) => SemitoneControl::constant(*value),
        ControlValue::Signal(signal) => SemitoneControl::signal(*signal),
        ControlValue::Semitones(control) => Ok(control.clone()),
        _ => Err(ControlModelError::InvalidControlValue {
            key: "transpose",
            found: value.kind_label(),
        }),
    }
}

fn pan_control(
    value: &ControlValue,
) -> Result<crate::domain::control::PanControl, ControlModelError> {
    use crate::domain::control::PanControl;
    match value {
        ControlValue::Bipolar(value) => Ok(PanControl::new(*value)),
        ControlValue::Pan(control) => Ok(*control),
        _ => Err(ControlModelError::InvalidControlValue {
            key: "pan",
            found: value.kind_label(),
        }),
    }
}

fn finite_scalar(value: &ControlValue, key: &ControlKey) -> Result<f64, ControlModelError> {
    match value {
        ControlValue::Scalar(value) if value.is_finite() => Ok(*value),
        _ => Err(ControlModelError::InvalidControlValue {
            key: key.canonical_name(),
            found: value.kind_label(),
        }),
    }
}

fn non_negative_scalar(value: &ControlValue, key: &ControlKey) -> Result<f64, ControlModelError> {
    match value {
        ControlValue::Scalar(value) if value.is_finite() && *value >= 0.0 => Ok(*value),
        ControlValue::Unipolar(value) => Ok(value.value()),
        _ => Err(ControlModelError::InvalidControlValue {
            key: key.canonical_name(),
            found: value.kind_label(),
        }),
    }
}

fn unit_scalar(value: &ControlValue, key: &ControlKey) -> Result<f64, ControlModelError> {
    match value {
        ControlValue::Unipolar(value) => Ok(value.value()),
        ControlValue::Scalar(value) if value.is_finite() && (0.0..=1.0).contains(value) => {
            Ok(*value)
        }
        _ => Err(ControlModelError::InvalidControlValue {
            key: key.canonical_name(),
            found: value.kind_label(),
        }),
    }
}

fn signed_unit_scalar(value: &ControlValue, key: &ControlKey) -> Result<f64, ControlModelError> {
    match value {
        ControlValue::Bipolar(value) => Ok(value.value()),
        _ => Err(ControlModelError::InvalidControlValue {
            key: key.canonical_name(),
            found: value.kind_label(),
        }),
    }
}

fn project_voice_repetition(
    origin: ScoreNodeId,
    voice: &Voice,
    offset: Time,
    window: &TransportSpan,
) -> Vec<EvaluatedEvent> {
    voice
        .iter()
        .enumerate()
        .filter_map(|(index, tile)| {
            let whole = clip_to_repeat_limit(tile.phase().translate(offset), voice.repeat())?;
            let visible = whole.intersection(window)?;
            let mut moment =
                Moment::new(whole, tile.intent().clone()).with_position(tile.position());
            if let Some(value) = tile.value_identity() {
                moment = moment.with_value_identity(value.clone());
            }
            if let Some(id) = tile.id() {
                moment = moment.with_id(id.value());
            }

            Some(EvaluatedEvent::new(
                QueryKey::new(
                    origin,
                    leaf_event_id(tile.id().map(|id| id.value()), index),
                    whole,
                ),
                ProjectedMoment::new(moment, visible, ControlMap::new()),
            ))
        })
        .collect()
}

fn project_control_repetition(
    origin: ControlScoreNodeId,
    track: &ControlTrack,
    offset: Time,
    window: &TransportSpan,
) -> Vec<EvaluatedControl> {
    track
        .iter()
        .filter_map(|tile| project_control_tile(origin, track.repeat(), tile, offset, window))
        .collect()
}

fn project_control_tile(
    origin: ControlScoreNodeId,
    repeat: Repeat,
    tile: &ControlTile,
    offset: Time,
    window: &TransportSpan,
) -> Option<EvaluatedControl> {
    let whole = clip_to_repeat_limit(tile.phase().translate(offset), repeat)?;
    let visible = whole.intersection(window)?;

    Some(EvaluatedControl::new(
        origin,
        whole,
        visible,
        tile.key().clone(),
        tile.value().clone(),
    ))
}

fn repeat_extent(repeat: Repeat) -> Option<TransportSpan> {
    match repeat {
        Repeat::Until(until) if until > Time::ZERO => {
            Some(TransportSpan::new(Time::ZERO, until).unwrap())
        }
        Repeat::Until(_) => None,
        _ => None,
    }
}

fn first_repetition(repeat: Repeat, period: Time, window_start: Time) -> i64 {
    let repetition = (window_start / period).floor();
    // A cyclic source has phase before and after zero. Finite sources retain
    // their explicit beginning at zero, including after a timeline shift.
    if repeat == Repeat::Forever {
        repetition
    } else {
        repetition.max(0)
    }
}

fn repeat_is_active(repeat: Repeat, period: Time, repetition: i64, window_end: Time) -> bool {
    match repeat {
        Repeat::Forever => true,
        Repeat::Once => repetition == 0,
        Repeat::Count(count) => (0..i64::from(count)).contains(&repetition),
        Repeat::Until(until) => {
            if until <= Time::ZERO {
                false
            } else {
                let offset = voice_offset(repetition, period);
                offset < until && offset < window_end
            }
        }
    }
}

fn voice_offset(repetition: i64, period: Time) -> Time {
    period * Time::whole_number(repetition)
}

fn clip_to_repeat_limit(whole: TransportSpan, repeat: Repeat) -> Option<TransportSpan> {
    match repeat {
        Repeat::Until(until) if until > Time::ZERO => {
            whole.intersection(&TransportSpan::new(Time::ZERO, until).unwrap())
        }
        Repeat::Until(_) => None,
        _ => Some(whole),
    }
}

fn leaf_event_id(stable_id: Option<u64>, fallback_index: usize) -> LeafEventId {
    LeafEventId::new(stable_id.unwrap_or(fallback_index as u64))
}

fn voice_instance_id(key: QueryKey, voice_whole: TransportSpan) -> VoiceInstanceId {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    key.owner.hash(&mut hasher);
    key.leaf_event.hash(&mut hasher);
    voice_whole.start().hash(&mut hasher);
    voice_whole.end().hash(&mut hasher);
    VoiceInstanceId::new(hasher.finish() as i64)
}

fn slice_control_value(
    value: &ControlValue,
    whole: TransportSpan,
    subspan: TransportSpan,
) -> ControlValue {
    match value {
        ControlValue::Ramp { from, to } => {
            let duration = whole.end() - whole.start();
            let start_progress = ((subspan.start() - whole.start()) / duration).value();
            let end_progress = ((subspan.end() - whole.start()) / duration).value();
            let start_value = from + (to - from) * start_progress;
            let end_value = from + (to - from) * end_progress;
            ControlValue::Ramp {
                from: start_value,
                to: end_value,
            }
        }
        _ => value.clone(),
    }
}

fn reflect_controls(controls: &ControlMap) -> ControlMap {
    controls
        .iter()
        .map(|(key, value)| (key.clone(), reflect_control_value(value)))
        .collect()
}

fn reflect_control_value(value: &ControlValue) -> ControlValue {
    match value {
        ControlValue::Ramp { from, to } => ControlValue::Ramp {
            from: *to,
            to: *from,
        },
        _ => value.clone(),
    }
}

/// Rewrites only the spatial position of each event's projected moment.
///
/// Position is pure moment data, so the scheduling identity (query key and
/// lifecycle kind) is preserved unchanged: a spatial transform places an
/// existing voice elsewhere, it never forks a new lifecycle.
fn map_event_positions(
    events: Vec<EvaluatedEvent>,
    map: impl Fn(SpatialMotion) -> SpatialMotion,
) -> Vec<EvaluatedEvent> {
    events
        .into_iter()
        .map(|event| {
            let projected = remap_position(event.projected(), &map);
            EvaluatedEvent::new_with_kind(event.key(), projected, event.kind().clone())
        })
        .collect()
}

fn remap_position(
    projected: &ProjectedMoment,
    map: &impl Fn(SpatialMotion) -> SpatialMotion,
) -> ProjectedMoment {
    let mut moment = Moment::new(projected.whole(), projected.intent().clone())
        .with_position(map(projected.position()));
    if let Some(value) = projected.as_moment().value_identity() {
        moment = moment.with_value_identity(value.clone());
    }
    if let Some(id) = projected.id() {
        moment = moment.with_id(id);
    }

    ProjectedMoment::new(moment, projected.visible(), projected.controls().clone())
}

fn remap_projected(
    projected: &ProjectedMoment,
    whole: TransportSpan,
    visible: TransportSpan,
    controls: ControlMap,
) -> ProjectedMoment {
    let mut moment =
        Moment::new(whole, projected.intent().clone()).with_position(projected.position());
    if let Some(value) = projected.as_moment().value_identity() {
        moment = moment.with_value_identity(value.clone());
    }
    if let Some(id) = projected.id() {
        moment = moment.with_id(id);
    }

    ProjectedMoment::new(moment, visible, controls)
}

fn scale_span(span: TransportSpan, factor: Time) -> TransportSpan {
    TransportSpan::new(span.start() * factor, span.end() * factor).unwrap()
}

fn translate_span(span: TransportSpan, offset: Time) -> TransportSpan {
    span.translate(offset)
}

fn map_span_into_slot(
    span: TransportSpan,
    cycle_start: Time,
    slot_start: Time,
    slot_width: Time,
) -> TransportSpan {
    // Map whole-span endpoints by their own cycles. A slow event can span
    // multiple slots; its lifecycle identity must not change with the queried
    // cycle. Exact end boundaries belong to the preceding slot.
    let offset = slot_start - cycle_start;
    let map = |time: Time, is_end: bool| {
        let mut cycle = time.floor();
        if is_end && time == Time::whole_number(cycle) {
            cycle -= 1;
        }
        let origin = Time::whole_number(cycle);
        origin + offset + (time - origin) * slot_width
    };
    TransportSpan::new(map(span.start(), false), map(span.end(), true)).unwrap()
}

fn reflect_span(span: TransportSpan, cycle: TransportSpan) -> TransportSpan {
    TransportSpan::new(
        cycle.start() + (cycle.end() - span.end()),
        cycle.start() + (cycle.end() - span.start()),
    )
    .unwrap()
}

fn cycle_span(time: Time) -> TransportSpan {
    let start = cycle_start(time);
    TransportSpan::new(start, start + Time::ONE).unwrap()
}

fn cycle_start(time: Time) -> Time {
    Time::whole_number(time.floor())
}

fn split_by_cycle(window: &TransportSpan) -> Vec<TransportSpan> {
    let mut pieces = Vec::new();
    let mut current_start = window.start();

    while current_start < window.end() {
        let next_boundary = Time::whole_number(current_start.floor() + 1);
        let current_end = next_boundary.min(window.end());
        pieces.push(TransportSpan::new(current_start, current_end).unwrap());
        current_start = current_end;
    }

    pieces
}

fn sort_evaluated_events(events: &mut [EvaluatedEvent]) {
    events.sort_by(|left, right| {
        left.projected()
            .whole()
            .start()
            .cmp(&right.projected().whole().start())
            .then(
                left.projected()
                    .whole()
                    .end()
                    .cmp(&right.projected().whole().end()),
            )
    });
}

fn sort_controls(controls: &mut [EvaluatedControl]) {
    controls.sort_by(|left, right| {
        left.whole
            .start()
            .cmp(&right.whole.start())
            .then(left.whole.end().cmp(&right.whole.end()))
            .then(left.key.cmp(&right.key))
            .then(left.owner.cmp(&right.owner))
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{
        control::{ControlTile, ControlTrack},
        intent::Intent,
        voice::Tile,
    };

    fn span(start: (i64, i64), end: (i64, i64)) -> TransportSpan {
        TransportSpan::new(Time::new(start.0, start.1), Time::new(end.0, end.1)).unwrap()
    }

    fn sample_event_score(start: (i64, i64), end: (i64, i64), sample: &str) -> Score {
        Score::events(vec![
            Moment::spanning(
                Time::new(start.0, start.1),
                Time::new(end.0, end.1),
                Intent::sample(sample),
            )
            .unwrap(),
        ])
    }

    fn sample_voice(start: (i64, i64), end: (i64, i64), sample: &str, id: u64) -> Voice {
        Voice::new(
            Time::ONE,
            vec![
                Tile::spanning(
                    Time::new(start.0, start.1),
                    Time::new(end.0, end.1),
                    Intent::sample(sample),
                )
                .unwrap()
                .with_id(id),
            ],
        )
        .unwrap()
    }

    fn gain_ramp_track(start: (i64, i64), end: (i64, i64), from: f64, to: f64) -> ControlTrack {
        ControlTrack::new(
            Time::ONE,
            vec![
                ControlTile::spanning(
                    Time::new(start.0, start.1),
                    Time::new(end.0, end.1),
                    ControlKey::Gain,
                    ControlValue::Ramp { from, to },
                )
                .unwrap(),
            ],
        )
        .unwrap()
    }

    #[test]
    fn merge_overlays_source_queries_deterministically() {
        let score = Score::merge(vec![
            Score::from(sample_voice((0, 1), (1, 4), "kick", 1)),
            Score::from(sample_voice((0, 1), (1, 2), "hat", 2)),
        ]);
        let projected: Vec<_> = evaluate_score(&score, &span((0, 1), (1, 1)))
            .unwrap()
            .into_iter()
            .map(EvaluatedEvent::into_projected)
            .collect();

        assert_eq!(projected.len(), 2);
        assert_eq!(projected[0].intent(), &Intent::sample("kick"));
    }

    #[test]
    fn with_controls_slices_source_segments_and_attaches_control_map() {
        let score = Score::with_controls(
            Score::from(sample_voice((0, 1), (1, 1), "pad", 7)),
            ControlScore::from(gain_ramp_track((0, 1), (1, 2), 1.0, 0.0)),
        );
        let evaluated = evaluate_score(&score, &span((0, 1), (1, 1))).unwrap();
        let projected: Vec<_> = evaluated
            .iter()
            .filter(|event| matches!(event.kind(), EvaluatedEventKind::StartVoice { .. }))
            .cloned()
            .map(EvaluatedEvent::into_projected)
            .collect();

        assert_eq!(evaluated.len(), 2);
        assert_eq!(projected.len(), 1);
        let (start_voice_id, update_voice_id) = {
            let start = &evaluated[0];
            let update = &evaluated[1];
            let (start_id, update_id) = match (start.kind(), update.kind()) {
                (
                    EvaluatedEventKind::StartVoice { voice_id: s, .. },
                    EvaluatedEventKind::UpdateVoiceControls { voice_id: u, .. },
                ) => (*s, *u),
                other => panic!("expected start + update, got {other:?}"),
            };
            (start_id, update_id)
        };
        assert_eq!(start_voice_id, update_voice_id);
        assert!(matches!(
            evaluated[0].projected().controls().get(&ControlKey::Gain),
            Some(ControlValue::Ramp { .. })
        ));
    }

    #[test]
    fn voice_lifecycle_controls_attach_once_without_splitting_note_segments() {
        let controls = ControlTrack::new(
            Time::ONE,
            vec![
                ControlTile::spanning(
                    Time::ZERO,
                    Time::new(1, 2),
                    ControlKey::Attack,
                    ControlValue::Scalar(0.05),
                )
                .unwrap(),
            ],
        )
        .unwrap();
        let score = Score::with_controls(
            Score::from(sample_voice((0, 1), (1, 1), "pad", 7)),
            ControlScore::from(controls),
        );

        let evaluated = evaluate_score(&score, &span((0, 1), (1, 1))).unwrap();

        assert_eq!(evaluated.len(), 1);
        assert!(matches!(
            evaluated[0].kind(),
            EvaluatedEventKind::StartVoice { .. }
        ));
        assert!(matches!(
            evaluated[0].projected().controls().get(&ControlKey::Attack),
            Some(ControlValue::Scalar(value)) if (*value - 0.05).abs() < f64::EPSILON
        ));
    }

    #[test]
    fn continuous_runtime_controls_emit_updates_without_segmenting_notes() {
        let controls = ControlTrack::new(
            Time::ONE,
            vec![
                ControlTile::spanning(
                    Time::ZERO,
                    Time::new(1, 2),
                    ControlKey::PitchBend,
                    ControlValue::Bipolar(SignedUnitValue::new(0.0).unwrap()),
                )
                .unwrap(),
                ControlTile::spanning(
                    Time::new(1, 2),
                    Time::ONE,
                    ControlKey::PitchBend,
                    ControlValue::Bipolar(SignedUnitValue::new(1.0).unwrap()),
                )
                .unwrap(),
            ],
        )
        .unwrap();
        let score = Score::with_controls(
            Score::from(sample_voice((0, 1), (1, 1), "pad", 7)),
            ControlScore::from(controls),
        );

        let evaluated = evaluate_score(&score, &span((0, 1), (1, 1))).unwrap();

        assert_eq!(evaluated.len(), 2);
        let start = &evaluated[0];
        let update = &evaluated[1];
        let (voice_whole, voice_id) = match start.kind() {
            EvaluatedEventKind::StartVoice {
                voice_whole,
                voice_id,
            } => (*voice_whole, *voice_id),
            other => panic!("expected start event, found {other:?}"),
        };
        assert!(matches!(
            update.kind(),
            EvaluatedEventKind::UpdateVoiceControls {
                voice_whole: update_whole,
                voice_id: update_id,
            } if *update_whole == voice_whole && *update_id == voice_id
        ));
        assert_eq!(start.projected().whole(), span((0, 1), (1, 1)));
        assert_eq!(update.projected().whole(), span((0, 1), (1, 1)));
    }

    #[test]
    fn overlapping_override_controls_return_typed_error() {
        let track = ControlTrack::new(
            Time::ONE,
            vec![
                ControlTile::spanning(
                    Time::ZERO,
                    Time::new(3, 4),
                    ControlKey::PlaybackRate,
                    ControlValue::Scalar(0.5),
                )
                .unwrap(),
                ControlTile::spanning(
                    Time::new(1, 2),
                    Time::ONE,
                    ControlKey::PlaybackRate,
                    ControlValue::Scalar(0.7),
                )
                .unwrap(),
            ],
        )
        .unwrap();
        let score = Score::with_controls(
            Score::from(sample_voice((0, 1), (1, 1), "pad", 7)),
            ControlScore::from(track),
        );

        assert!(matches!(
            evaluate_score(&score, &span((0, 1), (1, 1))),
            Err(ControlModelError::ConflictingOverrideControls {
                key: "playback_rate"
            })
        ));
    }

    #[test]
    fn overlapping_gain_controls_multiply_deterministically() {
        let track = ControlTrack::new(
            Time::ONE,
            vec![
                ControlTile::spanning(
                    Time::ZERO,
                    Time::new(3, 4),
                    ControlKey::Gain,
                    ControlValue::Scalar(0.5),
                )
                .unwrap(),
                ControlTile::spanning(
                    Time::new(1, 2),
                    Time::ONE,
                    ControlKey::Gain,
                    ControlValue::Scalar(0.7),
                )
                .unwrap(),
            ],
        )
        .unwrap();
        let score = Score::with_controls(
            Score::from(sample_voice((0, 1), (1, 1), "pad", 7)),
            ControlScore::from(track),
        );
        let projected: Vec<_> = evaluate_score(&score, &span((0, 1), (1, 1)))
            .unwrap()
            .into_iter()
            .map(EvaluatedEvent::into_projected)
            .collect();

        assert_eq!(projected.len(), 3);
        assert!(matches!(
            projected[1].controls().get(&ControlKey::Gain),
            Some(ControlValue::Scalar(value)) if (*value - 0.35).abs() < f64::EPSILON
        ));
    }

    #[test]
    fn sample_only_controls_are_rejected_on_synth_sources() {
        let score = Score::with_controls(
            Score::from(
                Voice::new(
                    Time::ONE,
                    vec![
                        Tile::spanning(
                            Time::ZERO,
                            Time::ONE,
                            Intent::synth(crate::domain::intent::BuiltInSynthSource::Sine),
                        )
                        .unwrap(),
                    ],
                )
                .unwrap(),
            ),
            ControlScore::from(
                ControlTrack::new(
                    Time::ONE,
                    vec![
                        ControlTile::spanning(
                            Time::ZERO,
                            Time::ONE,
                            ControlKey::PlaybackRate,
                            ControlValue::Scalar(1.5),
                        )
                        .unwrap(),
                    ],
                )
                .unwrap(),
            ),
        );

        assert!(matches!(
            evaluate_score(&score, &span((0, 1), (1, 1))),
            Err(ControlModelError::UnsupportedControlForSource {
                key: "playback_rate",
                source_kind: "synth playback",
            })
        ));
    }

    #[test]
    fn reflect_cycle_reverses_gain_ramps() {
        let score = Score::reflect_cycle(Score::with_controls(
            Score::from(sample_voice((0, 1), (1, 1), "pad", 7)),
            ControlScore::from(gain_ramp_track((0, 1), (1, 1), 1.0, 0.0)),
        ));
        let projected: Vec<_> = evaluate_score(&score, &span((0, 1), (1, 1)))
            .unwrap()
            .into_iter()
            .map(EvaluatedEvent::into_projected)
            .collect();

        assert!(matches!(
            projected[0].controls().get(&ControlKey::Gain),
            Some(ControlValue::Ramp { from, to }) if *from <= *to
        ));
    }

    #[test]
    fn weighted_slots_repeat_fast_child_inside_its_allotted_duration() {
        let score = Score::weighted_cycle_slots(vec![
            WeightedScore::new(
                Score::time_scale(
                    Score::from(sample_voice((0, 1), (1, 1), "c", 31)),
                    Time::whole_number(3),
                ),
                Time::whole_number(2),
            ),
            WeightedScore::new(
                Score::from(sample_voice((0, 1), (1, 1), "d", 32)),
                Time::ONE,
            ),
        ]);
        for cycle in 0..8 {
            let events = evaluate_score(&score, &span((cycle, 1), (cycle + 1, 1))).unwrap();
            assert_eq!(events.len(), 4);
            for (index, event) in events[..3].iter().enumerate() {
                assert_eq!(event.projected().intent(), &Intent::sample("c"));
                assert_eq!(
                    event.projected().whole().start(),
                    Time::whole_number(cycle) + Time::new(index as i64 * 2, 9)
                );
                assert_eq!(
                    event.projected().whole().end() - event.projected().whole().start(),
                    Time::new(2, 9)
                );
            }
            assert_eq!(
                events[3].projected().whole().start(),
                Time::whole_number(cycle) + Time::new(2, 3)
            );
        }
    }

    #[test]
    fn weighted_slots_keep_nested_alternation_and_control_phase_on_seek() {
        let choices = Score::cycle_route(vec![
            Score::from(sample_voice((0, 1), (1, 1), "c", 41)),
            Score::from(sample_voice((0, 1), (1, 1), "d", 42)),
        ]);
        let score = Score::weighted_cycle_slots(vec![
            WeightedScore::new(choices, Time::whole_number(2)),
            WeightedScore::new(
                Score::from(sample_voice((0, 1), (1, 1), "g", 43)),
                Time::ONE,
            ),
        ]);
        let all = evaluate_score(&score, &span((0, 1), (8, 1))).unwrap();
        assert_eq!(all.len(), 16);
        for cycle in 0..8 {
            let events = evaluate_score(&score, &span((cycle, 1), (cycle + 1, 1))).unwrap();
            assert_eq!(
                events[0].projected().intent(),
                &Intent::sample(if cycle % 2 == 0 { "c" } else { "d" })
            );
            assert_eq!(
                events[0].projected().whole().end(),
                Time::whole_number(cycle) + Time::new(2, 3)
            );
            assert_eq!(
                events[1].projected().whole().start(),
                Time::whole_number(cycle) + Time::new(2, 3)
            );
        }
        let controls = ControlScore::weighted_cycle_slots(vec![
            WeightedControlScore::new(
                ControlScore::from(gain_ramp_track((0, 1), (1, 1), 0.2, 0.2)),
                Time::whole_number(2),
            ),
            WeightedControlScore::new(
                ControlScore::from(gain_ramp_track((0, 1), (1, 1), 0.8, 0.8)),
                Time::ONE,
            ),
        ]);
        let at_seven = evaluate_control_score(&controls, &span((7, 1), (8, 1))).unwrap();
        assert_eq!(at_seven.len(), 2);
        assert_eq!(at_seven[0].whole.end(), Time::new(23, 3));
        assert_eq!(at_seven[1].whole.start(), Time::new(23, 3));
    }

    #[test]
    fn slow_note_whole_and_voice_identity_survive_a_weighted_slot_seek() {
        let score = Score::weighted_cycle_slots(vec![
            WeightedScore::new(
                Score::time_scale(
                    Score::from(sample_voice((0, 1), (1, 1), "c", 61)),
                    Time::new(1, 2),
                ),
                Time::ONE,
            ),
            WeightedScore::new(
                Score::from(sample_voice((0, 1), (1, 1), "d", 62)),
                Time::ONE,
            ),
        ]);
        let first = evaluate_score(&score, &span((0, 1), (1, 1))).unwrap();
        let second = evaluate_score(&score, &span((1, 1), (2, 1))).unwrap();
        assert_eq!(first[0].projected().whole(), span((0, 1), (3, 2)));
        assert_eq!(first[0].projected().whole(), second[0].projected().whole());
        assert_eq!(first[0].kind(), second[0].kind());
        assert_eq!(first[0].projected().visible(), span((0, 1), (1, 2)));
        assert_eq!(second[0].projected().visible(), span((1, 1), (3, 2)));
    }

    #[test]
    fn single_weighted_slot_keeps_slow_note_as_one_lifecycle() {
        let score = Score::weighted_cycle_slots(vec![WeightedScore::new(
            Score::time_scale(
                Score::from(sample_voice((0, 1), (1, 1), "c", 51)),
                Time::new(1, 2),
            ),
            Time::ONE,
        )]);
        let events = evaluate_score(&score, &span((0, 1), (8, 1))).unwrap();
        assert_eq!(events.len(), 4);
        for (index, event) in events.iter().enumerate() {
            assert_eq!(
                event.projected().whole().start(),
                Time::whole_number(index as i64 * 2)
            );
            assert_eq!(
                event.projected().whole().end(),
                Time::whole_number(index as i64 * 2 + 2)
            );
        }
    }

    #[test]
    fn cycle_route_alternates_children_by_cycle() {
        let score = Score::cycle_route(vec![
            Score::from(sample_voice((0, 1), (1, 2), "kick", 1)),
            Score::from(sample_voice((0, 1), (1, 2), "snare", 2)),
        ]);

        let cycle_zero = evaluate_score(&score, &span((0, 1), (1, 1))).unwrap();
        let cycle_one = evaluate_score(&score, &span((1, 1), (2, 1))).unwrap();

        assert_eq!(cycle_zero[0].projected().intent(), &Intent::sample("kick"));
        assert_eq!(cycle_one[0].projected().intent(), &Intent::sample("snare"));
    }

    #[test]
    fn degrade_keeps_voice_lifecycle_events_together() {
        let controls = ControlTrack::new(
            Time::ONE,
            vec![
                ControlTile::spanning(
                    Time::ZERO,
                    Time::new(1, 2),
                    ControlKey::PitchBend,
                    ControlValue::Bipolar(SignedUnitValue::new(0.0).unwrap()),
                )
                .unwrap(),
                ControlTile::spanning(
                    Time::new(1, 2),
                    Time::ONE,
                    ControlKey::PitchBend,
                    ControlValue::Bipolar(SignedUnitValue::new(1.0).unwrap()),
                )
                .unwrap(),
            ],
        )
        .unwrap();
        let controlled = Score::with_controls(
            Score::from(sample_voice((0, 1), (1, 1), "pad", 1)),
            ControlScore::from(controls),
        );
        let degraded = Score::degrade(controlled, DegradePolicy::new(Time::new(1, 2), 42));

        let events = evaluate_score(&degraded, &span((0, 1), (1, 1))).unwrap();
        let starts = events
            .iter()
            .filter(|event| matches!(event.kind(), EvaluatedEventKind::StartVoice { .. }))
            .count();
        let updates = events
            .iter()
            .filter(|event| matches!(event.kind(), EvaluatedEventKind::UpdateVoiceControls { .. }))
            .count();

        assert!(
            (starts == 0 && updates == 0) || (starts == 1 && updates > 0),
            "degrade must keep or drop the entire voice lifecycle together; got {starts} starts and {updates} updates"
        );
    }

    #[test]
    fn degrade_group_key_ignores_visible_span() {
        let shifted = Score::shift(
            Score::from(sample_voice((0, 1), (1, 1), "long", 99)),
            Time::new(1, 2),
        );
        let clipped_window = span((1, 1), (2, 1));
        let mut compared = 0;
        for seed in 0..64 {
            let source = Score::degrade(shifted.clone(), DegradePolicy::new(Time::new(1, 2), seed));
            let broad = evaluate_score(&source, &span((0, 1), (2, 1))).unwrap();
            let clipped = evaluate_score(&source, &clipped_window).unwrap();
            // A shifted cyclic source also has an occurrence before phase
            // zero. Compare only lifecycles visible in both query windows.
            let expected: Vec<_> = broad
                .iter()
                .filter(|event| event.projected().whole().intersects(&clipped_window))
                .map(lifecycle_key)
                .collect();
            compared += expected.len();
            assert_eq!(
                expected,
                clipped.iter().map(lifecycle_key).collect::<Vec<_>>(),
                "seed={seed}",
            );
        }
        assert!(compared > 0, "exercise surviving clipped lifecycles");
    }

    #[test]
    fn degrade_is_stable_across_adjacent_split_windows() {
        let source = Score::degrade(
            Score::merge(vec![
                Score::from(sample_voice((0, 1), (1, 4), "kick", 1)),
                Score::shift(
                    Score::from(sample_voice((0, 1), (1, 4), "snare", 2)),
                    Time::ONE,
                ),
                Score::shift(
                    Score::from(sample_voice((0, 1), (1, 4), "hat", 3)),
                    Time::new(2, 1),
                ),
                Score::shift(
                    Score::from(sample_voice((0, 1), (1, 4), "clap", 4)),
                    Time::new(3, 1),
                ),
            ]),
            DegradePolicy::new(Time::new(1, 2), 77),
        );

        let whole = evaluate_score(&source, &span((0, 1), (4, 1))).unwrap();
        let left = evaluate_score(&source, &span((0, 1), (2, 1))).unwrap();
        let right = evaluate_score(&source, &span((2, 1), (4, 1))).unwrap();

        let whole_keys = whole
            .iter()
            .map(lifecycle_key)
            .collect::<std::collections::BTreeSet<_>>();
        let split_keys = left
            .iter()
            .chain(right.iter())
            .map(lifecycle_key)
            .collect::<std::collections::BTreeSet<_>>();

        assert_eq!(whole_keys, split_keys);
    }

    #[test]
    fn degrade_is_stable_for_same_seed_and_window() {
        let score = Score::degrade(
            Score::merge(vec![
                Score::from(sample_voice((0, 1), (1, 4), "kick", 1)),
                Score::shift(
                    Score::from(sample_voice((0, 1), (1, 4), "snare", 2)),
                    Time::ONE,
                ),
                Score::shift(
                    Score::from(sample_voice((0, 1), (1, 4), "hat", 3)),
                    Time::new(2, 1),
                ),
                Score::shift(
                    Score::from(sample_voice((0, 1), (1, 4), "clap", 4)),
                    Time::new(3, 1),
                ),
            ]),
            DegradePolicy::new(Time::new(1, 2), 42),
        );

        let first = evaluate_score(&score, &span((0, 1), (4, 1))).unwrap();
        let second = evaluate_score(&score, &span((0, 1), (4, 1))).unwrap();

        assert_eq!(first, second);
    }

    #[test]
    fn deduplicate_removes_same_span_same_intent_lifecycles() {
        let left = Score::from(sample_voice((0, 1), (1, 4), "kick", 1));
        let right = Score::from(sample_voice((0, 1), (1, 4), "kick", 2));
        let score = Score::deduplicate(
            Score::merge(vec![left, right]),
            DeduplicatePolicy::new(DeduplicateKey::WholeSpanAndIntent, DeduplicateWinner::First),
        );

        let events = evaluate_score(&score, &span((0, 1), (1, 1))).unwrap();
        let groups = group_lifecycles(events);
        assert_eq!(groups.len(), 1);
    }

    #[test]
    fn priority_merge_keeps_higher_priority_lifecycle_on_conflict() {
        let high = Score::from(sample_voice((0, 1), (1, 2), "high", 1));
        let low = Score::from(sample_voice((1, 4), (3, 4), "low", 2));
        let score = Score::priority_merge(
            vec![high, low],
            PriorityMergePolicy::new(ConflictPolicy::WholeSpanOverlap),
        );

        let events = evaluate_score(&score, &span((0, 1), (1, 1))).unwrap();
        let groups = group_lifecycles(events);
        assert_eq!(groups.len(), 1);
        assert!(matches!(
            groups[0].intent(),
            Intent::Sample(sample) if sample.sample_id == "high"
        ));
    }

    #[test]
    fn weighted_choice_is_stable_across_adjacent_split_windows() {
        let kick = Score::from(sample_voice((0, 1), (1, 4), "kick", 1));
        let snare = Score::from(sample_voice((0, 1), (1, 4), "snare", 2));
        let hat = Score::from(sample_voice((0, 1), (1, 4), "hat", 3));
        let score = Score::weighted_choice(
            vec![
                WeightedScore::new(kick, Time::new(7, 1)),
                WeightedScore::new(snare, Time::new(2, 1)),
                WeightedScore::new(hat, Time::new(1, 1)),
            ],
            123,
        );

        let whole = evaluate_score(&score, &span((0, 1), (8, 1))).unwrap();
        let left = evaluate_score(&score, &span((0, 1), (4, 1))).unwrap();
        let right = evaluate_score(&score, &span((4, 1), (8, 1))).unwrap();

        let whole_keys = whole
            .iter()
            .map(lifecycle_key)
            .collect::<std::collections::BTreeSet<_>>();
        let split_keys = left
            .iter()
            .chain(right.iter())
            .map(lifecycle_key)
            .collect::<std::collections::BTreeSet<_>>();

        assert_eq!(whole_keys, split_keys);
    }

    #[test]
    fn mask_clip_limits_visible_span_to_open_gate_controls() {
        let source = Score::from(sample_voice((0, 1), (1, 1), "pad", 1));
        let mask = ControlTrack::new(
            Time::ONE,
            vec![
                ControlTile::spanning(
                    Time::new(1, 4),
                    Time::new(3, 4),
                    ControlKey::Gate,
                    ControlValue::Bool(true),
                )
                .unwrap(),
            ],
        )
        .unwrap();
        let score = Score::mask_clip(source, ControlScore::from(mask));

        let events = evaluate_score(&score, &span((0, 1), (1, 1))).unwrap();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].projected().whole(), span((0, 1), (1, 1)));
        assert_eq!(events[0].projected().visible(), span((1, 4), (3, 4)));
    }

    #[test]
    fn mask_clip_with_gate_never_open_emits_no_events() {
        let source = Score::from(sample_voice((0, 1), (1, 1), "pad", 1));
        let mask = ControlTrack::new(
            Time::ONE,
            vec![
                ControlTile::spanning(
                    Time::ZERO,
                    Time::ONE,
                    ControlKey::Gate,
                    ControlValue::Bool(false),
                )
                .unwrap(),
            ],
        )
        .unwrap();
        let score = Score::mask_clip(source, ControlScore::from(mask));

        let events = evaluate_score(&score, &span((0, 1), (1, 1))).unwrap();
        assert!(events.is_empty());
    }

    #[test]
    fn mask_clip_shorter_than_source_only_clips_while_gate_is_open() {
        let source = Score::from(sample_voice((0, 1), (1, 1), "pad", 1));
        let mask = ControlTrack::new(
            Time::ONE,
            vec![
                ControlTile::spanning(
                    Time::new(1, 4),
                    Time::new(1, 2),
                    ControlKey::Gate,
                    ControlValue::Bool(true),
                )
                .unwrap(),
            ],
        )
        .unwrap();
        let score = Score::mask_clip(source, ControlScore::from(mask));

        let events = evaluate_score(&score, &span((0, 1), (1, 1))).unwrap();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].projected().visible(), span((1, 4), (1, 2)));
    }

    #[test]
    fn mask_clip_longer_than_source_passes_through_while_gate_is_open() {
        let source = Score::from(sample_voice((1, 4), (1, 2), "pad", 1));
        let mask = ControlTrack::new(
            Time::ONE,
            vec![
                ControlTile::spanning(
                    Time::ZERO,
                    Time::ONE,
                    ControlKey::Gate,
                    ControlValue::Bool(true),
                )
                .unwrap(),
            ],
        )
        .unwrap();
        let score = Score::mask_clip(source, ControlScore::from(mask));

        let events = evaluate_score(&score, &span((0, 1), (1, 1))).unwrap();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].projected().visible(), span((1, 4), (1, 2)));
    }

    #[test]
    fn control_mask_clip_limits_visible_span_to_open_gate() {
        let source = ControlScore::from(
            ControlTrack::new(
                Time::ONE,
                vec![
                    ControlTile::spanning(
                        Time::ZERO,
                        Time::ONE,
                        ControlKey::Gain,
                        ControlValue::Scalar(1.0),
                    )
                    .unwrap(),
                ],
            )
            .unwrap(),
        );
        let mask = ControlScore::from(
            ControlTrack::new(
                Time::ONE,
                vec![
                    ControlTile::spanning(
                        Time::new(1, 4),
                        Time::new(3, 4),
                        ControlKey::Gate,
                        ControlValue::Bool(true),
                    )
                    .unwrap(),
                ],
            )
            .unwrap(),
        );
        let controls = evaluate_control_score(
            &ControlScore::mask_clip(source, mask),
            &span((0, 1), (1, 1)),
        )
        .unwrap();
        assert_eq!(controls.len(), 1);
        assert_eq!(controls[0].whole, span((0, 1), (1, 1)));
        assert_eq!(controls[0].visible, span((1, 4), (3, 4)));
        assert_eq!(controls[0].key, ControlKey::Gain);
    }

    #[test]
    fn control_priority_merge_keeps_higher_priority_tile_on_overlap() {
        let high = ControlScore::from(
            ControlTrack::new(
                Time::ONE,
                vec![
                    ControlTile::spanning(
                        Time::ZERO,
                        Time::new(1, 2),
                        ControlKey::Gain,
                        ControlValue::Scalar(1.0),
                    )
                    .unwrap(),
                ],
            )
            .unwrap(),
        );
        let low = ControlScore::from(
            ControlTrack::new(
                Time::ONE,
                vec![
                    ControlTile::spanning(
                        Time::new(1, 4),
                        Time::new(3, 4),
                        ControlKey::Gain,
                        ControlValue::Scalar(0.25),
                    )
                    .unwrap(),
                ],
            )
            .unwrap(),
        );
        let controls = evaluate_control_score(
            &ControlScore::priority_merge(
                vec![high, low],
                PriorityMergePolicy::new(ConflictPolicy::WholeSpanOverlap),
            ),
            &span((0, 1), (1, 1)),
        )
        .unwrap();
        assert_eq!(controls.len(), 1);
        assert!(matches!(
            controls[0].value,
            ControlValue::Scalar(value) if (value - 1.0).abs() < f64::EPSILON
        ));
    }

    #[test]
    fn control_weighted_choice_is_stable_across_adjacent_split_windows() {
        let a = ControlScore::from(
            ControlTrack::new(
                Time::ONE,
                vec![
                    ControlTile::spanning(
                        Time::ZERO,
                        Time::ONE,
                        ControlKey::Gain,
                        ControlValue::Scalar(1.0),
                    )
                    .unwrap(),
                ],
            )
            .unwrap(),
        );
        let b = ControlScore::from(
            ControlTrack::new(
                Time::ONE,
                vec![
                    ControlTile::spanning(
                        Time::ZERO,
                        Time::ONE,
                        ControlKey::Gain,
                        ControlValue::Scalar(0.5),
                    )
                    .unwrap(),
                ],
            )
            .unwrap(),
        );
        let score = ControlScore::weighted_choice(
            vec![
                WeightedControlScore::new(a, Time::new(7, 1)),
                WeightedControlScore::new(b, Time::new(1, 1)),
            ],
            123,
        );

        let whole = evaluate_control_score(&score, &span((0, 1), (8, 1))).unwrap();
        let left = evaluate_control_score(&score, &span((0, 1), (4, 1))).unwrap();
        let right = evaluate_control_score(&score, &span((4, 1), (8, 1))).unwrap();

        let key = |control: &EvaluatedControl| {
            let value = match control.value {
                ControlValue::Scalar(value) => value.to_bits(),
                _ => 0,
            };
            (control.whole.start(), value)
        };
        let whole_keys = whole
            .iter()
            .map(key)
            .collect::<std::collections::BTreeSet<_>>();
        let split_keys = left
            .iter()
            .chain(right.iter())
            .map(key)
            .collect::<std::collections::BTreeSet<_>>();

        assert_eq!(whole_keys, split_keys);
    }

    #[test]
    fn segment_sampled_gain_step_is_visible_as_update_in_later_query_windows() {
        let controls = ControlTrack::new(
            Time::ONE,
            vec![
                ControlTile::spanning(
                    Time::ZERO,
                    Time::new(1, 2),
                    ControlKey::Gain,
                    ControlValue::Scalar(1.0),
                )
                .unwrap(),
                ControlTile::spanning(
                    Time::new(1, 2),
                    Time::ONE,
                    ControlKey::Gain,
                    ControlValue::Scalar(0.5),
                )
                .unwrap(),
            ],
        )
        .unwrap();
        let score = Score::with_controls(
            Score::from(sample_voice((0, 1), (1, 1), "pad", 7)),
            ControlScore::from(controls),
        );

        let first = evaluate_score(&score, &span((0, 1), (1, 2))).unwrap();
        let second = evaluate_score(&score, &span((1, 2), (1, 1))).unwrap();

        assert_eq!(first.len(), 1);
        assert!(matches!(
            first[0].kind(),
            EvaluatedEventKind::StartVoice { .. }
        ));
        assert_eq!(second.len(), 1);
        assert!(matches!(
            second[0].kind(),
            EvaluatedEventKind::UpdateVoiceControls { .. }
        ));
        assert!(matches!(
            second[0].projected().controls().get(&ControlKey::Gain),
            Some(ControlValue::Scalar(value)) if (*value - 0.5).abs() < f64::EPSILON
        ));
    }

    #[test]
    fn aligned_segment_and_runtime_boundaries_emit_one_update_at_step() {
        let controls = ControlTrack::new(
            Time::ONE,
            vec![
                ControlTile::spanning(
                    Time::ZERO,
                    Time::new(1, 2),
                    ControlKey::Gain,
                    ControlValue::Scalar(1.0),
                )
                .unwrap(),
                ControlTile::spanning(
                    Time::new(1, 2),
                    Time::ONE,
                    ControlKey::Gain,
                    ControlValue::Scalar(0.5),
                )
                .unwrap(),
                ControlTile::spanning(
                    Time::ZERO,
                    Time::new(1, 2),
                    ControlKey::PitchBend,
                    ControlValue::Bipolar(SignedUnitValue::new(0.0).unwrap()),
                )
                .unwrap(),
                ControlTile::spanning(
                    Time::new(1, 2),
                    Time::ONE,
                    ControlKey::PitchBend,
                    ControlValue::Bipolar(SignedUnitValue::new(1.0).unwrap()),
                )
                .unwrap(),
            ],
        )
        .unwrap();
        let score = Score::with_controls(
            Score::from(sample_voice((0, 1), (1, 1), "pad", 7)),
            ControlScore::from(controls),
        );

        let evaluated = evaluate_score(&score, &span((0, 1), (1, 1))).unwrap();
        let at_half: Vec<_> = evaluated
            .iter()
            .filter(|event| {
                matches!(event.kind(), EvaluatedEventKind::UpdateVoiceControls { .. })
                    && event.projected().visible().start() == Time::new(1, 2)
            })
            .collect();

        assert_eq!(at_half.len(), 1);
        assert!(matches!(
            at_half[0].projected().controls().get(&ControlKey::Gain),
            Some(ControlValue::Scalar(value)) if (*value - 0.5).abs() < f64::EPSILON
        ));
        assert!(matches!(
            at_half[0].projected().controls().get(&ControlKey::PitchBend),
            Some(ControlValue::Bipolar(value)) if value.value() == 1.0
        ));
    }

    #[test]
    fn segment_sampled_gain_ramp_emits_one_start_and_two_updates() {
        let controls = ControlTrack::new(
            Time::ONE,
            vec![
                ControlTile::spanning(
                    Time::ZERO,
                    Time::new(1, 3),
                    ControlKey::Gain,
                    ControlValue::Scalar(1.0),
                )
                .unwrap(),
                ControlTile::spanning(
                    Time::new(1, 3),
                    Time::new(2, 3),
                    ControlKey::Gain,
                    ControlValue::Scalar(0.5),
                )
                .unwrap(),
                ControlTile::spanning(
                    Time::new(2, 3),
                    Time::ONE,
                    ControlKey::Gain,
                    ControlValue::Scalar(0.25),
                )
                .unwrap(),
            ],
        )
        .unwrap();
        let score = Score::with_controls(
            Score::from(sample_voice((0, 1), (1, 1), "pad", 7)),
            ControlScore::from(controls),
        );

        let evaluated = evaluate_score(&score, &span((0, 1), (1, 1))).unwrap();
        let starts = evaluated
            .iter()
            .filter(|event| matches!(event.kind(), EvaluatedEventKind::StartVoice { .. }))
            .count();
        let updates = evaluated
            .iter()
            .filter(|event| matches!(event.kind(), EvaluatedEventKind::UpdateVoiceControls { .. }))
            .count();
        let voice_ids = evaluated
            .iter()
            .map(|event| match event.kind() {
                EvaluatedEventKind::StartVoice { voice_id, .. }
                | EvaluatedEventKind::UpdateVoiceControls { voice_id, .. } => *voice_id,
            })
            .collect::<Vec<_>>();

        assert_eq!(starts, 1);
        assert_eq!(updates, 2);
        assert!(voice_ids.windows(2).all(|window| window[0] == window[1]));
    }

    fn sample_ids_in_window(score: &Score, start: (i64, i64), end: (i64, i64)) -> Vec<String> {
        evaluate_score(score, &span(start, end))
            .unwrap()
            .into_iter()
            .filter(|event| matches!(event.kind(), EvaluatedEventKind::StartVoice { .. }))
            .filter_map(|event| match event.projected().intent() {
                Intent::Sample(sample) => Some(sample.sample_id.clone()),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn concat_plays_equal_one_cycle_children_in_sequence() {
        let score = Score::concat(vec![
            sample_event_score((0, 1), (1, 1), "a"),
            sample_event_score((0, 1), (1, 1), "b"),
        ]);
        assert!(matches!(score.kind(), ScoreKind::Concat(_)));
        assert_eq!(sample_ids_in_window(&score, (0, 1), (1, 1)), vec!["a"]);
        assert_eq!(sample_ids_in_window(&score, (1, 1), (2, 1)), vec!["b"]);
    }

    #[test]
    fn concat_unequal_duration_children_use_cumulative_offsets() {
        let slow_a = Score::time_scale(sample_event_score((0, 1), (1, 1), "a"), Time::new(1, 3));
        let score = Score::concat(vec![slow_a, sample_event_score((0, 1), (1, 1), "b")]);
        assert_eq!(sample_ids_in_window(&score, (0, 1), (3, 1)), vec!["a"]);
        assert_eq!(sample_ids_in_window(&score, (1, 1), (2, 1)), vec!["a"]);
        assert_eq!(sample_ids_in_window(&score, (3, 1), (4, 1)), vec!["b"]);
        let fast_a = Score::time_scale(sample_event_score((0, 1), (1, 1), "a"), Time::new(3, 1));
        let score = Score::concat(vec![fast_a, sample_event_score((0, 1), (1, 1), "b")]);
        let events = evaluate_score(&score, &span((0, 1), (4, 3))).unwrap();
        assert_eq!(events.len(), 2);
        assert_eq!(events[0].projected().whole(), span((0, 1), (1, 3)));
        assert_eq!(events[1].projected().whole(), span((1, 3), (4, 3)));
    }

    #[test]
    fn nested_concat_offsets_by_segment_duration() {
        let four_a = Score::concat(vec![
            sample_event_score((0, 1), (1, 1), "a"),
            sample_event_score((0, 1), (1, 1), "a"),
            sample_event_score((0, 1), (1, 1), "a"),
            sample_event_score((0, 1), (1, 1), "a"),
        ]);
        let score = Score::concat(vec![
            four_a,
            Score::concat(vec![
                sample_event_score((0, 1), (1, 1), "b"),
                sample_event_score((0, 1), (1, 1), "b"),
            ]),
        ]);
        assert_eq!(sample_ids_in_window(&score, (0, 1), (4, 1)), vec!["a"; 4]);
        assert_eq!(sample_ids_in_window(&score, (4, 1), (5, 1)), vec!["b"]);
        assert_eq!(sample_ids_in_window(&score, (5, 1), (6, 1)), vec!["b"]);
    }

    #[test]
    fn concat_normalizes_non_origin_child_before_sequencing() {
        let score = Score::concat(vec![
            sample_event_score((2, 1), (3, 1), "a"),
            sample_event_score((0, 1), (1, 1), "b"),
        ]);
        assert_eq!(sample_ids_in_window(&score, (0, 1), (1, 1)), vec!["a"]);
        assert_eq!(sample_ids_in_window(&score, (1, 1), (2, 1)), vec!["b"]);
    }

    #[test]
    fn concat_after_merge_preserves_the_full_span_of_offset_children() {
        let score = Score::concat(vec![
            Score::merge(vec![
                sample_event_score((2, 1), (5, 2), "a"),
                sample_event_score((5, 2), (3, 1), "b"),
            ]),
            sample_event_score((0, 1), (1, 1), "c"),
        ]);
        let events = evaluate_score(&score, &span((0, 1), (2, 1))).unwrap();
        assert_eq!(events.len(), 3);
        assert_eq!(events[0].projected().whole(), span((0, 1), (1, 2)));
        assert_eq!(events[1].projected().whole(), span((1, 2), (1, 1)));
        assert_eq!(events[2].projected().whole(), span((1, 1), (2, 1)));
    }
}
