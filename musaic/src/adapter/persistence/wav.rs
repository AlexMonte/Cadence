//! Bounded WAV decoding on the control thread, before an import becomes visible.
//! This first import path accepts RIFF PCM and IEEE-float mono/stereo WAV only.

use std::{io::Cursor, sync::Arc};

use cadence::{adapter::audio::Frame, infrastructure::audio::SampleBuffer};
use symphonia::core::{
    audio::SampleBuffer as InterleavedBuffer, codecs::DecoderOptions, errors::Error as DecodeError,
    formats::FormatOptions, io::MediaSourceStream, meta::MetadataOptions, probe::Hint,
};

use super::PersistenceError;

pub(super) const MAX_WAV_BYTES: usize = 64 * 1024 * 1024;
pub(super) const MAX_WAV_FRAMES: u64 = 16 * 1024 * 1024;

#[derive(Debug, Clone, Copy)]
struct WavLayout {
    sample_rate: u32,
    channels: u16,
    frame_count: u64,
}

pub(super) struct DecodedWav {
    pub buffer: SampleBuffer,
    pub channels: u16,
}

fn invalid(message: impl Into<String>) -> PersistenceError {
    PersistenceError::Sample(message.into())
}

/// Reject truncated files and unreasonable declared sizes before decoder allocation.
fn layout(bytes: &[u8]) -> Result<WavLayout, PersistenceError> {
    if bytes.len() > MAX_WAV_BYTES {
        return Err(invalid("WAV import is limited to 64 MiB per sample"));
    }
    if bytes.len() < 12 || &bytes[..4] != b"RIFF" || &bytes[8..12] != b"WAVE" {
        return Err(invalid(
            "Only RIFF WAV files are supported by sample import",
        ));
    }
    let end = u32::from_le_bytes(bytes[4..8].try_into().unwrap()) as usize + 8;
    if end < 12 || end > bytes.len() {
        return Err(invalid("WAV is truncated or has an invalid RIFF length"));
    }
    let mut position = 12_usize;
    let mut format = None;
    let mut data_length = None;
    while position < end {
        if end - position < 8 {
            return Err(invalid("WAV has a truncated chunk header"));
        }
        let tag = &bytes[position..position + 4];
        let length =
            u32::from_le_bytes(bytes[position + 4..position + 8].try_into().unwrap()) as usize;
        position += 8;
        if length > end - position {
            return Err(invalid("WAV has a truncated chunk"));
        }
        let chunk = &bytes[position..position + length];
        if tag == b"fmt " {
            if format.is_some() || length < 16 {
                return Err(invalid("WAV has a missing or duplicate format description"));
            }
            let encoding = u16::from_le_bytes(chunk[..2].try_into().unwrap());
            let channels = u16::from_le_bytes(chunk[2..4].try_into().unwrap());
            let rate = u32::from_le_bytes(chunk[4..8].try_into().unwrap());
            let byte_rate = u32::from_le_bytes(chunk[8..12].try_into().unwrap());
            let alignment = u16::from_le_bytes(chunk[12..14].try_into().unwrap());
            let bits = u16::from_le_bytes(chunk[14..16].try_into().unwrap());
            if !matches!((encoding, bits), (1, 8 | 16 | 24 | 32) | (3, 32 | 64)) {
                return Err(invalid(
                    "WAV must contain PCM 8/16/24/32-bit or float 32/64-bit audio",
                ));
            }
            if !(1..=2).contains(&channels) {
                return Err(invalid(
                    "WAV import currently supports mono and stereo audio",
                ));
            }
            if !(1..=384_000).contains(&rate)
                || alignment != channels * (bits / 8)
                || byte_rate as u64 != rate as u64 * alignment as u64
            {
                return Err(invalid(
                    "WAV has inconsistent sample rate or frame alignment",
                ));
            }
            format = Some((channels, rate, alignment));
        } else if tag == b"data" && data_length.replace(length).is_some() {
            return Err(invalid(
                "WAV files with multiple data chunks are not supported",
            ));
        }
        position += length;
        if length % 2 != 0 {
            position += 1;
            if position > end {
                return Err(invalid("WAV is missing chunk padding"));
            }
        }
    }
    let (channels, sample_rate, alignment) =
        format.ok_or_else(|| invalid("WAV has no format chunk"))?;
    let data_length = data_length.ok_or_else(|| invalid("WAV has no audio data"))?;
    if data_length == 0 || data_length % alignment as usize != 0 {
        return Err(invalid("WAV contains empty or incomplete audio frames"));
    }
    let frame_count = (data_length / alignment as usize) as u64;
    if frame_count > MAX_WAV_FRAMES {
        return Err(invalid(
            "WAV import is limited to 16 million frames per sample",
        ));
    }
    Ok(WavLayout {
        sample_rate,
        channels,
        frame_count,
    })
}

