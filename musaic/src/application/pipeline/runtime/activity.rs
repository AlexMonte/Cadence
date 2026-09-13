//! Musical activity at the audio clock, independent of the editor's future preview.
//!
//! This is score/gate activity, not a PCM meter: effect tails have no authored note
//! boundary. Projection is cached per accepted revision and cycle; only the small
//! active set changes each frame. Pending or rejected edits never supply routes.
use std::collections::{BTreeMap, BTreeSet};

use bevy::prelude::*;
use cadence::{
    application::{EvaluatedEvent, EvaluatedEventKind, audio::VoiceInstanceId},
    infrastructure::playback::{PlaybackRuntime, PlaybackState},
    prelude::{
        CadenceCompiler, ControlKey, ControlMap, ControlValue, Intent, Span, Time as CycleTime,
    },
};
use tessera::prelude::NodeId;

use super::{CompiledProject, RuntimeState};
use crate::{
    application::session::MusaicProject,
    domain::document::{DocumentQueries, MusaicDocument},
};

/// Routing is captured with the score rather than inferred from unaccepted edits.
#[derive(Debug, Clone, Default)]
pub struct ActivityRouting {
    parents: BTreeMap<NodeId, NodeId>,
    edges: BTreeSet<(NodeId, NodeId)>,
    control_edges: BTreeSet<(NodeId, NodeId)>,
}

impl ActivityRouting {
    pub fn from_document(document: &MusaicDocument) -> Self {
        let parents = document
            .graph
            .nodes()
            .filter_map(|node| {
                let location = document.graph.location_of(&node.id)?;
                Some((
                    node.id.clone(),
                    document
                        .graph
                        .container_node_for_surface(location.surface)?,
                ))
            })
            .collect();
        let edges = DocumentQueries::new(document)
            .connections_on_surface(document.root_surface)
            .into_iter()
            .map(|edge| (edge.from, edge.to))
            .collect();
        let Ok(program) = crate::domain::document::export_document_program(document) else {
            return Self {
                parents,
                edges,
                control_edges: BTreeSet::new(),
            };
        };
        let control_edges =
            crate::domain::document::connection_policy::endpoint_connections(&program)
                .into_iter()
                .filter(|edge| {
                    use tessera::prelude::{InputEndpoint, NodeInputRole, RootSurfaceNodeKind};
                    let signature = match program.root_surface.nodes.get(&edge.to) {
                        Some(RootSurfaceNodeKind::Transform(node)) => &node.signature,
                        Some(RootSurfaceNodeKind::FlowControl(node)) => &node.signature,
                        _ => return false,
                    };
                    let InputEndpoint::Socket(port) = &edge.input else {
                        return false;
                    };
                    signature.input_socket(port).is_some_and(|socket| {
                        matches!(
                            socket.role,
                            NodeInputRole::Aux
                                | NodeInputRole::Control
                                | NodeInputRole::Mask
                                | NodeInputRole::Route
                                | NodeInputRole::Sidechain
                        )
                    })
                })
                .map(|edge| (edge.from, edge.to))
                .collect();
        Self {
            parents,
            edges,
            control_edges,
        }
    }

    fn add_route(
        &self,
        source: &NodeId,
        output: &str,
        level: f32,
        activity: &mut PlaybackActivity,
    ) {
        let mut root = source.clone();
        let mut seen = BTreeSet::new();
        while seen.insert(root.clone()) {
            activity.add_node(root.clone(), level);
            let Some(parent) = self.parents.get(&root) else {
                break;
            };
            root = parent.clone();
        }
        let target = NodeId::new(output);
        // Intersect forward reachability with the destination's ancestors. A
        // source used by two outputs must not illuminate a muted destination.
        let mut reaches_output = BTreeSet::from([target.clone()]);
        loop {
            let before = reaches_output.len();
            for (from, to) in &self.edges {
                if reaches_output.contains(to) {
                    reaches_output.insert(from.clone());
                }
            }
            if reaches_output.len() == before {
                break;
            }
        }
        let mut frontier = vec![root];
        let mut visited = BTreeSet::new();
        while let Some(node) = frontier.pop() {
            if !visited.insert(node.clone()) {
                continue;
            }
            for (from, to) in &self.edges {
                if from == &node && reaches_output.contains(to) {
                    activity.add_node(to.clone(), level);
                    activity.connections.insert((from.clone(), to.clone()));
                    frontier.push(to.clone());
                }
            }
        }
        // Value/control inputs are in use while the musical route they control
        // is active. Do not mark unchosen musical input branches as sounding.
        let mut controls: Vec<_> = self
            .control_edges
            .iter()
            .filter(|(_, to)| visited.contains(to))
            .map(|(from, to)| (from.clone(), to.clone()))
            .collect();
        let mut seen_controls = BTreeSet::new();
        while let Some((from, to)) = controls.pop() {
            activity.connections.insert((from.clone(), to));
            activity.add_node(from.clone(), level);
            if seen_controls.insert(from.clone()) {
                controls.extend(self.edges.iter().filter(|(_, to)| to == &from).cloned());
            }
        }
    }
}

