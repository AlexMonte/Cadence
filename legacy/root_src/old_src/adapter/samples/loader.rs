#[cfg(target_arch = "wasm32")]
use std::io::Cursor;
#[cfg(not(target_arch = "wasm32"))]
use std::{fs::File, path::Path};

use cadence_core::infrastructure::playback::{Frame, SampleBuffer};
use symphonia::{
    core::{
        audio::{AudioBuffer, AudioBufferRef, Signal},
        codecs::DecoderOptions,
        conv::{FromSample, IntoSample},
        errors::Error as SymphoniaError,
        io::MediaSourceStream,
        meta::MetadataOptions,
        probe::Hint,
        sample::Sample,
    },
    default::{get_codecs, get_probe},
};
use thiserror::Error;

#[derive(Debug, Error)]
pub(crate) enum LoadSampleError {
    #[error("failed to open sample file: {0}")]
    Io(#[from] std::io::Error),
    #[error("failed to probe sample file: {0}")]
    Probe(#[from] symphonia::core::errors::Error),
    #[error("sample file does not expose a default track")]
    NoDefaultTrack,
    #[error("sample file does not expose a sample rate")]
    UnknownSampleRate,
    #[error("unsupported channel configuration")]
    UnsupportedChannelConfiguration,
}

#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn load_sample(path: impl AsRef<Path>) -> Result<SampleBuffer, LoadSampleError> {
    let path = path.as_ref();
    let file = File::open(path)?;
    let media_source = MediaSourceStream::new(Box::new(file), Default::default());
    let mut hint = Hint::new();
    if let Some(extension) = path.extension().and_then(|ext| ext.to_str()) {
        hint.with_extension(extension);
    }

    load_sample_from_media_source(media_source, hint)
}

#[cfg(target_arch = "wasm32")]
pub(crate) fn load_sample_bytes(
    bytes: impl Into<Vec<u8>>,
    extension_hint: Option<&str>,
) -> Result<SampleBuffer, LoadSampleError> {
    let media_source =
        MediaSourceStream::new(Box::new(Cursor::new(bytes.into())), Default::default());
    let mut hint = Hint::new();
    if let Some(extension) = extension_hint {
        hint.with_extension(extension);
    }
    load_sample_from_media_source(media_source, hint)
}

fn load_sample_from_media_source(
    media_source: MediaSourceStream,
    hint: Hint,
) -> Result<SampleBuffer, LoadSampleError> {
    let probed = get_probe().format(
        &hint,
        media_source,
        &Default::default(),
        &MetadataOptions::default(),
    )?;
    let mut format = probed.format;
    let track = format
        .default_track()
        .ok_or(LoadSampleError::NoDefaultTrack)?;
    let track_id = track.id;
    let codec_params = track.codec_params.clone();
    let sample_rate = codec_params
        .sample_rate
        .ok_or(LoadSampleError::UnknownSampleRate)?;
    let mut decoder = get_codecs()
        .make(&codec_params, &DecoderOptions::default())
        .map_err(LoadSampleError::Probe)?;
    let mut frames = Vec::new();

    loop {
        let packet = match format.next_packet() {
            Ok(packet) => packet,
            Err(SymphoniaError::IoError(error))
                if error.kind() == std::io::ErrorKind::UnexpectedEof =>
            {
                break;
            }
            Err(error) => return Err(LoadSampleError::Probe(error)),
        };

        if packet.track_id() != track_id {
            continue;
        }

        let decoded = decoder.decode(&packet).map_err(LoadSampleError::Probe)?;
        frames.extend(load_frames_from_buffer_ref(&decoded)?);
    }

    Ok(SampleBuffer::new(sample_rate, frames))
}

fn load_frames_from_buffer_ref(buffer: &AudioBufferRef<'_>) -> Result<Vec<Frame>, LoadSampleError> {
    match buffer {
        AudioBufferRef::U8(buffer) => load_frames_from_buffer(buffer),
        AudioBufferRef::U16(buffer) => load_frames_from_buffer(buffer),
        AudioBufferRef::U24(buffer) => load_frames_from_buffer(buffer),
        AudioBufferRef::U32(buffer) => load_frames_from_buffer(buffer),
        AudioBufferRef::S8(buffer) => load_frames_from_buffer(buffer),
        AudioBufferRef::S16(buffer) => load_frames_from_buffer(buffer),
        AudioBufferRef::S24(buffer) => load_frames_from_buffer(buffer),
        AudioBufferRef::S32(buffer) => load_frames_from_buffer(buffer),
        AudioBufferRef::F32(buffer) => load_frames_from_buffer(buffer),
        AudioBufferRef::F64(buffer) => load_frames_from_buffer(buffer),
    }
}

fn load_frames_from_buffer<S>(buffer: &AudioBuffer<S>) -> Result<Vec<Frame>, LoadSampleError>
where
    S: Sample,
    f32: FromSample<S>,
{
    match buffer.spec().channels.count() {
        1 => Ok(buffer
            .chan(0)
            .iter()
            .map(|sample| Frame::from_mono((*sample).into_sample()))
            .collect()),
        2 => Ok(buffer
            .chan(0)
            .iter()
            .zip(buffer.chan(1).iter())
            .map(|(left, right)| Frame::new((*left).into_sample(), (*right).into_sample()))
            .collect()),
        _ => Err(LoadSampleError::UnsupportedChannelConfiguration),
    }
}
