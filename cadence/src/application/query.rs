use std::hash::{Hash, Hasher};

use crate::application::audio::VoiceInstanceId;
use crate::domain::{
    control::{
        ControlKey, ControlMap, ControlMerge, ControlModelError, ControlTile, ControlTiming,
        ControlTrack, ControlValue, SignedUnitValue, UnitValue,
    },
    intent::Intent,
    moment::Moment,
    mosaic::Mosaic,
    prelude::Time,
    projection::ProjectedMoment,
    score::{
        ConflictPolicy, ControlScore, ControlScoreKind, ControlScoreNodeId, DeduplicateKey,
        WeightedControlScore,
        DeduplicatePolicy, DeduplicateWinner, DegradePolicy, PriorityMergePolicy, Score, ScoreKind,
        ScoreNodeId, WeightedScore,
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
        let kind = match self.kind {
            EvaluatedEventKind::StartVoice { .. } => {
                let voice_whole = projected.whole();
                EvaluatedEventKind::StartVoice {
                    voice_whole,
                    voice_id: voice_instance_id(key, voice_whole),
                }
            }
            EvaluatedEventKind::UpdateVoiceControls {
                voice_whole,
                voice_id,
            } => EvaluatedEventKind::UpdateVoiceControls {
                voice_whole,
                voice_id,
            },
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
        ScoreKind::Voice(voice) => Ok(evaluate_voice_leaf(score.id(), voice, window)),
        ScoreKind::Mosaic(mosaic) => Ok(evaluate_mosaic_leaf(score.id(), mosaic, window)),
        ScoreKind::Merge(children) => {
            let mut events = Vec::new();
            for child in children {
                events.extend(evaluate_score(child, window)?);
            }
            Ok(events)
        }
        ScoreKind::Concat(children) => evaluate_score_concat(score.id(), children, window),
        ScoreKind::CycleRoute(children) => evaluate_score_cycle_route(children, window),
        ScoreKind::CycleSlots(children) => evaluate_score_cycle_slots(children, window),
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
            evaluate_control_score(mask, window),
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
        match event.kind() {
            EvaluatedEventKind::StartVoice { .. } => {
                assert!(
                    !group_index.contains_key(&key),
                    "each lifecycle group must contain exactly one start event"
                );
                let index = groups.len();
                group_index.insert(key, index);
                groups.push(LifecycleGroup {
                    key,
                    events: vec![event],
                });
            }
            EvaluatedEventKind::UpdateVoiceControls { .. } => {
                let index = group_index
                    .get(&key)
                    .copied()
                    .expect("runtime control updates must follow an existing start event");
                groups[index].events.push(event);
            }
        }
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
            .filter(|group| lifecycle_survives_degrade(group.key, policy))
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

    let roll = seeded_roll_below(seed, stable_hash(&cycle_index), total_weight);
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
) -> Vec<EvaluatedControl> {
    let mut accepted = Vec::<EvaluatedControl>::new();
    for child in children {
        for candidate in evaluate_control_score(child, window) {
            if !accepted
                .iter()
                .any(|existing| control_tiles_conflict(existing, &candidate, policy.conflict()))
            {
                accepted.push(candidate);
            }
        }
    }
    accepted
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
        ConflictPolicy::WholeSpanOverlap => left.whole.intersects(&right.whole),
    }
}

fn evaluate_control_weighted_choice(
    options: &[WeightedControlScore],
    seed: u64,
    window: &TransportSpan,
) -> Vec<EvaluatedControl> {
    if options.is_empty() {
        return Vec::new();
    }

    let mut controls = Vec::new();
    for cycle_window in split_by_cycle(window) {
        let cycle_index = cycle_window.start().floor();
        let Some(selected) = choose_weighted_control_option(options, seed, cycle_index) else {
            continue;
        };
        controls.extend(evaluate_control_score(selected.score(), &cycle_window));
    }
    controls
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

    let roll = seeded_roll_below(seed, stable_hash(&cycle_index), total_weight);
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

fn seeded_roll_below(seed: u64, key: u64, upper_bound: Time) -> Time {
    seeded_roll_unit(seed, key) * upper_bound
}

fn splitmix64(mut value: u64) -> u64 {
    value = value.wrapping_add(0x9E37_79B9_7F4A_7C15);
    value = (value ^ (value >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    value = (value ^ (value >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    value ^ (value >> 31)
}

#[must_use]
fn evaluate_control_score(score: &ControlScore, window: &TransportSpan) -> Vec<EvaluatedControl> {
    let mut controls = evaluate_control_score_unsorted(score, window);
    sort_controls(&mut controls);
    controls
}

#[must_use]
fn evaluate_control_score_unsorted(
    score: &ControlScore,
    window: &TransportSpan,
) -> Vec<EvaluatedControl> {
    match score.kind() {
        ControlScoreKind::Track(track) => evaluate_control_track_leaf(score.id(), track, window),
        ControlScoreKind::Merge(children) => children
            .iter()
            .flat_map(|child| evaluate_control_score(child, window))
            .collect(),
        ControlScoreKind::Concat(children) => {
            evaluate_control_score_concat(score.id(), children, window)
        }
        ControlScoreKind::CycleRoute(children) => evaluate_control_cycle_route(children, window),
        ControlScoreKind::CycleSlots(children) => evaluate_control_cycle_slots(children, window),
        ControlScoreKind::TimeScale { inner, rate } => {
            let scaled_window =
                TransportSpan::new(window.start() * *rate, window.end() * *rate).unwrap();

            evaluate_control_score(inner, &scaled_window)
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

            evaluate_control_score(inner, &shifted_window)
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
            evaluate_reflected_controls(score.id(), inner, window)
        }
        ControlScoreKind::PriorityMerge { children, policy } => {
            evaluate_control_priority_merge(children, window, *policy)
        }
        ControlScoreKind::WeightedChoice { options, seed } => {
            evaluate_control_weighted_choice(options, *seed, window)
        }
        ControlScoreKind::MaskClip { source, mask } => mask_clip_controls(
            score.id(),
            evaluate_control_score(source, window),
            evaluate_control_score(mask, window),
        ),
    }
}

fn score_sequence_origin(score: &Score) -> Time {
    match score.kind() {
        ScoreKind::Voice(voice) => voice
            .tiles()
            .iter()
            .map(|tile| tile.phase().start())
            .min()
            .unwrap_or(Time::ZERO),
        ScoreKind::Mosaic(mosaic) => mosaic
            .bounds()
            .map(|bounds| bounds.start())
            .unwrap_or(Time::ZERO),
        ScoreKind::Concat(_) => Time::ZERO,
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
        ScoreKind::Voice(voice) => voice.period(),
        ScoreKind::Mosaic(mosaic) => mosaic
            .bounds()
            .map(|bounds| bounds.end() - bounds.start())
            .unwrap_or(Time::ZERO),
        ScoreKind::Concat(children) => children
            .iter()
            .map(score_sequencing_extent)
            .fold(Time::ZERO, |total, extent| total + extent),
        ScoreKind::Merge(children)
        | ScoreKind::CycleRoute(children)
        | ScoreKind::CycleSlots(children) => children
            .iter()
            .map(score_sequencing_extent)
            .max()
            .unwrap_or(Time::ZERO),
        ScoreKind::TimeScale { inner, rate } => score_sequencing_extent(inner) * *rate,
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
        ControlScoreKind::TimeScale { inner, rate } => control_sequencing_extent(inner) * *rate,
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
) -> Vec<EvaluatedControl> {
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
                    evaluate_control_score(child, &shifted_window)
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
    controls
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
) -> Vec<EvaluatedControl> {
    if children.is_empty() {
        return Vec::new();
    }

    split_by_cycle(window)
        .into_iter()
        .flat_map(|cycle_window| {
            let cycle_index = cycle_window
                .start()
                .floor()
                .rem_euclid(children.len() as i64);
            evaluate_control_score(&children[cycle_index as usize], &cycle_window)
        })
        .collect()
}

fn evaluate_score_cycle_slots(
    children: &[Score],
    window: &TransportSpan,
) -> Result<Vec<EvaluatedEvent>, ControlModelError> {
    if children.is_empty() {
        return Ok(Vec::new());
    }

    let slot_width = Time::ONE / Time::whole_number(children.len() as i64);
    let mut events = Vec::new();

    for cycle_window in split_by_cycle(window) {
        let cycle_start = cycle_start(cycle_window.start());

        for (index, child) in children.iter().enumerate() {
            let slot_start = cycle_start + slot_width * Time::whole_number(index as i64);
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
) -> Vec<EvaluatedControl> {
    if children.is_empty() {
        return Vec::new();
    }

    let slot_width = Time::ONE / Time::whole_number(children.len() as i64);
    let mut controls = Vec::new();

    for cycle_window in split_by_cycle(window) {
        let cycle_start = cycle_start(cycle_window.start());

        for (index, child) in children.iter().enumerate() {
            let slot_start = cycle_start + slot_width * Time::whole_number(index as i64);
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
                evaluate_control_score(child, &inner_window)
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

    controls
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
) -> Vec<EvaluatedControl> {
    let mut controls = Vec::new();

    for cycle_window in split_by_cycle(window) {
        let cycle = cycle_span(cycle_window.start());
        let reflected_window = reflect_span(cycle_window, cycle);

        controls.extend(
            evaluate_control_score(inner, &reflected_window)
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

    controls
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

    let first_repetition = if effective_window.start() <= Time::ZERO {
        0
    } else {
        (effective_window.start() / voice.period()).floor() as u64
    };

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

    let first_repetition = if effective_window.start() <= Time::ZERO {
        0
    } else {
        (effective_window.start() / track.period()).floor() as u64
    };

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

fn evaluate_mosaic_leaf(
    origin: ScoreNodeId,
    mosaic: &Mosaic,
    window: &TransportSpan,
) -> Vec<EvaluatedEvent> {
    mosaic
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
    let control_spans = evaluate_control_score(controls, window);
    let mut resolved = Vec::new();

    for event in source_events {
        let overlapping_controls = control_spans
            .iter()
            .filter(|control| control.whole.intersects(&event.projected().whole()))
            .collect::<Vec<_>>();
        let onset_controls = overlapping_controls
            .iter()
            .copied()
            .filter(|control| control.key.spec().timing() == ControlTiming::Onset)
            .collect::<Vec<_>>();
        let lifecycle_controls = overlapping_controls
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

                resolved.extend(runtime_update_events(
                    owner,
                    &event,
                    lifecycle_whole,
                    entry_time,
                    voice_id,
                    &runtime_controls,
                    &segment_boundaries,
                    window,
                )?);
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
        merge_into_control_map(controls_map, control.key.clone(), control.value.clone())?;
    }

    Ok(())
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
        merge_into_control_map(
            &mut controls_map,
            control.key.clone(),
            control.value.clone(),
        )?;
    }

    Ok(controls_map)
}

fn runtime_update_events(
    owner: ScoreNodeId,
    event: &EvaluatedEvent,
    voice_whole: TransportSpan,
    entry_time: Time,
    voice_id: VoiceInstanceId,
    runtime_controls: &[&EvaluatedControl],
    segment_boundaries: &[Time],
    window: &TransportSpan,
) -> Result<Vec<EvaluatedEvent>, ControlModelError> {
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

        let projected = remap_projected(event.projected(), voice_whole, visible, snapshot.clone());
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

fn repeat_is_active(repeat: Repeat, period: Time, repetition: u64, window_end: Time) -> bool {
    match repeat {
        Repeat::Forever => true,
        Repeat::Once => repetition == 0,
        Repeat::Count(count) => repetition < u64::from(count),
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

fn voice_offset(repetition: u64, period: Time) -> Time {
    period * Time::whole_number(repetition as i64)
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
    TransportSpan::new(
        slot_start + (span.start() - cycle_start) * slot_width,
        slot_start + (span.end() - cycle_start) * slot_width,
    )
    .unwrap()
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
        projection::ProjectedMosaic,
        voice::Tile,
    };

    fn span(start: (i64, i64), end: (i64, i64)) -> TransportSpan {
        TransportSpan::new(Time::new(start.0, start.1), Time::new(end.0, end.1)).unwrap()
    }

    fn sample_mosaic_score(start: (i64, i64), end: (i64, i64), sample: &str) -> Score {
        Score::mosaic(Mosaic::new(vec![
            Moment::spanning(
                Time::new(start.0, start.1),
                Time::new(end.0, end.1),
                Intent::sample(sample),
            )
            .unwrap(),
        ]))
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
        let projected = ProjectedMosaic::new(
            evaluate_score(&score, &span((0, 1), (1, 1)))
                .unwrap()
                .into_iter()
                .map(EvaluatedEvent::into_projected)
                .collect(),
        );

        assert_eq!(projected.len(), 2);
        assert_eq!(projected.moments()[0].intent(), &Intent::sample("kick"));
    }

    #[test]
    fn with_controls_slices_source_segments_and_attaches_control_map() {
        let score = Score::with_controls(
            Score::from(sample_voice((0, 1), (1, 1), "pad", 7)),
            ControlScore::from(gain_ramp_track((0, 1), (1, 2), 1.0, 0.0)),
        );
        let evaluated = evaluate_score(&score, &span((0, 1), (1, 1))).unwrap();
        let projected = ProjectedMosaic::new(
            evaluated
                .iter()
                .filter(|event| matches!(event.kind(), EvaluatedEventKind::StartVoice { .. }))
                .cloned()
                .map(EvaluatedEvent::into_projected)
                .collect(),
        );

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
    fn projected_mosaic_from_score_stays_thin() {
        let projected = ProjectedMosaic::new(
            evaluate_score(
                &Score::with_controls(
                    Score::from(sample_voice((0, 1), (1, 1), "pad", 7)),
                    ControlScore::from(gain_ramp_track((0, 1), (1, 1), 1.0, 0.0)),
                ),
                &span((0, 1), (1, 1)),
            )
            .unwrap()
            .into_iter()
            .map(EvaluatedEvent::into_projected)
            .collect(),
        );

        let mosaic = projected.as_mosaic();
        assert_eq!(mosaic.len(), projected.len());
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
        let projected = ProjectedMosaic::new(
            evaluate_score(&score, &span((0, 1), (1, 1)))
                .unwrap()
                .into_iter()
                .map(EvaluatedEvent::into_projected)
                .collect(),
        );

        assert_eq!(projected.len(), 3);
        assert!(matches!(
            projected.moments()[1].controls().get(&ControlKey::Gain),
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
        let projected = ProjectedMosaic::new(
            evaluate_score(&score, &span((0, 1), (1, 1)))
                .unwrap()
                .into_iter()
                .map(EvaluatedEvent::into_projected)
                .collect(),
        );

        assert!(matches!(
            projected.moments()[0].controls().get(&ControlKey::Gain),
            Some(ControlValue::Ramp { from, to }) if *from <= *to
        ));
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
        let source = Score::degrade(
            Score::shift(
                Score::from(sample_voice((0, 1), (1, 1), "long", 99)),
                Time::new(1, 2),
            ),
            DegradePolicy::new(Time::new(1, 2), 123),
        );

        let broad = evaluate_score(&source, &span((0, 1), (2, 1))).unwrap();
        let clipped = evaluate_score(&source, &span((1, 1), (2, 1))).unwrap();

        assert_eq!(broad.is_empty(), clipped.is_empty());
        if !broad.is_empty() {
            assert_eq!(lifecycle_key(&broad[0]), lifecycle_key(&clipped[0]));
        }
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
        let controls =
            evaluate_control_score(&ControlScore::mask_clip(source, mask), &span((0, 1), (1, 1)));
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
        );
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

        let whole = evaluate_control_score(&score, &span((0, 1), (8, 1)));
        let left = evaluate_control_score(&score, &span((0, 1), (4, 1)));
        let right = evaluate_control_score(&score, &span((4, 1), (8, 1)));

        let key = |control: &EvaluatedControl| {
            let value = match control.value {
                ControlValue::Scalar(value) => value.to_bits(),
                _ => 0,
            };
            (control.whole.start(), value)
        };
        let whole_keys = whole.iter().map(key).collect::<std::collections::BTreeSet<_>>();
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
            sample_mosaic_score((0, 1), (1, 1), "a"),
            sample_mosaic_score((0, 1), (1, 1), "b"),
        ]);
        assert!(matches!(score.kind(), ScoreKind::Concat(_)));
        assert_eq!(sample_ids_in_window(&score, (0, 1), (1, 1)), vec!["a"]);
        assert_eq!(sample_ids_in_window(&score, (1, 1), (2, 1)), vec!["b"]);
    }

    #[test]
    fn concat_unequal_duration_children_use_cumulative_offsets() {
        let slow_a = Score::time_scale(sample_mosaic_score((0, 1), (1, 1), "a"), Time::new(3, 1));
        let score = Score::concat(vec![slow_a, sample_mosaic_score((0, 1), (1, 1), "b")]);
        assert_eq!(sample_ids_in_window(&score, (0, 1), (3, 1)), vec!["a"]);
        assert_eq!(sample_ids_in_window(&score, (3, 1), (4, 1)), vec!["b"]);
        assert!(sample_ids_in_window(&score, (1, 1), (2, 1)).is_empty());
    }

    #[test]
    fn nested_concat_offsets_by_segment_duration() {
        let four_a = Score::concat(vec![
            sample_mosaic_score((0, 1), (1, 1), "a"),
            sample_mosaic_score((0, 1), (1, 1), "a"),
            sample_mosaic_score((0, 1), (1, 1), "a"),
            sample_mosaic_score((0, 1), (1, 1), "a"),
        ]);
        let score = Score::concat(vec![
            four_a,
            Score::concat(vec![
                sample_mosaic_score((0, 1), (1, 1), "b"),
                sample_mosaic_score((0, 1), (1, 1), "b"),
            ]),
        ]);
        assert_eq!(sample_ids_in_window(&score, (0, 1), (4, 1)), vec!["a"; 4]);
        assert_eq!(sample_ids_in_window(&score, (4, 1), (5, 1)), vec!["b"]);
        assert_eq!(sample_ids_in_window(&score, (5, 1), (6, 1)), vec!["b"]);
    }

    #[test]
    fn concat_normalizes_non_origin_child_before_sequencing() {
        let score = Score::concat(vec![
            sample_mosaic_score((2, 1), (3, 1), "a"),
            sample_mosaic_score((0, 1), (1, 1), "b"),
        ]);
        assert_eq!(sample_ids_in_window(&score, (0, 1), (1, 1)), vec!["a"]);
        assert_eq!(sample_ids_in_window(&score, (1, 1), (2, 1)), vec!["b"]);
    }
}