#[derive(Resource, Debug, Clone, Default)]
pub struct PlaybackActivity {
    /// Actual playback position, never the scheduler's look-ahead position.
    pub cycle: f64,
    pub playing: bool,
    /// Stable authored identities (notes, containing tiles and active route).
    pub nodes: BTreeMap<NodeId, f32>,
    pub connections: BTreeSet<(NodeId, NodeId)>,
}
impl PlaybackActivity {
    fn add_node(&mut self, node: NodeId, level: f32) {
        self.nodes
            .entry(node)
            .and_modify(|old| *old = old.max(level))
            .or_insert(level);
    }
}

#[derive(Resource, Default)]
pub(super) struct ActivityCache {
    revision: Option<u64>,
    cycle: Option<i64>,
    session_start: Option<CycleTime>,
    last_position: Option<CycleTime>,
    holds: BTreeMap<(String, VoiceInstanceId), HeldActivity>,
    events: Vec<(String, EvaluatedEvent)>,
    routes: BTreeMap<(String, NodeId), PlaybackActivity>,
}

/// The renderer keeps at most 128 song voices. Retaining the newest lifecycle
/// starts also bounds visual state for arbitrarily large legato/clip ratios.
const MAX_ACTIVITY_HOLDS: usize = 128;

struct HeldActivity {
    event: EvaluatedEvent,
    gate_end: f64,
    priority: (CycleTime, bool),
}

impl ActivityCache {
    fn reset_at(&mut self, position: CycleTime) {
        self.cycle = None;
        self.events.clear();
        self.holds.clear();
        self.session_start = Some(position);
        self.last_position = None;
    }
}

/// Run before transport commands are consumed: seeking cancels audio voices,
/// including holds whose nominal note span has already ended. Cadence only
/// backfills a note if its nominal span intersects the new playback position.
pub(super) fn reset_activity_for_transport_changes(
    playback: NonSend<PlaybackRuntime>,
    runtime: Res<RuntimeState>,
    project: Res<MusaicProject>,
    mut cache: ResMut<ActivityCache>,
) {
    let status = playback.status();
    if let Some(request) = runtime.transport_request {
        let position = match request {
            super::TransportRequest::Seek(position) => position,
            super::TransportRequest::Stop | super::TransportRequest::Panic => CycleTime::ZERO,
        };
        cache.reset_at(position);
    } else if status.state != PlaybackState::Playing
        || status.cps != project.document.playback.cycles_per_second()
    {
        cache.reset_at(status.cycle_position);
    }
}

