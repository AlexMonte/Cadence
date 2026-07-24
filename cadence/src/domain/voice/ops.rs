//! Source-level transforms for repeating voices.

use crate::domain::{
    intent::Intent,
    prelude::Time,
    span::{Phase, Span},
    voice::{Repeat, Tile, Voice},
};

/// Returns the voices as simultaneous layers.
///
/// Unlike [`chain`], this does not merge them into one longer period.
#[must_use]
pub fn overlay(voices: impl IntoIterator<Item = Voice>) -> Vec<Voice> {
    voices.into_iter().collect()
}

/// Places voices back-to-back inside one longer repeating voice.
#[must_use]
pub fn chain(voices: impl IntoIterator<Item = Voice>) -> Voice {
    let voices = voices.into_iter().collect::<Vec<_>>();
    let period = voices
        .iter()
        .fold(Time::ZERO, |acc, voice| acc + voice.period());
    let mut offset = Time::ZERO;
    let mut tiles = Vec::new();

    for voice in voices {
        for tile in voice.iter() {
            tiles.push(shift_tile(tile, offset));
        }
        offset = offset + voice.period();
    }

    Voice::new(period, tiles)
        .expect("chained voices must produce a valid voice")
        .with_repeat(Repeat::Forever)
}

/// Splits one tile into `parts` equal phase-local tiles.
///
/// # Panics
///
/// Panics if `parts == 0`.
#[must_use]
pub fn subdivide(tile: &Tile, parts: usize) -> Vec<Tile> {
    assert!(parts > 0, "subdivide requires at least one part");

    let duration = tile.phase().end() - tile.phase().start();
    let part = duration / Time::whole_number(parts as i64);

    (0..parts)
        .map(|index| {
            let offset = part * Time::whole_number(index as i64);
            let start = tile.phase().start() + offset;
            let end = start + part;

            clone_tile_with_phase(tile, Span::<Phase>::new(start, end).unwrap())
        })
        .collect()
}

/// Largest number of steps a runtime Euclidean rhythm may request.
///
/// Hand-authored literals are small, but parser-driven input can be hostile or
/// typo'd; this cap keeps [`try_euclid`] from allocating a huge necklace.
pub const MAX_EUCLID_STEPS: u32 = 1024;

/// Reasons a runtime Euclidean rhythm request can be rejected.
///
/// The compile-time [`euclid`]/[`euclid_inv`] family proves these invariants
/// statically; [`try_euclid`]/[`try_euclid_inv`] re-check them on dynamic input
/// (for example values parsed from mini-notation) and report a reason so callers
/// can attach a source span.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum EuclidError {
    /// `steps` was zero; a cycle cannot be divided into no slots.
    #[error("euclid needs at least one step")]
    ZeroSteps,
    /// `pulses` exceeded `steps`; the requested rhythm cannot fit.
    #[error("pulses ({pulses}) cannot exceed steps ({steps})")]
    PulsesExceedSteps {
        /// Requested number of onsets.
        pulses: u32,
        /// Requested number of slots.
        steps: u32,
    },
    /// `steps` exceeded [`MAX_EUCLID_STEPS`].
    #[error("steps ({steps}) exceeds the maximum of {max}")]
    StepsTooLarge {
        /// Requested number of slots.
        steps: u32,
        /// The enforced ceiling, [`MAX_EUCLID_STEPS`].
        max: u32,
    },
}

/// Validates parameters and builds the onset necklace, the single source of
/// truth shared by every Euclidean builder.
///
/// `pulses == 0` is valid silence (all slots off), not an error.
fn try_euclid_necklace(pulses: u32, steps: u32, rotation: i32) -> Result<Vec<bool>, EuclidError> {
    if steps == 0 {
        return Err(EuclidError::ZeroSteps);
    }
    if steps > MAX_EUCLID_STEPS {
        return Err(EuclidError::StepsTooLarge {
            steps,
            max: MAX_EUCLID_STEPS,
        });
    }
    if pulses > steps {
        return Err(EuclidError::PulsesExceedSteps { pulses, steps });
    }

    Ok((0..steps)
        .map(|step| euclid_has_pulse(step, pulses, steps, rotation))
        .collect())
}

