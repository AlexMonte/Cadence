//! Mid-note modulation must reach PCM, not just the projected control map.
use cadence::{
    adapter::audio::{AudioRenderer, AudioRendererSettings, Frame},
    infrastructure::playback::{PlaybackRuntime, PlaybackSettings},
    prelude::{BuiltInSynthSource, Intent, PreparedScore, Time},
};
use musaic::application::pipeline::lowering::lower_tessera_ir_with_sounds;
use std::collections::BTreeMap;
use tessera::prelude::*;

fn r(n: i64, d: i64) -> Rational {
    Rational::new(n, d)
}
fn modulated(kind: TransformKind, values: Vec<ContainerSurfaceTile>) -> cadence::prelude::Score {
    let mut board = Board::new();
    board
        .at(0, 2)
        .named("pattern")
        .footprint(TileFootprint::new(2, 2))
        .sequence(notes(["c"]))
        .unwrap();
    board.at(2, 2).named("effect").transform(kind).unwrap();
    board
        .at(3, 2)
        .named("out")
        .footprint(TileFootprint::new(2, 2))
        .output()
        .unwrap();
    let values = board
        .at(2, 0)
        .named("values")
        .footprint(TileFootprint::new(2, 2))
        .sequence(values)
        .unwrap();
    board
        .bind_output_side(
            &values,
            OutputEndpoint::Socket(OutputPort::new("out")),
            SpatialSide::South,
        )
        .unwrap();
    let ir = TesseraCompiler::new()
        .compile_authored(&board.finish())
        .unwrap()
        .ir;
    let (scores, diagnostics) = lower_tessera_ir_with_sounds(
        &ir,
        &BTreeMap::from([(NodeId::new("out"), Intent::synth(BuiltInSynthSource::Sine))]),
    );
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
    scores[&NodeId::new("out")].clone()
}
fn values(a: Rational, b: Rational) -> Vec<ContainerSurfaceTile> {
    [a, b]
        .into_iter()
        .map(|value| ContainerSurfaceTile::Atom(AtomTile::Scalar(ScalarAtom { value })))
        .collect()
}
fn render(score: cadence::prelude::Score) -> Vec<Frame> {
    let (audio, mut renderer) =
        AudioRenderer::split(AudioRendererSettings::new(8000, 1024)).unwrap();
    let mut runtime = PlaybackRuntime::new(
        PlaybackSettings {
            cps: Time::ONE,
            look_ahead: Time::new(1, 4),
            step: Time::new(1, 64),
        },
        audio,
    );
    let prepared = PreparedScore::new(score).unwrap();
    runtime.play_prepared_score(prepared).unwrap();
    let mut output = vec![Frame::ZERO; 8000];
    for chunk in output.chunks_mut(64) {
        runtime.tick().unwrap();
        renderer.render(chunk);
    }
    output
}
fn energy(frames: &[Frame]) -> f64 {
    frames
        .iter()
        .map(|f| f64::from(f.left).powi(2) + f64::from(f.right).powi(2))
        .sum()
}

#[test]
fn patterned_gate_closes_an_already_playing_note() {
    let audio = render(modulated(TransformKind::Gate, values(r(1, 1), r(0, 1))));
    assert!(energy(&audio[1000..3000]) > 1.0);
    assert!(
        energy(&audio[5000..7000]) < 1e-6,
        "Gate=0 in the second half must silence the held note"
    );
}

#[test]
fn patterned_highpass_changes_an_already_playing_note() {
    let audio = render(modulated(
        TransformKind::HighPassCutoff,
        values(r(20, 1), r(2000, 1)),
    ));
    let first = energy(&audio[1000..3000]);
    let second = energy(&audio[5000..7000]);
    assert!(first > 1.0);
    assert!(
        second < first / 20.0,
        "High-pass cutoff ignored a mid-note change: first={first}, second={second}"
    );
}

#[test]
fn expression_returns_to_its_inherited_value_outside_a_control_tile() {
    use cadence::prelude::{
        ControlKey, ControlScore, ControlTile, ControlTrack, ControlValue, Score, Tile, UnitValue,
        Voice,
    };
    let note = Score::from(
        Voice::new(
            Time::ONE,
            vec![
                Tile::spanning(
                    Time::ZERO,
                    Time::ONE,
                    Intent::synth(BuiltInSynthSource::Sine),
                )
                .unwrap(),
            ],
        )
        .unwrap(),
    );
    let inherited = Score::with_controls(
        note,
        ControlScore::from(
            ControlTrack::new(
                Time::ONE,
                vec![
                    ControlTile::spanning(
                        Time::ZERO,
                        Time::ONE,
                        ControlKey::Expression,
                        ControlValue::Unipolar(UnitValue::new(0.5).unwrap()),
                    )
                    .unwrap(),
                ],
            )
            .unwrap(),
        ),
    );
    let control = ControlScore::from(
        ControlTrack::new(
            Time::ONE,
            vec![
                ControlTile::spanning(
                    Time::ZERO,
                    Time::new(1, 2),
                    ControlKey::Expression,
                    ControlValue::Unipolar(UnitValue::new(0.25).unwrap()),
                )
                .unwrap(),
            ],
        )
        .unwrap(),
    );
    let audio = render(Score::with_controls(inherited, control));
    let first = energy(&audio[1000..3000]);
    let second = energy(&audio[5000..7000]);
    assert!(
        second > first * 10.0,
        "Expression must release its multiplier after its tile: first={first}, second={second}"
    );
}

#[test]
fn patterned_gate_can_open_a_note_that_started_silent() {
    let audio = render(modulated(TransformKind::Gate, values(r(0, 1), r(1, 1))));
    assert!(energy(&audio[1000..3000]) < 1e-6);
    assert!(
        energy(&audio[5000..7000]) > 1.0,
        "A closed gate must preserve the held voice for a later opening"
    );
}

#[test]
fn numeric_control_rest_releases_expression_on_a_held_note() {
    let audio = render(modulated(
        TransformKind::Expression,
        vec![
            ContainerSurfaceTile::Atom(AtomTile::Scalar(ScalarAtom { value: r(1, 4) })),
            ContainerSurfaceTile::Atom(AtomTile::Rest),
        ],
    ));
    assert!(energy(&audio[5000..7000]) > energy(&audio[1000..3000]) * 10.0);
}