pub(super) fn decode_wav(bytes: Arc<[u8]>) -> Result<DecodedWav, PersistenceError> {
    let expected = layout(&bytes)?;
    let stream = MediaSourceStream::new(Box::new(Cursor::new(bytes)), Default::default());
    let mut hint = Hint::new();
    hint.with_extension("wav");
    let probed = symphonia::default::get_probe()
        .format(
            &hint,
            stream,
            &FormatOptions::default(),
            &MetadataOptions::default(),
        )
        .map_err(|error| invalid(format!("Cannot read WAV: {error}")))?;
    let mut format = probed.format;
    let track = format
        .default_track()
        .ok_or_else(|| invalid("WAV has no audio track"))?;
    let track_id = track.id;
    let mut decoder = symphonia::default::get_codecs()
        .make(&track.codec_params, &DecoderOptions::default())
        .map_err(|error| invalid(format!("Cannot decode WAV: {error}")))?;
    let mut frames = Vec::with_capacity(expected.frame_count as usize);
    loop {
        let packet = match format.next_packet() {
            Ok(packet) => packet,
            Err(DecodeError::IoError(error))
                if error.kind() == std::io::ErrorKind::UnexpectedEof =>
            {
                break;
            }
            Err(error) => return Err(invalid(format!("Cannot read WAV packet: {error}"))),
        };
        if packet.track_id() != track_id {
            continue;
        }
        let decoded = decoder
            .decode(&packet)
            .map_err(|error| invalid(format!("Cannot decode WAV packet: {error}")))?;
        let spec = *decoded.spec();
        if spec.rate != expected.sample_rate || spec.channels.count() != expected.channels as usize
        {
            return Err(invalid("WAV format changed while decoding"));
        }
        if frames.len().saturating_add(decoded.frames()) > expected.frame_count as usize {
            return Err(invalid("WAV contains more frames than declared"));
        }
        let mut interleaved = InterleavedBuffer::<f32>::new(decoded.capacity() as u64, spec);
        interleaved.copy_interleaved_ref(decoded);
        for frame in interleaved
            .samples()
            .chunks_exact(expected.channels as usize)
        {
            if frame.iter().any(|sample| !sample.is_finite()) {
                return Err(invalid("WAV contains non-finite audio samples"));
            }
            frames.push(Frame {
                left: frame[0],
                right: *frame.get(1).unwrap_or(&frame[0]),
            });
        }
    }
    if frames.len() as u64 != expected.frame_count {
        return Err(invalid(
            "WAV ended before every declared audio frame was decoded",
        ));
    }
    Ok(DecodedWav {
        buffer: SampleBuffer::new(expected.sample_rate, frames),
        channels: expected.channels,
    })
}