/// Places one tile per slot whose onset flag matches `want_onset`.
///
/// Emitting on `false` yields the inverse rhythm by inverting the *actual*
/// necklace, so the inverse never depends on re-running the distribution with a
/// complementary pulse count.
fn voice_from_necklace(necklace: &[bool], want_onset: bool, intent: &Intent) -> Voice {
    let steps = necklace.len() as i64;
    let step_width = Time::ONE / Time::whole_number(steps);
    let mut tiles = Vec::new();
    for (step, &has_pulse) in necklace.iter().enumerate() {
        if has_pulse == want_onset {
            let start = step_width * Time::whole_number(step as i64);
            let end = start + step_width;
            tiles.push(
                Tile::spanning(start, end, intent.clone())
                    .expect("euclid step spans must be valid"),
            );
        }
    }

    Voice::new(Time::ONE, tiles)
        .expect("euclid must produce a valid voice")
        .with_repeat(Repeat::Forever)
}

/// Builds a one-cycle Euclidean rhythm voice from dynamic parameters.
///
/// `pulses` events are distributed across `steps` equal slots; `rotation`
/// rotates the resulting pattern by step count (any `i32`, reduced modulo
/// `steps`). Use this for values that are not known at compile time, such as
/// those parsed from mini-notation. For hand-authored literals prefer the
/// compile-time-checked [`euclid`].
///
/// # Errors
///
/// Returns [`EuclidError`] when `steps == 0`, `steps` exceeds
/// [`MAX_EUCLID_STEPS`], or `pulses > steps`. `pulses == 0` is valid silence.
pub fn try_euclid(
    pulses: u32,
    steps: u32,
    rotation: i32,
    intent: Intent,
) -> Result<Voice, EuclidError> {
    let necklace = try_euclid_necklace(pulses, steps, rotation)?;
    Ok(voice_from_necklace(&necklace, true, &intent))
}

/// Builds a one-cycle Euclidean rhythm from dynamic parameters and keeps only
/// the inverse/rest slots.
///
/// This inverts the actual onset necklace produced by [`try_euclid`]; it is not
/// the Euclidean rhythm of the complementary pulse count.
///
/// # Errors
///
/// Returns [`EuclidError`] under the same conditions as [`try_euclid`].
pub fn try_euclid_inv(
    pulses: u32,
    steps: u32,
    rotation: i32,
    intent: Intent,
) -> Result<Voice, EuclidError> {
    let necklace = try_euclid_necklace(pulses, steps, rotation)?;
    Ok(voice_from_necklace(&necklace, false, &intent))
}

/// Builds a one-cycle Euclidean rhythm voice, checking `PULSES`/`STEPS` at
/// compile time.
///
/// `PULSES` events are distributed across `STEPS` equal slots. The rotation is
/// zero; use [`euclid_rot`] to rotate. For dynamic parameters use [`try_euclid`].
///
/// The bound `STEPS > 0 && PULSES <= STEPS` is enforced at monomorphization, so
/// an impossible request such as a 9-of-8 rhythm fails to compile:
///
/// ```compile_fail
/// use cadence::domain::intent::Intent;
/// use cadence::domain::voice::ops::euclid;
///
/// let _ = euclid::<9, 8>(Intent::sample("bd"));
/// ```
///
/// A valid request compiles and runs:
///
/// ```
/// use cadence::domain::intent::Intent;
/// use cadence::domain::voice::ops::euclid;
///
/// let rhythm = euclid::<3, 8>(Intent::sample("bd"));
/// assert_eq!(rhythm.tiles().len(), 3);
/// ```
#[must_use]
pub fn euclid<const PULSES: u32, const STEPS: u32>(intent: Intent) -> Voice {
    euclid_rot::<PULSES, STEPS>(0, intent)
}

/// Like [`euclid`], but rotates the pattern by `rotation` step counts.
///
/// `rotation` is any `i32`, reduced modulo `STEPS`, so it carries no invariant
/// and stays a runtime parameter (handy for live modulation).
#[must_use]
pub fn euclid_rot<const PULSES: u32, const STEPS: u32>(rotation: i32, intent: Intent) -> Voice {
    const {
        assert!(
            STEPS > 0 && PULSES <= STEPS,
            "euclid requires STEPS > 0 and PULSES <= STEPS"
        );
    }
    try_euclid(PULSES, STEPS, rotation, intent).expect("const-checked euclid is always valid")
}

/// Builds a one-cycle Euclidean rhythm and keeps only the inverse/rest slots,
/// checking `PULSES`/`STEPS` at compile time.
///
/// The rotation is zero; use [`euclid_inv_rot`] to rotate. For dynamic
/// parameters use [`try_euclid_inv`].
#[must_use]
pub fn euclid_inv<const PULSES: u32, const STEPS: u32>(intent: Intent) -> Voice {
    euclid_inv_rot::<PULSES, STEPS>(0, intent)
}