fn ingest_activity(
    holds: &mut BTreeMap<(String, VoiceInstanceId), HeldActivity>,
    events: &[(String, EvaluatedEvent)],
    position: CycleTime,
    session_start: CycleTime,
) {
    holds.retain(|_, hold| hold.gate_end > position.value());
    // Reports contain an onset snapshot plus runtime boundaries for a voice.
    // Ingest only boundaries already reached by the device, then retain the
    // latest snapshot beyond the nominal span until its original gate ends.
    let mut due: Vec<_> = events
        .iter()
        .filter(|(_, event)| {
            event.projected().visible().start() <= position
                && voice_whole(event).end() > session_start
        })
        .collect();
    due.sort_by_key(|(_, event)| {
        (
            event.projected().visible().start(),
            matches!(event.kind(), EvaluatedEventKind::UpdateVoiceControls { .. }),
        )
    });
    for (output, event) in due {
        let (voice_id, update) = match event.kind() {
            EvaluatedEventKind::StartVoice { voice_id, .. } => (*voice_id, false),
            EvaluatedEventKind::UpdateVoiceControls { voice_id, .. } => (*voice_id, true),
        };
        let key = (output.clone(), voice_id);
        let priority = (event.projected().visible().start(), update);
        if let Some(held) = holds.get_mut(&key) {
            if priority >= held.priority {
                held.event = event.clone();
                held.priority = priority;
            }
        } else {
            let end = gate_end(event);
            if end <= position.value() {
                continue;
            }
            holds.insert(
                key,
                HeldActivity {
                    event: event.clone(),
                    gate_end: end,
                    priority,
                },
            );
        }
    }
    while holds.len() > MAX_ACTIVITY_HOLDS {
        let oldest = holds
            .iter()
            .min_by_key(|(_, hold)| voice_whole(&hold.event).start())
            .map(|(key, _)| key.clone())
            .unwrap();
        holds.remove(&oldest);
    }
}

fn voice_whole(event: &EvaluatedEvent) -> Span {
    match event.kind() {
        EvaluatedEventKind::StartVoice { voice_whole, .. }
        | EvaluatedEventKind::UpdateVoiceControls { voice_whole, .. } => *voice_whole,
    }
}

fn event_controls(event: &EvaluatedEvent) -> Option<ControlMap> {
    let projected = event.projected();
    let mut controls = match projected.intent() {
        Intent::Sample(sample) => sample.default_controls(),
        Intent::Synth(synth) => synth.default_controls(),
        _ => return None,
    };
    controls.extend(projected.controls().clone());
    Some(controls)
}

fn gate_end(event: &EvaluatedEvent) -> f64 {
    let whole = voice_whole(event);
    let Some(controls) = event_controls(event) else {
        return whole.start().value();
    };
    // Legato and clip length are launch controls, matching Cadence's
    // lifecycle_from_controls; runtime gain/gate updates cannot resize them.
    let clip = scalar(
        &controls,
        ControlKey::ClipLength,
        1.0,
        whole.start().value(),
        0.0,
    );
    let legato = scalar(
        &controls,
        ControlKey::Legato,
        1.0,
        whole.start().value(),
        0.0,
    );
    whole.start().value() + (whole.end() - whole.start()).value() * clip * legato
}

pub(super) fn update_playback_activity(
    playback: NonSend<PlaybackRuntime>,
    sync: Res<cadence::bevy::PlaybackSync>,
    runtime: Res<RuntimeState>,
    project: Res<MusaicProject>,
    mut cache: ResMut<ActivityCache>,
    mut activity: ResMut<PlaybackActivity>,
) {
    let status = playback.status();
    let Some(compiled) = runtime
        .compiled
        .as_ref()
        .filter(|_| status.state == PlaybackState::Playing)
    else {
        if activity.playing || !activity.nodes.is_empty() {
            *activity = PlaybackActivity::default();
        }
        // A resume or stopped seek must reconstruct the current cycle, even if
        // it lands in the same numeric cycle as the previous playback session.
        cache.reset_at(status.cycle_position);
        return;
    };
    let position = status.cycle_position;
    let cycle = position.value().floor() as i64;
    if cache.revision != Some(sync.last_applied_revision) {
        let entry = if cache.revision.is_some() {
            CycleTime::new(cycle, 1)
        } else {
            cache.session_start.unwrap_or(position)
        };
        cache.reset_at(entry);
        cache.routes.clear();
        cache.revision = Some(sync.last_applied_revision);
    }
    // Cadence drops stale scheduled work after a stall beyond lookahead. Match
    // its bounded current-window reconstruction, never replay an unbounded past.
    let lookahead = cadence::infrastructure::playback::PlaybackSettings::default().look_ahead;
    if cache
        .last_position
        .is_some_and(|last| position < last || position - last > lookahead)
    {
        cache.reset_at(position);
    }
    let session_start = cache.session_start.unwrap_or(position);
    if cache.cycle != Some(cycle) {
        // Process the end of the previous cached cycle before replacing it, so
        // a short onset between two UI frames can still start a longer hold.
        let ActivityCache { events, holds, .. } = &mut *cache;
        ingest_activity(holds, events, position, session_start);
        cache.events.clear();
        cache.cycle = Some(cycle);
        if let Some(window) = Span::new(CycleTime::new(cycle, 1), CycleTime::new(cycle + 1, 1)) {
            for (output, score) in &compiled.scores {
                if let Ok(report) = CadenceCompiler::new().preview(score, &window) {
                    cache.events.extend(
                        report
                            .events
                            .into_iter()
                            .map(|event| (output.clone(), event)),
                    );
                }
            }
        }
    }
    let ActivityCache {
        events,
        routes,
        holds,
        ..
    } = &mut *cache;
    *activity = project_activity(
        compiled,
        events,
        position,
        |node| project.document.graph.contains_node(node),
        routes,
        holds,
        session_start,
    );
    cache.last_position = Some(position);
    // Stable identities can move, but deleted notes must never bind to a new
    // tile that later occupies the same cell.
    activity
        .nodes
        .retain(|node, _| project.document.graph.contains_node(node));
    activity.connections.retain(|(from, to)| {
        project.document.graph.contains_node(from) && project.document.graph.contains_node(to)
    });
}