/// Decode compressed bank audio once on the worker to a portable float WAV.
/// Gapless trimming preserves the frame origin used by preset loop points.
pub(super) fn transcode_bank_audio(bytes: Arc<[u8]>) -> Result<Arc<[u8]>, PersistenceError> {
    if bytes.starts_with(b"RIFF") {
        return Ok(bytes);
    }
    let stream = MediaSourceStream::new(Box::new(Cursor::new(bytes)), Default::default());
    let probed = symphonia::default::get_probe()
        .format(
            &Hint::new(),
            stream,
            &FormatOptions {
                enable_gapless: true,
                ..Default::default()
            },
            &MetadataOptions::default(),
        )
        .map_err(|error| invalid(format!("Cannot read instrument audio: {error}")))?;
    let mut format = probed.format;
    let track = format
        .default_track()
        .ok_or_else(|| invalid("Instrument recording has no audio track"))?;
    let track_id = track.id;
    let mut decoder = symphonia::default::get_codecs()
        .make(&track.codec_params, &DecoderOptions::default())
        .map_err(|error| invalid(format!("Cannot decode instrument audio: {error}")))?;
    let mut payload = Vec::new();
    let mut first_spec = None;
    loop {
        let packet = match format.next_packet() {
            Ok(packet) => packet,
            Err(DecodeError::IoError(error))
                if error.kind() == std::io::ErrorKind::UnexpectedEof =>
            {
                break;
            }
            Err(error) => {
                return Err(invalid(format!(
                    "Cannot read instrument audio packet: {error}"
                )));
            }
        };
        if packet.track_id() != track_id {
            continue;
        }
        let decoded = decoder
            .decode(&packet)
            .map_err(|error| invalid(format!("Cannot decode instrument audio packet: {error}")))?;
        let spec = *decoded.spec();
        if !(1..=384_000).contains(&spec.rate) || !(1..=2).contains(&spec.channels.count()) {
            return Err(invalid(
                "Instrument audio must be mono/stereo at 1 to 384000 Hz",
            ));
        }
        if first_spec
            .replace(spec)
            .is_some_and(|previous| previous != spec)
        {
            return Err(invalid("Instrument audio format changed while decoding"));
        }
        let extra = decoded
            .frames()
            .saturating_mul(spec.channels.count())
            .saturating_mul(4);
        if payload.len().saturating_add(extra) > MAX_WAV_BYTES - 44 {
            return Err(invalid("Decoded instrument WAV exceeds 64 MiB"));
        }
        let mut samples = InterleavedBuffer::<f32>::new(decoded.capacity() as u64, spec);
        samples.copy_interleaved_ref(decoded);
        for value in samples.samples() {
            if !value.is_finite() {
                return Err(invalid("Instrument audio contains non-finite samples"));
            }
            payload.extend_from_slice(&value.to_le_bytes());
        }
    }
    let spec = first_spec.ok_or_else(|| invalid("Instrument audio contains no decoded frames"))?;
    let channels = spec.channels.count() as u16;
    let length = payload.len() as u32;
    let mut wav = Vec::with_capacity(payload.len() + 44);
    wav.extend_from_slice(b"RIFF");
    wav.extend_from_slice(&(36 + length).to_le_bytes());
    wav.extend_from_slice(b"WAVEfmt ");
    wav.extend_from_slice(&16_u32.to_le_bytes());
    wav.extend_from_slice(&3_u16.to_le_bytes());
    wav.extend_from_slice(&channels.to_le_bytes());
    wav.extend_from_slice(&spec.rate.to_le_bytes());
    wav.extend_from_slice(&(spec.rate * u32::from(channels) * 4).to_le_bytes());
    wav.extend_from_slice(&(channels * 4).to_le_bytes());
    wav.extend_from_slice(&32_u16.to_le_bytes());
    wav.extend_from_slice(b"data");
    wav.extend_from_slice(&length.to_le_bytes());
    wav.extend_from_slice(&payload);
    Ok(wav.into())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mono_wav(encoding: u16, bits: u16, data: &[u8]) -> Arc<[u8]> {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(b"RIFF");
        bytes.extend_from_slice(&(36_u32 + data.len() as u32).to_le_bytes());
        bytes.extend_from_slice(b"WAVEfmt ");
        bytes.extend_from_slice(&16_u32.to_le_bytes());
        bytes.extend_from_slice(&encoding.to_le_bytes());
        bytes.extend_from_slice(&1_u16.to_le_bytes());
        bytes.extend_from_slice(&8_000_u32.to_le_bytes());
        bytes.extend_from_slice(&(8_000_u32 * u32::from(bits / 8)).to_le_bytes());
        bytes.extend_from_slice(&(bits / 8).to_le_bytes());
        bytes.extend_from_slice(&bits.to_le_bytes());
        bytes.extend_from_slice(b"data");
        bytes.extend_from_slice(&(data.len() as u32).to_le_bytes());
        bytes.extend_from_slice(data);
        bytes.into()
    }

    #[test]
    fn supported_pcm_and_float_formats_preserve_signed_amplitudes() {
        let cases = [
            (1, 8, vec![64, 160]),
            (
                1,
                16,
                [-16_384_i16, 8_192]
                    .into_iter()
                    .flat_map(i16::to_le_bytes)
                    .collect(),
            ),
            (
                1,
                24,
                [-4_194_304_i32, 2_097_152]
                    .into_iter()
                    .flat_map(|value| value.to_le_bytes()[..3].to_vec())
                    .collect(),
            ),
            (
                1,
                32,
                [-1_073_741_824_i32, 536_870_912]
                    .into_iter()
                    .flat_map(i32::to_le_bytes)
                    .collect(),
            ),
            (
                3,
                32,
                [-0.5_f32, 0.25]
                    .into_iter()
                    .flat_map(f32::to_le_bytes)
                    .collect(),
            ),
            (
                3,
                64,
                [-0.5_f64, 0.25]
                    .into_iter()
                    .flat_map(f64::to_le_bytes)
                    .collect(),
            ),
        ];
        for (encoding, bits, data) in cases {
            let decoded = decode_wav(mono_wav(encoding, bits, &data)).unwrap();
            assert_eq!(
                decoded.buffer.frames(),
                &[Frame::from_mono(-0.5), Frame::from_mono(0.25)],
                "encoding={encoding} bits={bits}"
            );
        }
    }

    #[test]
    fn non_finite_float_sample_is_rejected() {
        let data = [0.0_f32, f32::NAN]
            .into_iter()
            .flat_map(f32::to_le_bytes)
            .collect::<Vec<_>>();
        assert!(decode_wav(mono_wav(3, 32, &data)).is_err());
    }
}