/// Like [`euclid_inv`], but rotates the underlying pattern by `rotation` step
/// counts before inverting.
#[must_use]
pub fn euclid_inv_rot<const PULSES: u32, const STEPS: u32>(rotation: i32, intent: Intent) -> Voice {
    const {
        assert!(
            STEPS > 0 && PULSES <= STEPS,
            "euclid_inv requires STEPS > 0 and PULSES <= STEPS"
        );
    }
    try_euclid_inv(PULSES, STEPS, rotation, intent)
        .expect("const-checked euclid_inv is always valid")
}

/// Scales a voice period and every tile span by `factor`.
///
/// # Panics
///
/// Panics if `factor <= 0`.
#[must_use]
pub fn stretch(voice: &Voice, factor: Time) -> Voice {
    assert!(factor > Time::ZERO, "stretch factor must be positive");

    Voice::new(
        voice.period() * factor,
        voice
            .iter()
            .map(|tile| {
                clone_tile_with_phase(
                    tile,
                    Span::<Phase>::new(tile.phase().start() * factor, tile.phase().end() * factor)
                        .unwrap(),
                )
            })
            .collect(),
    )
    .unwrap()
    .with_repeat(voice.repeat())
}

/// Inverse of [`stretch`].
///
/// # Panics
///
/// Panics if `factor <= 0`.
#[must_use]
pub fn shrink(voice: &Voice, factor: Time) -> Voice {
    assert!(factor > Time::ZERO, "shrink factor must be positive");
    stretch(voice, Time::ONE / factor)
}

/// Reinterprets the voice as if its internal phase were running at `rate`.
///
/// Rates greater than one create denser repetitions inside the same period;
/// rates smaller than one spread material out.
///
/// # Panics
///
/// Panics if `rate <= 0`.
#[must_use]
pub fn warp_rate(voice: &Voice, rate: Time) -> Voice {
    assert!(rate > Time::ZERO, "warp rate must be positive");

    let mut warped = Vec::new();

    for tile in voice.iter() {
        for phase in warped_tile_spans(tile.phase(), voice.period(), rate) {
            warped.push(clone_tile_with_phase(tile, phase));
        }
    }

    Voice::new(voice.period(), warped)
        .unwrap()
        .with_repeat(voice.repeat())
}

/// Convenience wrapper for [`warp_rate`] with `factor > 1`.
#[must_use]
pub fn fast_by(voice: &Voice, factor: Time) -> Voice {
    assert!(
        factor > Time::ONE,
        "fast_by factor must be greater than one"
    );
    warp_rate(voice, factor)
}

/// Convenience wrapper for slowing the internal phase by `factor > 1`.
#[must_use]
pub fn slow_by(voice: &Voice, factor: Time) -> Voice {
    assert!(
        factor > Time::ONE,
        "slow_by factor must be greater than one"
    );
    warp_rate(voice, Time::ONE / factor)
}

/// Rotates tile phases inside one period, wrapping across the cycle boundary.
#[must_use]
pub fn rotate(voice: &Voice, offset: Time) -> Voice {
    let mut rotated = Vec::new();

    for tile in voice.iter() {
        let shifted_start = wrap_phase(tile.phase().start() + offset, voice.period());
        let shifted_end = tile.phase().end() - tile.phase().start() + shifted_start;

        if shifted_end <= voice.period() {
            rotated.push(clone_tile_with_phase(
                tile,
                Span::<Phase>::new(shifted_start, shifted_end).unwrap(),
            ));
        } else {
            rotated.push(clone_tile_with_phase(
                tile,
                Span::<Phase>::new(shifted_start, voice.period()).unwrap(),
            ));
            rotated.push(clone_tile_with_phase(
                tile,
                Span::<Phase>::new(Time::ZERO, shifted_end - voice.period()).unwrap(),
            ));
        }
    }

    Voice::new(voice.period(), rotated)
        .unwrap()
        .with_repeat(voice.repeat())
}

/// Mirrors all tile spans around the voice period.
#[must_use]
pub fn mirror(voice: &Voice) -> Voice {
    Voice::new(
        voice.period(),
        voice
            .iter()
            .map(|tile| {
                clone_tile_with_phase(
                    tile,
                    Span::<Phase>::new(
                        voice.period() - tile.phase().end(),
                        voice.period() - tile.phase().start(),
                    )
                    .unwrap(),
                )
            })
            .collect(),
    )
    .unwrap()
    .with_repeat(voice.repeat())
}

