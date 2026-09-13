//! Deterministic stereo WAV export using the same runtime and renderer as live
//! playback. The caller supplies decoded samples; no filesystem is accessed.
use super::{AudioRenderer, AudioRendererError, AudioRendererSettings, Frame};
use crate::{
    domain::rational::Time,
    infrastructure::{
        PreparedScore,
        playback::{PlaybackError, PlaybackRuntime, PlaybackSettings, SampleBank},
    },
};
use std::time::Duration;
use thiserror::Error;

// PCM output is bounded independently of caller-provided settings.
const MAX_EXPORT_BYTES: usize = 256 * 1024 * 1024;

#[derive(Debug, Clone, Copy)]
/// Explicit output and work bounds for a WAV render.
pub struct OfflineRenderSettings {
    /// Output sample rate, 8–192 kHz.
    pub sample_rate: u32,
    /// Requested exact duration, rounded to the nearest output frame.
    pub duration: Duration,
    /// Musical cycles per second, positive and at most 256.
    pub cps: Time,
    /// Maximum frames the caller authorizes allocating for this export.
    pub max_frames: usize,
    /// Maximum worker block size (1–8192 frames); large blocks are subdivided for scheduling.
    pub block_frames: usize,
}
impl OfflineRenderSettings {
    /// Creates settings with 256-frame worker blocks and a caller-selected frame budget.
    pub fn new(sample_rate: u32, duration: Duration, cps: Time, max_frames: usize) -> Self {
        Self {
            sample_rate,
            duration,
            cps,
            max_frames,
            block_frames: 256,
        }
    }
}

