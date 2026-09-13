//! Run: cargo run -p cadence --example melody_bass -- /tmp/cadence-melody-bass.wav
//! A generated stereo melody, bass line, pad and noise percussion; no assets/device.
use cadence::{
    adapter::audio::offline::{OfflineRenderSettings, render_wav},
    infrastructure::playback::SampleBank,
    prelude::{
        ControlKey, ControlScore, ControlTile, ControlTrack, ControlValue, Intent, PreparedScore,
        Score, SynthPreset, Tile, Time, Voice,
    },
};
use std::{error::Error, path::PathBuf, time::Duration};

fn part(preset: SynthPreset, notes: &[(i64, i64, f64)], gain: f64) -> Score {
    let length = Time::whole_number(4);
    let mut controls = vec![
        ControlTile::spanning(
            Time::ZERO,
            length,
            ControlKey::Gain,
            ControlValue::Scalar(gain),
        )
        .unwrap(),
    ];
    let tiles = notes
        .iter()
        .map(|(start, duration, pitch)| {
            let onset = Time::new(*start, 4);
            let end = Time::new(*start + *duration, 4);
            controls.push(
                ControlTile::spanning(onset, end, ControlKey::Pitch, ControlValue::Scalar(*pitch))
                    .unwrap(),
            );
            Tile::spanning(onset, end, Intent::synth_preset(preset)).unwrap()
        })
        .collect();
    Score::with_controls(
        Score::from(Voice::new(length, tiles).unwrap()),
        ControlScore::from(ControlTrack::new(length, controls).unwrap()),
    )
}

fn main() -> Result<(), Box<dyn Error>> {
    let destination = std::env::args_os()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| std::env::temp_dir().join("cadence-melody-bass.wav"));
    let melody = part(
        SynthPreset::Bass,
        &[
            (0, 1, 72.0),
            (1, 1, 75.0),
            (2, 2, 79.0),
            (4, 1, 77.0),
            (5, 1, 75.0),
            (6, 1, 72.0),
            (7, 1, 70.0),
            (8, 1, 68.0),
            (9, 1, 72.0),
            (10, 2, 75.0),
            (12, 1, 70.0),
            (13, 1, 74.0),
            (14, 2, 77.0),
        ],
        0.18,
    );
    let melody = Score::with_controls(
        melody,
        ControlScore::from(ControlTrack::new(
            Time::whole_number(4),
            vec![ControlTile::spanning(
                Time::ZERO,
                Time::whole_number(4),
                ControlKey::LowPassCutoff,
                ControlValue::Scalar(4_000.0),
            )?],
        )?),
    );
    let bass = part(
        SynthPreset::Bass,
        &[
            (0, 2, 36.0),
            (3, 1, 43.0),
            (4, 2, 41.0),
            (7, 1, 36.0),
            (8, 2, 32.0),
            (11, 1, 39.0),
            (12, 2, 34.0),
            (15, 1, 41.0),
        ],
        0.24,
    );
    let pad = part(
        SynthPreset::Pad,
        &[(0, 4, 55.0), (4, 4, 56.0), (8, 4, 51.0), (12, 4, 53.0)],
        0.12,
    );
    let percussion_notes: Vec<_> = (0..16).map(|step| (step, 1, 60.0)).collect();
    let percussion = part(SynthPreset::Percussion, &percussion_notes, 0.11);
    let score = Score::merge(vec![melody, bass, pad, percussion]);
    let cps = Time::new(1, 2); // 120 BPM with four beats per cycle.
    let wav = render_wav(
        PreparedScore::new(score).expect("example score is structurally valid"),
        SampleBank::new(),
        OfflineRenderSettings::new(48_000, Duration::from_secs(16), cps, 800_000),
    )?;
    std::fs::write(&destination, wav)?;
    println!("Wrote {}", destination.display());
    println!("120 BPM: filtered bass, brighter melody, slow pad, deterministic noise percussion.");
    Ok(())
}