/// Clips tiles to the parts that overlap any gate span.
#[must_use]
pub fn gate_by_overlap(voice: &Voice, gates: &[Span<Phase>]) -> Voice {
    let mut tiles = Vec::new();

    for tile in voice.iter() {
        tiles.extend(
            gates
                .iter()
                .filter_map(|gate| tile.phase().intersection(gate))
                .map(|phase| clone_tile_with_phase(tile, phase)),
        );
    }

    Voice::new(voice.period(), tiles)
        .unwrap()
        .with_repeat(voice.repeat())
}

fn shift_tile(tile: &Tile, offset: Time) -> Tile {
    clone_tile_with_phase(
        tile,
        Span::<Phase>::new(tile.phase().start() + offset, tile.phase().end() + offset).unwrap(),
    )
}

fn clone_tile_with_phase(tile: &Tile, phase: Span<Phase>) -> Tile {
    let mut cloned = Tile::new(phase, tile.intent().clone())
        .unwrap()
        .with_position(tile.position());
    if let Some(id) = tile.id() {
        cloned = cloned.with_id(id);
    }
    cloned
}

fn warped_tile_spans(tile_phase: Span<Phase>, period: Time, rate: Time) -> Vec<Span<Phase>> {
    let total_inner = period * rate;
    let period_window = Span::<Phase>::new(Time::ZERO, period).unwrap();
    let copy_count = (total_inner / period).floor() + 1;
    let mut warped = Vec::new();

    for copy in 0..copy_count {
        let wrap_offset = period * Time::whole_number(copy);
        let inner_start = tile_phase.start() + wrap_offset;

        if inner_start >= total_inner {
            break;
        }

        let mapped =
            Span::<Phase>::new(inner_start / rate, (tile_phase.end() + wrap_offset) / rate)
                .unwrap();

        if let Some(clipped) = mapped.intersection(&period_window) {
            warped.push(clipped);
        }
    }

    warped
}

fn wrap_phase(time: Time, period: Time) -> Time {
    let quotient = (time / period).floor();
    let wrapped = time - period * Time::whole_number(quotient);

    if wrapped == period {
        Time::ZERO
    } else {
        wrapped
    }
}