fn project_activity(
    compiled: &CompiledProject,
    events: &[(String, EvaluatedEvent)],
    position: CycleTime,
    source_exists: impl Fn(&NodeId) -> bool,
    routes: &mut BTreeMap<(String, NodeId), PlaybackActivity>,
    holds: &mut BTreeMap<(String, VoiceInstanceId), HeldActivity>,
    session_start: CycleTime,
) -> PlaybackActivity {
    let mut activity = PlaybackActivity {
        cycle: position.value(),
        playing: true,
        ..default()
    };
    ingest_activity(holds, events, position, session_start);
    for ((output, _), hold) in holds.iter() {
        let event = &hold.event;
        let Some(source) = event
            .projected()
            .id()
            .and_then(|id| compiled.source_nodes.get(&id.value()))
        else {
            continue;
        };
        if !source_exists(source) {
            continue;
        }
        let level = event_level(event, position.value(), hold.gate_end);
        if level > 0.001 {
            let route = routes
                .entry((output.clone(), source.clone()))
                .or_insert_with(|| {
                    let mut route = PlaybackActivity::default();
                    compiled
                        .activity_routing
                        .add_route(source, output, 1.0, &mut route);
                    route
                });
            for node in route.nodes.keys() {
                activity.add_node(node.clone(), level);
            }
            activity
                .connections
                .extend(route.connections.iter().cloned());
        }
    }
    activity
}

fn scalar(
    controls: &ControlMap,
    key: ControlKey,
    fallback: f64,
    position: f64,
    progress: f64,
) -> f64 {
    match controls.get(&key) {
        Some(ControlValue::Scalar(value)) => *value,
        Some(ControlValue::Unipolar(value)) => value.value(),
        Some(ControlValue::Signal(signal)) => signal.eval(position),
        Some(ControlValue::Ramp { from, to }) => from + (to - from) * progress,
        _ => fallback,
    }
}

