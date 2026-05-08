#[cfg(not(target_arch = "wasm32"))]
mod imp {
    use cpal::{
        FromSample, Sample, SampleFormat, SizedSample, Stream,
        traits::{DeviceTrait, HostTrait, StreamTrait},
    };
    use thiserror::Error;

    use cadence_core::infrastructure::audio::{
        AudioControl, AudioRenderer, AudioRendererError, AudioRendererSettings, Frame,
    };

    pub(crate) struct CpalAudioBackend {
        _stream: Stream,
    }

    impl CpalAudioBackend {
        pub(crate) fn new(
            queue_capacity: usize,
        ) -> Result<(Self, AudioControl), CpalAudioBackendError> {
            let host = cpal::default_host();
            let device = host
                .default_output_device()
                .ok_or(CpalAudioBackendError::NoOutputDevice)?;
            let supported_config = device.default_output_config()?;
            let sample_format = supported_config.sample_format();
            let config = supported_config.config();
            let channels = usize::from(config.channels);
            let sample_rate = config.sample_rate.0;
            let (audio, renderer) =
                AudioRenderer::split(AudioRendererSettings::new(sample_rate, queue_capacity))?;

            let stream = match sample_format {
                SampleFormat::F32 => {
                    build_output_stream::<f32>(&device, &config, channels, renderer)?
                }
                SampleFormat::I16 => {
                    build_output_stream::<i16>(&device, &config, channels, renderer)?
                }
                SampleFormat::U16 => {
                    build_output_stream::<u16>(&device, &config, channels, renderer)?
                }
                sample_format => {
                    return Err(CpalAudioBackendError::UnsupportedSampleFormat(
                        sample_format,
                    ));
                }
            };

            stream.play()?;

            Ok((Self { _stream: stream }, audio))
        }
    }

    fn build_output_stream<T>(
        device: &cpal::Device,
        config: &cpal::StreamConfig,
        channels: usize,
        mut renderer: AudioRenderer,
    ) -> Result<Stream, cpal::BuildStreamError>
    where
        T: Sample + SizedSample + FromSample<f32>,
    {
        let mut scratch = Vec::new();
        let mut started = false;

        device.build_output_stream(
            config,
            move |data: &mut [T], _| {
                if channels == 0 {
                    return;
                }

                if !started {
                    renderer.on_start_processing();
                    started = true;
                }

                let frame_count = data.len() / channels;
                if scratch.len() != frame_count {
                    scratch.resize(frame_count, Frame::ZERO);
                }

                renderer.render(&mut scratch);

                for (frame_index, frame) in scratch.iter().enumerate() {
                    let left = frame.left.clamp(-1.0, 1.0);
                    let right = frame.right.clamp(-1.0, 1.0);
                    let base = frame_index * channels;

                    data[base] = T::from_sample(left);
                    if channels > 1 {
                        data[base + 1] = T::from_sample(right);
                    }
                    for channel in 2..channels {
                        data[base + channel] = T::from_sample((left + right) * 0.5);
                    }
                }
            },
            move |error| eprintln!("audio stream error: {error}"),
            None,
        )
    }

    #[derive(Debug, Error)]
    pub(crate) enum CpalAudioBackendError {
        #[error("no default output device available")]
        NoOutputDevice,
        #[error("failed to query default output config: {0}")]
        DefaultOutputConfig(#[from] cpal::DefaultStreamConfigError),
        #[error("failed to build output stream: {0}")]
        BuildStream(#[from] cpal::BuildStreamError),
        #[error("failed to start output stream: {0}")]
        PlayStream(#[from] cpal::PlayStreamError),
        #[error("unsupported output sample format: {0:?}")]
        UnsupportedSampleFormat(SampleFormat),
        #[error("failed to create audio renderer: {0}")]
        Renderer(#[from] AudioRendererError),
    }
}

pub(crate) use imp::CpalAudioBackend;