#[derive(Debug, Error)]
/// A WAV export request could not be rendered within its declared bounds.
pub enum OfflineRenderError {
    /// Invalid rate, tempo or block size.
    #[error("invalid offline settings: {0}")]
    InvalidSettings(&'static str),
    /// The duration exceeds the caller's frame limit or the 256 MiB export limit.
    #[error("WAV export exceeds the requested frame budget or the 256 MiB output limit")]
    FrameBudget,
    /// The operating system could not reserve the bounded output buffer.
    #[error("could not allocate the WAV export buffer")]
    Allocation,
    /// The renderer could not initialize.
    #[error(transparent)]
    Renderer(#[from] AudioRendererError),
    /// Score scheduling or control validation failed.
    #[error(transparent)]
    Playback(#[from] PlaybackError),
    /// A configured voice or effect resource limit would change the result.
    #[error("WAV export exceeded audio voice or effect limits")]
    AudioResources,
    /// A DSP result was not finite.
    #[error("WAV export produced a non-finite sample")]
    NonFiniteSample,
}

/// Renders precisely the requested duration as stereo signed 16-bit PCM WAV.
/// Effects continue within that duration; append desired tail time to `duration`.
/// Peak values outside [-1, 1] are clipped during PCM conversion.
pub fn render_wav(
    score: PreparedScore,
    samples: SampleBank,
    settings: OfflineRenderSettings,
) -> Result<Vec<u8>, OfflineRenderError> {
    let frames = frame_count(&settings)?;
    let bytes = frames
        .checked_mul(4)
        .and_then(|bytes| bytes.checked_add(44))
        .ok_or(OfflineRenderError::FrameBudget)?;
    if bytes > MAX_EXPORT_BYTES {
        return Err(OfflineRenderError::FrameBudget);
    }
    let mut wav = Vec::new();
    wav.try_reserve_exact(bytes)
        .map_err(|_| OfflineRenderError::Allocation)?;
    wav.extend_from_slice(b"RIFF");
    wav.extend_from_slice(&((bytes - 8) as u32).to_le_bytes());
    wav.extend_from_slice(b"WAVEfmt ");
    wav.extend_from_slice(&16_u32.to_le_bytes());
    wav.extend_from_slice(&1_u16.to_le_bytes());
    wav.extend_from_slice(&2_u16.to_le_bytes());
    wav.extend_from_slice(&settings.sample_rate.to_le_bytes());
    wav.extend_from_slice(&(settings.sample_rate * 4).to_le_bytes());
    wav.extend_from_slice(&4_u16.to_le_bytes());
    wav.extend_from_slice(&16_u16.to_le_bytes());
    wav.extend_from_slice(b"data");
    wav.extend_from_slice(&((frames * 4) as u32).to_le_bytes());
    render_blocks(score, samples, settings, frames, |block| {
        for frame in block {
            for value in [frame.left, frame.right] {
                wav.extend_from_slice(&pcm16(value).to_le_bytes());
            }
        }
    })?;
    Ok(wav)
}

/// Renders unclipped stereo frames through the same scheduling/DSP path as WAV
/// export. Allocation is bounded by the requested frame count and 256 MiB.
pub fn render_buffer(
    score: PreparedScore,
    samples: SampleBank,
    settings: OfflineRenderSettings,
) -> Result<super::SampleBuffer, OfflineRenderError> {
    let count = frame_count(&settings)?;
    if count > MAX_EXPORT_BYTES / std::mem::size_of::<Frame>() {
        return Err(OfflineRenderError::FrameBudget);
    }
    let mut frames = Vec::new();
    frames
        .try_reserve_exact(count)
        .map_err(|_| OfflineRenderError::Allocation)?;
    render_blocks(score, samples, settings, count, |block| {
        frames.extend_from_slice(block)
    })?;
    Ok(super::SampleBuffer::new(settings.sample_rate, frames))
}

fn frame_count(settings: &OfflineRenderSettings) -> Result<usize, OfflineRenderError> {
    if !(8_000..=192_000).contains(&settings.sample_rate) {
        return Err(OfflineRenderError::InvalidSettings(
            "sample rate must be 8–192 kHz",
        ));
    }
    if settings.cps <= Time::ZERO || settings.cps > Time::whole_number(256) {
        return Err(OfflineRenderError::InvalidSettings(
            "cycles per second must be positive and at most 256",
        ));
    }
    if !(1..=8192).contains(&settings.block_frames) {
        return Err(OfflineRenderError::InvalidSettings(
            "block size must be 1–8192 frames",
        ));
    }
    let count = (settings
        .duration
        .as_nanos()
        .checked_mul(u128::from(settings.sample_rate))
        .ok_or(OfflineRenderError::FrameBudget)?
        + 500_000_000)
        / 1_000_000_000;
    let frames = usize::try_from(count).map_err(|_| OfflineRenderError::FrameBudget)?;
    if frames > settings.max_frames {
        return Err(OfflineRenderError::FrameBudget);
    }
    Ok(frames)
}
fn render_blocks(
    score: PreparedScore,
    samples: SampleBank,
    settings: OfflineRenderSettings,
    frames: usize,
    mut emit: impl FnMut(&[Frame]),
) -> Result<(), OfflineRenderError> {
    let (audio, mut renderer) =
        AudioRenderer::split(AudioRendererSettings::new(settings.sample_rate, 4096))?;
    // Keep each scheduling/render step within 1/16 cycle regardless of tempo.
    let block_frames = settings.block_frames.min(
        (f64::from(settings.sample_rate) / (settings.cps.value() * 16.0))
            .floor()
            .max(1.0) as usize,
    );
    let mut runtime = PlaybackRuntime::with_sample_bank(
        PlaybackSettings {
            cps: settings.cps,
            look_ahead: Time::new(1, 4),
            step: Time::new(1, 64),
        },
        audio.clone(),
        samples,
    );
    runtime.play_prepared_score(score)?;
    let mut block = vec![Frame::ZERO; block_frames];
    let mut remaining = frames;
    while remaining > 0 {
        runtime.tick()?;
        let count = remaining.min(block_frames);
        renderer.render(&mut block[..count]);
        if audio.resource_limit_count() > 0 {
            return Err(OfflineRenderError::AudioResources);
        }
        for frame in &block[..count] {
            for value in [frame.left, frame.right] {
                if !value.is_finite() {
                    return Err(OfflineRenderError::NonFiniteSample);
                }
            }
        }
        emit(&block[..count]);
        remaining -= count;
    }
    Ok(())
}
fn pcm16(value: f32) -> i16 {
    (value.clamp(-1.0, 1.0) * 32768.0)
        .round()
        .clamp(-32768.0, 32767.0) as i16
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        adapter::audio::SampleBuffer,
        domain::{
            intent::Intent,
            score::Score,
            voice::{Tile, Voice},
        },
    };
    fn score() -> Score {
        Score::from(
            Voice::new(
                Time::ONE,
                vec![Tile::spanning(Time::ZERO, Time::new(1, 2), Intent::sample("test")).unwrap()],
            )
            .unwrap(),
        )
    }
    fn bank() -> SampleBank {
        let bank = SampleBank::new();
        bank.load(
            "test",
            SampleBuffer::new(8_000, vec![Frame::new(0.25, -0.5); 1000]),
        );
        bank
    }
    #[test]
    fn wav_header_stereo_and_pcm_agree_with_direct_playback() {
        let settings =
            OfflineRenderSettings::new(8_000, Duration::from_millis(100), Time::ONE, 800);
        let prepared = PreparedScore::new(score()).unwrap();
        let wav = render_wav(prepared.clone(), bank(), settings).unwrap();
        assert_eq!(&wav[..4], b"RIFF");
        assert_eq!(&wav[8..16], b"WAVEfmt ");
        assert_eq!(u16::from_le_bytes(wav[22..24].try_into().unwrap()), 2);
        assert_eq!(u32::from_le_bytes(wav[24..28].try_into().unwrap()), 8000);
        assert_eq!(u32::from_le_bytes(wav[40..44].try_into().unwrap()), 3200);
        assert_eq!(wav.len(), 3244);
        let (audio, mut renderer) =
            AudioRenderer::split(AudioRendererSettings::new(8000, 4096)).unwrap();
        let mut runtime = PlaybackRuntime::with_sample_bank(
            PlaybackSettings {
                cps: Time::ONE,
                look_ahead: Time::ONE,
                step: Time::new(1, 64),
            },
            audio,
            bank(),
        );
        runtime.play_prepared_score(prepared).unwrap();
        runtime.tick().unwrap();
        let mut frames = [Frame::ZERO; 800];
        renderer.render(&mut frames);
        let expected: Vec<u8> = frames
            .iter()
            .flat_map(|frame| [pcm16(frame.left), pcm16(frame.right)])
            .flat_map(i16::to_le_bytes)
            .collect();
        assert_eq!(&wav[44..], expected);
        assert!(frames[100].left > 0.0 && frames[100].right < 0.0);
    }
    #[test]
    fn float_buffer_matches_wav_quantization_and_duration() {
        let settings =
            OfflineRenderSettings::new(8_000, Duration::from_millis(100), Time::ONE, 800);
        let clip = render_buffer(PreparedScore::new(score()).unwrap(), bank(), settings).unwrap();
        let wav = render_wav(PreparedScore::new(score()).unwrap(), bank(), settings).unwrap();
        assert_eq!(clip.len(), 800);
        assert_eq!(clip.sample_rate(), 8_000);
        let pcm: Vec<u8> = clip
            .frames()
            .iter()
            .flat_map(|f| [pcm16(f.left), pcm16(f.right)])
            .flat_map(i16::to_le_bytes)
            .collect();
        assert_eq!(&wav[44..], pcm);
    }
    #[test]
    fn export_rejects_invalid_and_over_budget_requests_before_rendering() {
        let settings =
            OfflineRenderSettings::new(48_000, Duration::from_secs(3600), Time::ONE, 1000);
        assert!(matches!(
            render_wav(
                PreparedScore::new(Score::empty()).unwrap(),
                bank(),
                settings
            ),
            Err(OfflineRenderError::FrameBudget)
        ));
        let settings = OfflineRenderSettings {
            sample_rate: 0,
            ..settings
        };
        assert!(matches!(
            render_wav(
                PreparedScore::new(Score::empty()).unwrap(),
                bank(),
                settings
            ),
            Err(OfflineRenderError::InvalidSettings(_))
        ));
    }
    #[test]
    fn pcm_is_identical_across_worker_block_sizes() {
        let settings = OfflineRenderSettings::new(8000, Duration::from_millis(100), Time::ONE, 800);
        let reference = render_wav(PreparedScore::new(score()).unwrap(), bank(), settings).unwrap();
        for block_frames in [1, 37, 8192] {
            assert_eq!(
                render_wav(
                    PreparedScore::new(score()).unwrap(),
                    bank(),
                    OfflineRenderSettings {
                        block_frames,
                        ..settings
                    }
                )
                .unwrap(),
                reference
            );
        }
    }
    #[test]
    fn missing_samples_are_an_export_error_not_silent_success() {
        let settings = OfflineRenderSettings::new(8000, Duration::from_millis(10), Time::ONE, 80);
        let error = render_wav(
            PreparedScore::new(score()).unwrap(),
            SampleBank::new(),
            settings,
        )
        .unwrap_err();
        assert!(error.to_string().contains("sample 'test' is not loaded"));
    }
}