fn euclid_has_pulse(step: u32, pulses: u32, steps: u32, rotation: i32) -> bool {
    if pulses == 0 {
        return false;
    }

    let rotated = (i64::from(step) - i64::from(rotation)).rem_euclid(i64::from(steps)) as u32;
    ((rotated * pulses) % steps) < pulses
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{intent::Intent, voice::TileId};

    fn source_tile(start: (i64, i64), end: (i64, i64), sample: &str, id: u64) -> Tile {
        Tile::spanning(
            Time::new(start.0, start.1),
            Time::new(end.0, end.1),
            Intent::sample(sample),
        )
        .unwrap()
        .with_id(id)
    }

    fn voice(period: (i64, i64), tiles: Vec<Tile>) -> Voice {
        Voice::new(Time::new(period.0, period.1), tiles).unwrap()
    }

    #[test]
    fn chain_builds_a_larger_repeating_voice() {
        let chained = chain(vec![
            voice((1, 1), vec![source_tile((0, 1), (1, 2), "kick", 1)]),
            voice((1, 1), vec![source_tile((0, 1), (1, 2), "snare", 2)]),
        ]);

        assert_eq!(chained.period(), Time::new(2, 1));
        assert_eq!(
            chained.tiles()[0].phase(),
            Span::<Phase>::new(Time::ZERO, Time::new(1, 2)).unwrap()
        );
        assert_eq!(
            chained.tiles()[1].phase(),
            Span::<Phase>::new(Time::ONE, Time::new(3, 2)).unwrap()
        );
    }

    #[test]
    fn subdivide_partitions_tile_phase_evenly() {
        let tile = source_tile((0, 1), (1, 1), "vox", 7);
        let parts = subdivide(&tile, 4);

        assert_eq!(parts.len(), 4);
        assert_eq!(parts[3].id(), Some(TileId::new(7)));
        assert_eq!(
            parts[3].phase(),
            Span::<Phase>::new(Time::new(3, 4), Time::ONE).unwrap()
        );
    }

    #[test]
    fn rotate_wraps_tiles_inside_the_period() {
        let rotated = rotate(
            &voice((1, 1), vec![source_tile((3, 4), (1, 1), "hat", 1)]),
            Time::new(1, 4),
        );

        assert_eq!(rotated.tiles().len(), 1);
        assert_eq!(
            rotated.tiles()[0].phase(),
            Span::<Phase>::new(Time::ZERO, Time::new(1, 4)).unwrap()
        );
    }

    #[test]
    fn mirror_reflects_tiles_inside_the_period() {
        let mirrored = mirror(&voice(
            (1, 1),
            vec![
                source_tile((1, 4), (1, 2), "snare", 9),
                source_tile((0, 1), (1, 4), "hat", 3),
            ],
        ));

        assert_eq!(
            mirrored.tiles()[0].phase(),
            Span::<Phase>::new(Time::new(1, 2), Time::new(3, 4)).unwrap()
        );
        assert_eq!(
            mirrored.tiles()[1].phase(),
            Span::<Phase>::new(Time::new(3, 4), Time::ONE).unwrap()
        );
    }

    #[test]
    fn gate_by_overlap_clips_tiles_to_gate_spans() {
        let gated = gate_by_overlap(
            &voice((1, 1), vec![source_tile((0, 1), (1, 1), "pad", 4)]),
            &[Span::<Phase>::new(Time::new(1, 4), Time::new(3, 4)).unwrap()],
        );

        assert_eq!(gated.tiles().len(), 1);
        assert_eq!(
            gated.tiles()[0].phase(),
            Span::<Phase>::new(Time::new(1, 4), Time::new(3, 4)).unwrap()
        );
    }

    #[test]
    fn fast_by_duplicates_a_half_cycle_pulse_inside_the_same_period() {
        let warped = fast_by(
            &voice((1, 1), vec![source_tile((0, 1), (1, 2), "bd", 7)]),
            Time::new(2, 1),
        );

        assert_eq!(warped.period(), Time::ONE);
        assert_eq!(warped.tiles().len(), 2);
        assert_eq!(warped.tiles()[0].id(), Some(TileId::new(7)));
        assert_eq!(
            warped.tiles()[0].phase(),
            Span::<Phase>::new(Time::ZERO, Time::new(1, 4)).unwrap()
        );
        assert_eq!(warped.tiles()[1].id(), Some(TileId::new(7)));
        assert_eq!(
            warped.tiles()[1].phase(),
            Span::<Phase>::new(Time::new(1, 2), Time::new(3, 4)).unwrap()
        );
    }

    #[test]
    fn slow_by_expands_a_half_cycle_pulse_to_fill_the_outer_cycle() {
        let warped = slow_by(
            &voice((1, 1), vec![source_tile((0, 1), (1, 2), "bd", 8)]),
            Time::new(2, 1),
        );

        assert_eq!(warped.period(), Time::ONE);
        assert_eq!(warped.tiles().len(), 1);
        assert_eq!(warped.repeat(), Repeat::Forever);
        assert_eq!(warped.tiles()[0].id(), Some(TileId::new(8)));
        assert_eq!(
            warped.tiles()[0].phase(),
            Span::<Phase>::new(Time::ZERO, Time::ONE).unwrap()
        );
    }

    #[test]
    fn euclid_distributes_three_pulses_over_eight_steps() {
        let rhythm = euclid::<3, 8>(Intent::sample("bd"));
        let phases = rhythm.tiles().iter().map(Tile::phase).collect::<Vec<_>>();

        assert_eq!(
            phases,
            vec![
                Span::<Phase>::new(Time::ZERO, Time::new(1, 8)).unwrap(),
                Span::<Phase>::new(Time::new(3, 8), Time::new(1, 2)).unwrap(),
                Span::<Phase>::new(Time::new(3, 4), Time::new(7, 8)).unwrap(),
            ]
        );
    }

    #[test]
    fn euclid_rotation_shifts_pulse_positions() {
        let rhythm = euclid_rot::<3, 8>(1, Intent::sample("bd"));
        let phases = rhythm.tiles().iter().map(Tile::phase).collect::<Vec<_>>();

        assert_eq!(
            phases,
            vec![
                Span::<Phase>::new(Time::new(1, 8), Time::new(1, 4)).unwrap(),
                Span::<Phase>::new(Time::new(1, 2), Time::new(5, 8)).unwrap(),
                Span::<Phase>::new(Time::new(7, 8), Time::ONE).unwrap(),
            ]
        );
    }

    #[test]
    fn euclid_inverse_keeps_the_non_pulse_slots() {
        let rhythm = euclid_inv::<3, 8>(Intent::sample("hh"));
        assert_eq!(rhythm.tiles().len(), 5);
    }

    #[test]
    fn try_euclid_rejects_zero_steps() {
        assert_eq!(
            try_euclid(0, 0, 0, Intent::sample("bd")),
            Err(EuclidError::ZeroSteps)
        );
    }

    #[test]
    fn try_euclid_rejects_pulses_exceeding_steps() {
        assert_eq!(
            try_euclid(9, 8, 0, Intent::sample("bd")),
            Err(EuclidError::PulsesExceedSteps {
                pulses: 9,
                steps: 8
            })
        );
    }

    #[test]
    fn try_euclid_zero_pulses_is_valid_silence() {
        let rhythm = try_euclid(0, 8, 0, Intent::sample("bd")).expect("zero pulses is valid");
        assert_eq!(rhythm.tiles().len(), 0);
    }

    #[test]
    fn try_euclid_caps_huge_step_counts() {
        let steps = MAX_EUCLID_STEPS + 1;
        assert_eq!(
            try_euclid(3, steps, 0, Intent::sample("bd")),
            Err(EuclidError::StepsTooLarge {
                steps,
                max: MAX_EUCLID_STEPS
            })
        );
    }

    #[test]
    fn try_euclid_normalizes_negative_rotation() {
        let negative = try_euclid(3, 8, -1, Intent::sample("bd")).expect("valid");
        let equivalent = try_euclid(3, 8, 7, Intent::sample("bd")).expect("valid");

        let negative_phases = negative.tiles().iter().map(Tile::phase).collect::<Vec<_>>();
        let equivalent_phases = equivalent
            .tiles()
            .iter()
            .map(Tile::phase)
            .collect::<Vec<_>>();

        assert_eq!(negative_phases, equivalent_phases);
    }

    #[test]
    fn try_euclid_inv_is_exact_complement() {
        let onsets = try_euclid(3, 8, 0, Intent::sample("bd")).expect("valid");
        let rests = try_euclid_inv(3, 8, 0, Intent::sample("bd")).expect("valid");

        assert_eq!(onsets.tiles().len() + rests.tiles().len(), 8);

        let onset_starts = onsets
            .tiles()
            .iter()
            .map(|tile| tile.phase().start())
            .collect::<Vec<_>>();
        for rest in rests.tiles() {
            assert!(!onset_starts.contains(&rest.phase().start()));
        }
    }

    #[test]
    fn warp_rate_preserves_repeat_policy() {
        let warped = warp_rate(
            &voice((1, 1), vec![source_tile((0, 1), (1, 2), "bd", 4)])
                .with_repeat(Repeat::Count(3)),
            Time::new(3, 2),
        );

        assert_eq!(warped.period(), Time::ONE);
        assert_eq!(warped.repeat(), Repeat::Count(3));
    }

    #[test]
    fn warp_rate_keeps_tile_identity_across_duplicates() {
        let warped = fast_by(
            &voice((1, 1), vec![source_tile((0, 1), (1, 2), "bd", 1)]),
            Time::new(2, 1),
        );

        assert_eq!(warped.tiles().len(), 2);
        assert_eq!(warped.tiles()[0].id(), Some(TileId::new(1)));
        assert_eq!(warped.tiles()[1].id(), Some(TileId::new(1)));
    }

    #[test]
    fn warp_rate_keeps_tiles_sorted_deterministically() {
        let warped = warp_rate(
            &voice(
                (1, 1),
                vec![
                    source_tile((1, 2), (3, 4), "late", 9),
                    source_tile((0, 1), (1, 4), "early", 5),
                ],
            ),
            Time::new(2, 1),
        );

        let phases = warped.tiles().iter().map(Tile::phase).collect::<Vec<_>>();

        assert_eq!(
            phases,
            vec![
                Span::<Phase>::new(Time::ZERO, Time::new(1, 8)).unwrap(),
                Span::<Phase>::new(Time::new(1, 4), Time::new(3, 8)).unwrap(),
                Span::<Phase>::new(Time::new(1, 2), Time::new(5, 8)).unwrap(),
                Span::<Phase>::new(Time::new(3, 4), Time::new(7, 8)).unwrap(),
            ]
        );
    }
}
