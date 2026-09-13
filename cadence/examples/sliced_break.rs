//! Run: cargo run -p cadence --example sliced_break -- /tmp/cadence-sliced-break.wav
//! No device, downloaded samples or app is required: this writes a stereo WAV.
use cadence::{
    adapter::audio::offline::{OfflineRenderSettings, render_wav},
    infrastructure::playback::{Frame, SampleBank, SampleBuffer},
    prelude::{
        ControlKey, ControlScore, ControlTile, ControlTrack, ControlValue, Intent, PreparedScore,
        SampleIntent, Score, Tile, Time, Voice,
    },
};
use std::{error::Error, f64::consts::TAU, path::PathBuf, time::Duration};

const RATE: u32 = 48_000;

fn generated_break() -> SampleBuffer {
    let mut frames = vec![Frame::ZERO; RATE as usize * 2];
    let mut random = 0x53a9_17b5_u32;
    for (index, frame) in frames.iter_mut().enumerate() {
        let slice = index / 6000;
        let local = index % 6000;
        let t = local as f64 / f64::from(RATE);
        random ^= random << 13;
        random ^= random >> 17;
        random ^= random << 5;
        let noise = f64::from(random) / f64::from(u32::MAX) * 2.0 - 1.0;
        let edge = (local as f64 / 64.0).min(1.0) * ((5999 - local) as f64 / 64.0).min(1.0);
        let hit = if [0, 6, 8, 11].contains(&slice) {
            let phase = TAU * (46.0 * t + 8.0 * (1.0 - (-32.0 * t).exp()));
            phase.sin() * (-28.0 * t).exp() * 0.65
        } else if [4, 12].contains(&slice) {
            (noise * 0.45 + (TAU * 180.0 * t).sin() * 0.18) * (-32.0 * t).exp()
        } else {
            noise * (-90.0 * t).exp() * if slice % 2 == 0 { 0.17 } else { 0.1 }
        } * edge;
        let spread = if slice % 2 == 0 { 0.85 } else { 1.0 };
        *frame = Frame::new((hit * spread) as f32, (hit * (1.85 - spread)) as f32);
    }
    SampleBuffer::new(RATE, frames)
}

fn main() -> Result<(), Box<dyn Error>> {
    let destination = std::env::args_os()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| std::env::temp_dir().join("cadence-sliced-break.wav"));
    let order = [
        0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 0, 7, 6, 3, 12, 5, 12, 15, 8, 9, 6,
        6, 4, 14, 13, 15,
    ];
    let mut tiles = Vec::new();
    let mut controls = vec![ControlTile::spanning(
        Time::ZERO,
        Time::whole_number(2),
        ControlKey::Fit,
        ControlValue::Bool(true),
    )?];
    for (slot, slice) in order.into_iter().enumerate() {
        let start = Time::new(slot as i64, 16);
        let end = Time::new(slot as i64 + 1, 16);
        let reversed = matches!(slot, 19 | 23 | 30 | 31);
        tiles.push(
            Tile::spanning(
                start,
                end,
                Intent::Sample(
                    SampleIntent::new("generated-break")
                        .slice(slice, 16)?
                        .reverse(reversed),
                ),
            )
            .ok_or("generated slice slot is invalid")?,
        );
        if matches!(slot, 21 | 26 | 27) {
            controls.push(ControlTile::spanning(
                start,
                end,
                ControlKey::PlaybackRate,
                ControlValue::Scalar(1.5),
            )?);
        }
        if slot >= 28 {
            controls.push(ControlTile::spanning(
                start,
                end,
                ControlKey::Transpose,
                ControlValue::Scalar(-5.0),
            )?);
        }
    }
    let score = Score::with_controls(
        Score::from(
            Voice::new(Time::whole_number(2), tiles).ok_or("generated break voice is invalid")?,
        ),
        ControlScore::from(ControlTrack::new(Time::whole_number(2), controls)?),
    );
    let samples = SampleBank::new();
    samples.load("generated-break", generated_break());
    let cps = Time::new(3, 5); // 144 BPM with four beats per cycle.
    let wav = render_wav(
        PreparedScore::new(score).expect("example score is structurally valid"),
        samples,
        OfflineRenderSettings::new(
            RATE,
            Duration::from_secs_f64(8.0 / cps.value()),
            cps,
            1_000_000,
        ),
    )?;
    std::fs::write(&destination, wav)?;
    println!("Wrote {}", destination.display());
    println!("16 generated slices; second bar rearranges, reverses and repitches them.");
    println!(
        "Fit uses rate 1.2 at 144 BPM (about +3.16 semitones). Later rate/pitch changes also change source duration."
    );
    Ok(())
}