fn event_level(event: &EvaluatedEvent, position: f64, end: f64) -> f32 {
    let projected = event.projected();
    let Some(controls) = event_controls(event) else {
        return 0.0;
    };
    if matches!(
        controls.get(&ControlKey::Gate),
        Some(ControlValue::Bool(false))
    ) {
        return 0.0;
    }
    let whole = voice_whole(event);
    let elapsed = position - whole.start().value();
    let duration = (whole.end() - whole.start()).value();
    let progress = (elapsed / duration).clamp(0.0, 1.0);
    if position >= end {
        return 0.0;
    }
    let default_gain = match projected.intent() {
        Intent::Sample(sample) => sample.gain,
        _ => 1.0,
    };
    let signal_base = if matches!(
        controls.get(&ControlKey::Gain),
        Some(ControlValue::Signal(_))
    ) {
        default_gain
    } else {
        1.0
    };
    let gain = signal_base
        * scalar(
            &controls,
            ControlKey::Gain,
            default_gain,
            position,
            progress,
        )
        * scalar(&controls, ControlKey::Velocity, 1.0, position, progress)
        * scalar(&controls, ControlKey::PostGain, 1.0, position, progress)
        * scalar(&controls, ControlKey::Expression, 1.0, position, progress);
    // A short onset emphasis follows note time, including seeking into a note.
    (gain.clamp(0.0, 1.0) * (0.65 + 0.35 * (1.0 - progress).powi(3))) as f32
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{
        board::BoardSlot,
        document::{AtomValue, NoteName, PlacementAddress, TileSpawnKind},
    };
    use cadence::prelude::{
        BuiltInSynthSource, ControlScore, ControlTile, ControlTrack, Score, Voice, tile,
    };

    fn note_score(start: CycleTime, end: CycleTime) -> Score {
        Score::from(
            Voice::new(
                CycleTime::ONE,
                vec![tile(start, end, Intent::synth(BuiltInSynthSource::Sine)).with_id(1u64)],
            )
            .unwrap(),
        )
    }
    fn compiled(score: Score) -> CompiledProject {
        let playback_score = cadence::prelude::PreparedScore::new(score.clone()).unwrap();
        CompiledProject {
            tessera_ir: Default::default(),
            source_nodes: BTreeMap::from([(1, NodeId::new("note"))]),
            scores: BTreeMap::from([(
                "out".into(),
                cadence::prelude::PreparedScore::new(score).unwrap(),
            )]),
            playback_score,
            activity_routing: ActivityRouting {
                parents: BTreeMap::from([(NodeId::new("note"), NodeId::new("pattern"))]),
                edges: [
                    ("pattern", "gain"),
                    ("gain", "out"),
                    ("gain", "muted"),
                    ("amount", "gain"),
                    ("other_note", "out"),
                ]
                .map(|(a, b)| (NodeId::new(a), NodeId::new(b)))
                .into(),
                control_edges: [(NodeId::new("amount"), NodeId::new("gain"))].into(),
            },
        }
    }
    fn query(compiled: &CompiledProject, position: CycleTime) -> PlaybackActivity {
        let floor = CycleTime::new(position.value().floor() as i64, 1);
        let window = Span::new(floor, floor + CycleTime::ONE).unwrap();
        let events: Vec<_> = compiled
            .scores
            .iter()
            .flat_map(|(output, score)| {
                CadenceCompiler::new()
                    .preview(score, &window)
                    .unwrap()
                    .events
                    .into_iter()
                    .map(|event| (output.clone(), event))
            })
            .collect();
        project_activity(
            compiled,
            &events,
            position,
            |_| true,
            &mut BTreeMap::new(),
            &mut BTreeMap::new(),
            floor,
        )
    }
    fn has(activity: &PlaybackActivity, node: &str) -> bool {
        activity.nodes.contains_key(&NodeId::new(node))
    }
    fn controlled(
        score: Score,
        key: ControlKey,
        values: Vec<(CycleTime, CycleTime, ControlValue)>,
    ) -> Score {
        let controls = ControlTrack::new(
            CycleTime::ONE,
            values
                .into_iter()
                .map(|(start, end, value)| {
                    ControlTile::spanning(start, end, key.clone(), value).unwrap()
                })
                .collect(),
        )
        .unwrap();
        Score::with_controls(score, ControlScore::from(controls))
    }

    #[test]
    fn exact_note_time_lights_ancestors_and_only_the_sounding_output_route() {
        let compiled = compiled(note_score(CycleTime::new(1, 4), CycleTime::new(3, 4)));
        assert!(query(&compiled, CycleTime::ZERO).nodes.is_empty());
        let active = query(&compiled, CycleTime::new(1, 4));
        for node in ["note", "pattern", "gain", "out", "amount"] {
            assert!(has(&active, node), "{node}");
        }
        assert!(!has(&active, "muted"));
        assert!(!has(&active, "other_note"));
        assert!(
            active
                .connections
                .contains(&(NodeId::new("amount"), NodeId::new("gain")))
        );
        assert!(query(&compiled, CycleTime::new(3, 4)).nodes.is_empty());
    }

    #[test]
    fn held_note_gain_and_gate_updates_override_the_onset_snapshot() {
        for (key, muted, audible) in [
            (
                ControlKey::Gain,
                ControlValue::Scalar(0.0),
                ControlValue::Scalar(1.0),
            ),
            (
                ControlKey::Gate,
                ControlValue::Bool(false),
                ControlValue::Bool(true),
            ),
        ] {
            let score = controlled(
                note_score(CycleTime::ZERO, CycleTime::ONE),
                key,
                vec![
                    (CycleTime::ZERO, CycleTime::new(1, 4), audible.clone()),
                    (CycleTime::new(1, 4), CycleTime::new(3, 4), muted),
                    (CycleTime::new(3, 4), CycleTime::ONE, audible),
                ],
            );
            let compiled = compiled(score);
            assert!(has(&query(&compiled, CycleTime::new(1, 8)), "note"));
            assert!(!has(&query(&compiled, CycleTime::new(1, 2)), "note"));
            assert!(has(&query(&compiled, CycleTime::new(7, 8)), "note"));
        }
    }

    #[test]
    fn gain_signal_and_shortened_gate_follow_musical_time() {
        let signal = cadence::domain::signal::Signal::square()
            .with_bias(0.5)
            .with_depth(0.5);
        let score = controlled(
            note_score(CycleTime::ZERO, CycleTime::ONE),
            ControlKey::Gain,
            vec![(
                CycleTime::ZERO,
                CycleTime::ONE,
                ControlValue::Signal(signal),
            )],
        );
        let compiled = compiled(score);
        for position in [CycleTime::new(1, 4), CycleTime::new(3, 4)] {
            assert_eq!(
                has(&query(&compiled, position), "note"),
                signal.eval(position.value()) > 0.0
            );
        }
        let short = compiled_with_clip(0.25);
        assert!(has(&query(&short, CycleTime::new(1, 8)), "note"));
        assert!(!has(&query(&short, CycleTime::new(1, 2)), "note"));
    }
    fn compiled_with_clip(length: f64) -> CompiledProject {
        compiled(controlled(
            note_score(CycleTime::ZERO, CycleTime::ONE),
            ControlKey::ClipLength,
            vec![(
                CycleTime::ZERO,
                CycleTime::ONE,
                ControlValue::Scalar(length),
            )],
        ))
    }

    #[test]
    fn deleting_a_source_also_unlinks_its_containing_pattern_and_route() {
        let compiled = compiled(note_score(CycleTime::ZERO, CycleTime::ONE));
        let window = Span::new(CycleTime::ZERO, CycleTime::ONE).unwrap();
        let events: Vec<_> = CadenceCompiler::new()
            .preview(&compiled.scores["out"], &window)
            .unwrap()
            .events
            .into_iter()
            .map(|event| ("out".into(), event))
            .collect();
        assert!(
            project_activity(
                &compiled,
                &events,
                CycleTime::new(1, 2),
                |_| false,
                &mut BTreeMap::new(),
                &mut BTreeMap::new(),
                CycleTime::ZERO,
            )
            .nodes
            .is_empty()
        );
    }

    #[test]
    fn actual_clock_pause_seek_and_pending_edits_do_not_use_future_preview_time() {
        use cadence::{
            adapter::audio::{AudioRenderer, AudioRendererSettings},
            infrastructure::playback::PlaybackSettings,
        };
        let mut project = MusaicProject::new_empty();
        let node = project
            .document
            .graph
            .insert_tile(
                &mut project.document.surfaces,
                project.document.root_surface,
                PlacementAddress::BoardSlot(BoardSlot::new(0, 0)),
                TileSpawnKind::Atom {
                    atom: AtomValue::NoteName(NoteName::C),
                },
            )
            .unwrap();
        let mut accepted = compiled(note_score(CycleTime::ZERO, CycleTime::new(1, 2)));
        accepted.source_nodes.insert(1, node.clone());
        let (audio, _renderer) =
            AudioRenderer::split(AudioRendererSettings::new(8000, 256)).unwrap();
        let mut playback = PlaybackRuntime::new(PlaybackSettings::default(), audio);
        playback.resume().unwrap();
        let mut app = App::new();
        app.insert_non_send_resource(playback)
            .insert_resource(project)
            .insert_resource(RuntimeState {
                compiled: Some(accepted.clone()),
                proposed: Some((100, 2, compiled(Score::empty()))),
                ..default()
            })
            .init_resource::<cadence::bevy::PlaybackSync>()
            .init_resource::<ActivityCache>()
            .init_resource::<PlaybackActivity>()
            .add_systems(Update, update_playback_activity);
        app.update();
        assert!(
            app.world()
                .resource::<PlaybackActivity>()
                .nodes
                .contains_key(&node)
        );
        assert_eq!(app.world().resource::<ActivityCache>().cycle, Some(0));
        app.world_mut()
            .non_send_resource_mut::<PlaybackRuntime>()
            .seek(CycleTime::new(3, 4))
            .unwrap();
        app.update();
        assert!(app.world().resource::<PlaybackActivity>().nodes.is_empty());
        app.world_mut()
            .non_send_resource_mut::<PlaybackRuntime>()
            .seek(CycleTime::new(1, 4))
            .unwrap();
        app.update();
        assert!(
            app.world()
                .resource::<PlaybackActivity>()
                .nodes
                .contains_key(&node)
        );
        app.world_mut()
            .non_send_resource_mut::<PlaybackRuntime>()
            .pause()
            .unwrap();
        app.update();
        assert!(!app.world().resource::<PlaybackActivity>().playing);
        app.world_mut()
            .non_send_resource_mut::<PlaybackRuntime>()
            .resume()
            .unwrap();
        app.update();
        assert!(
            app.world()
                .resource::<PlaybackActivity>()
                .nodes
                .contains_key(&node)
        );
        // Acceptance within the same cycle invalidates both event and route caches.
        app.world_mut().resource_mut::<RuntimeState>().compiled = Some(compiled(Score::empty()));
        app.world_mut()
            .resource_mut::<cadence::bevy::PlaybackSync>()
            .last_applied_revision = 101;
        app.update();
        assert!(app.world().resource::<PlaybackActivity>().nodes.is_empty());
        app.world_mut()
            .non_send_resource_mut::<PlaybackRuntime>()
            .stop()
            .unwrap();
        app.update();
        assert!(!app.world().resource::<PlaybackActivity>().playing);
    }

    fn held_score() -> Score {
        let voice = Voice::new(
            CycleTime::ONE,
            vec![
                tile(
                    CycleTime::ZERO,
                    CycleTime::new(1, 4),
                    Intent::synth(BuiltInSynthSource::Sine),
                )
                .with_id(1u64),
            ],
        )
        .unwrap()
        .with_repeat(cadence::prelude::Repeat::Once);
        controlled(
            Score::from(voice),
            ControlKey::Legato,
            vec![(CycleTime::ZERO, CycleTime::ONE, ControlValue::Scalar(6.0))],
        )
    }

    fn consume_test_transport_request(
        mut runtime: ResMut<RuntimeState>,
        mut playback: NonSendMut<PlaybackRuntime>,
    ) {
        if let Some(request) = runtime.transport_request.take() {
            match request {
                super::super::TransportRequest::Seek(position) => playback.seek(position),
                super::super::TransportRequest::Stop => playback.stop(),
                super::super::TransportRequest::Panic => playback.panic(),
            }
            .unwrap();
        }
    }

    fn audible_app(score: Score) -> (App, cadence::adapter::audio::AudioRenderer, NodeId) {
        use cadence::{
            adapter::audio::{AudioRenderer, AudioRendererSettings},
            infrastructure::playback::PlaybackSettings,
        };
        let mut project = MusaicProject::new_empty();
        project.document.playback.bpm = 240.0;
        let node = project
            .document
            .graph
            .insert_tile(
                &mut project.document.surfaces,
                project.document.root_surface,
                PlacementAddress::BoardSlot(BoardSlot::new(0, 0)),
                TileSpawnKind::Atom {
                    atom: AtomValue::NoteName(NoteName::C),
                },
            )
            .unwrap();
        let mut accepted = compiled(score.clone());
        accepted.source_nodes.insert(1, node.clone());
        let (audio, renderer) =
            AudioRenderer::split(AudioRendererSettings::new(8000, 4096)).unwrap();
        let mut playback = PlaybackRuntime::new(
            PlaybackSettings {
                cps: CycleTime::ONE,
                ..default()
            },
            audio,
        );
        playback
            .play_prepared_score(cadence::prelude::PreparedScore::new(score).unwrap())
            .unwrap();
        let mut app = App::new();
        app.insert_non_send_resource(playback)
            .insert_resource(project)
            .insert_resource(RuntimeState {
                compiled: Some(accepted),
                ..default()
            })
            .init_resource::<cadence::bevy::PlaybackSync>()
            .init_resource::<ActivityCache>()
            .init_resource::<PlaybackActivity>()
            .add_systems(
                Update,
                (
                    reset_activity_for_transport_changes,
                    consume_test_transport_request,
                    update_playback_activity,
                )
                    .chain(),
            );
        app.update();
        (app, renderer, node)
    }

    fn render_frames(
        app: &mut App,
        renderer: &mut cadence::adapter::audio::AudioRenderer,
        count: usize,
    ) -> Vec<cadence::adapter::audio::Frame> {
        let mut frames = vec![cadence::adapter::audio::Frame::from_mono(0.0); count];
        for chunk in frames.chunks_mut(64) {
            app.world_mut()
                .non_send_resource_mut::<PlaybackRuntime>()
                .tick()
                .unwrap();
            renderer.render(chunk);
            app.update();
        }
        frames
    }

    #[test]
    fn continuous_legato_hold_matches_audio_past_its_slot_and_across_cycles() {
        let (mut app, mut renderer, node) = audible_app(held_score());
        render_frames(&mut app, &mut renderer, 6000); // .75 cycle, after the .25-cycle slot
        assert!(
            app.world()
                .resource::<PlaybackActivity>()
                .nodes
                .contains_key(&node)
        );
        let held = render_frames(&mut app, &mut renderer, 4000); // 1.25 cycles
        assert!(
            held.iter().any(|frame| frame.left.abs() > 0.01),
            "Cadence still sounds the legato hold"
        );
        assert!(
            app.world()
                .resource::<PlaybackActivity>()
                .nodes
                .contains_key(&node),
            "the integer-cycle cache must retain the hold"
        );
        render_frames(&mut app, &mut renderer, 2200); // gate ended at 1.5
        assert!(
            !app.world()
                .resource::<PlaybackActivity>()
                .nodes
                .contains_key(&node)
        );
    }

    #[test]
    fn seeking_beyond_the_nominal_span_clears_the_hold_while_seeking_inside_backfills_it() {
        let (mut app, mut renderer, node) = audible_app(held_score());
        render_frames(&mut app, &mut renderer, 3000);
        assert!(
            app.world()
                .resource::<PlaybackActivity>()
                .nodes
                .contains_key(&node)
        );
        app.world_mut()
            .resource_mut::<RuntimeState>()
            .transport_request = Some(super::super::TransportRequest::Seek(CycleTime::new(1, 2)));
        app.update();
        assert!(
            !app.world()
                .resource::<PlaybackActivity>()
                .nodes
                .contains_key(&node)
        );
        let after_seek = render_frames(&mut app, &mut renderer, 1000);
        assert!(
            after_seek[500..]
                .iter()
                .all(|frame| frame.left.abs() < 0.0001),
            "Cadence cancels the tail instead of backfilling outside the nominal span"
        );
        app.world_mut()
            .resource_mut::<RuntimeState>()
            .transport_request = Some(super::super::TransportRequest::Seek(CycleTime::new(1, 8)));
        app.update();
        let resumed = render_frames(&mut app, &mut renderer, 9000); // 1.25 cycles
        assert!(resumed[8500..].iter().any(|frame| frame.left.abs() > 0.01));
        assert!(
            app.world()
                .resource::<PlaybackActivity>()
                .nodes
                .contains_key(&node)
        );
    }

    #[test]
    fn a_held_tail_keeps_its_last_gate_snapshot_instead_of_reopening_at_a_cycle_boundary() {
        let score = controlled(
            held_score(),
            ControlKey::Gate,
            vec![
                (
                    CycleTime::ZERO,
                    CycleTime::new(1, 8),
                    ControlValue::Bool(true),
                ),
                (
                    CycleTime::new(1, 8),
                    CycleTime::ONE,
                    ControlValue::Bool(false),
                ),
            ],
        );
        let (mut app, mut renderer, node) = audible_app(score);
        render_frames(&mut app, &mut renderer, 1200);
        assert!(
            !app.world()
                .resource::<PlaybackActivity>()
                .nodes
                .contains_key(&node)
        );
        let held = render_frames(&mut app, &mut renderer, 8800);
        assert!(held.iter().all(|frame| frame.left.abs() < 0.0001));
        assert!(
            !app.world()
                .resource::<PlaybackActivity>()
                .nodes
                .contains_key(&node)
        );
    }
}
