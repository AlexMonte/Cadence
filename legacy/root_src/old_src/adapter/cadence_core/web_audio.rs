use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};

use cadence_core::infrastructure::audio::{
    AudioControl, AudioRenderer, AudioRendererError, AudioRendererSettings, Frame,
};
use js_sys::Float32Array;
use wasm_bindgen::{JsCast, closure::Closure, prelude::*};

const CHANNELS: usize = 2;
const PUMP_INTERVAL_MS: i32 = 8;

thread_local! {
    static AUDIO_BRIDGE: RefCell<Option<WebAudioBridge>> = const { RefCell::new(None) };
    static NEXT_RENDERER_ID: Cell<u64> = const { Cell::new(1) };
}

#[wasm_bindgen(inline_js = r#"
let cadenceAudioBridge = null;

function cadenceWorkletSource() {
  return `
class CadenceProcessor extends AudioWorkletProcessor {
  constructor(options) {
    super();
    const { sampleSab, indexSab, capacityFrames, channels } = options.processorOptions;
    this.samples = new Float32Array(sampleSab);
    this.indices = new Int32Array(indexSab);
    this.capacityFrames = capacityFrames;
    this.channels = channels;
  }

  process(_inputs, outputs) {
    const output = outputs[0];
    if (!output || output.length === 0) {
      return true;
    }

    const frameCount = output[0].length;
    let read = Atomics.load(this.indices, 0);
    const write = Atomics.load(this.indices, 1);

    for (let frame = 0; frame < frameCount; frame++) {
      const hasData = read !== write;
      const base = ((read % this.capacityFrames) * this.channels);
      for (let channel = 0; channel < output.length; channel++) {
        output[channel][frame] = hasData ? (this.samples[base + channel] || 0) : 0;
      }
      if (hasData) {
        read = (read + 1) % this.capacityFrames;
      }
    }

    Atomics.store(this.indices, 0, read);
    return true;
  }
}

registerProcessor("cadence-runtime-processor", CadenceProcessor);
`;
}

async function cadenceCreateAudioBridge() {
  if (cadenceAudioBridge) {
    return Math.round(cadenceAudioBridge.context.sampleRate);
  }

  if (!globalThis.crossOriginIsolated || typeof SharedArrayBuffer === "undefined") {
    throw new Error("AudioWorklet playback requires cross-origin isolation and SharedArrayBuffer support.");
  }

  if (typeof AudioWorkletNode === "undefined") {
    throw new Error("AudioWorkletNode is unavailable in this browser.");
  }

  const context = new AudioContext();
  const sampleSab = new SharedArrayBuffer(Float32Array.BYTES_PER_ELEMENT * 2 * 32768);
  const indexSab = new SharedArrayBuffer(Int32Array.BYTES_PER_ELEMENT * 2);
  const moduleBlob = new Blob([cadenceWorkletSource()], { type: "application/javascript" });
  const moduleUrl = URL.createObjectURL(moduleBlob);
  try {
    await context.audioWorklet.addModule(moduleUrl);
  } finally {
    URL.revokeObjectURL(moduleUrl);
  }

  const options = {
    numberOfInputs: 0,
    numberOfOutputs: 1,
    outputChannelCount: [2],
    processorOptions: {
      sampleSab,
      indexSab,
      capacityFrames: 32768,
      channels: 2,
    },
  };
  const node = new AudioWorkletNode(context, "cadence-runtime-processor", options);
  node.connect(context.destination);

  cadenceAudioBridge = {
    context,
    node,
    capacityFrames: 32768,
    channels: 2,
    samples: new Float32Array(sampleSab),
    indices: new Int32Array(indexSab),
    lastError: null,
  };

  node.port.onmessage = (event) => {
    if (event && event.data && event.data.type === "error") {
      cadenceAudioBridge.lastError = String(event.data.message || "audio worklet error");
    }
  };

  return Math.round(context.sampleRate);
}

export async function cadencePrepareAudioBridge() {
  return await cadenceCreateAudioBridge();
}

export async function cadenceResumeAudioBridge() {
  await cadenceCreateAudioBridge();
  await cadenceAudioBridge.context.resume();
  return Math.round(cadenceAudioBridge.context.sampleRate);
}

export function cadenceAudioBridgeCurrentTime() {
  return cadenceAudioBridge ? cadenceAudioBridge.context.currentTime : 0;
}

export function cadenceAudioBridgeSampleRate() {
  return cadenceAudioBridge ? Math.round(cadenceAudioBridge.context.sampleRate) : 0;
}

export function cadenceAudioBridgeAvailableFrames() {
  if (!cadenceAudioBridge) {
    return 0;
  }
  const read = Atomics.load(cadenceAudioBridge.indices, 0);
  const write = Atomics.load(cadenceAudioBridge.indices, 1);
  const capacity = cadenceAudioBridge.capacityFrames;
  const used = write >= read ? (write - read) : (capacity - (read - write));
  return Math.max(0, capacity - used - 1);
}

export function cadenceAudioBridgePush(samples, frameCount, channels) {
  if (!cadenceAudioBridge) {
    throw new Error("audio bridge unavailable");
  }
  if (channels !== cadenceAudioBridge.channels) {
    throw new Error("channel mismatch");
  }

  const read = Atomics.load(cadenceAudioBridge.indices, 0);
  const write = Atomics.load(cadenceAudioBridge.indices, 1);
  const capacity = cadenceAudioBridge.capacityFrames;
  const used = write >= read ? (write - read) : (capacity - (read - write));
  const free = Math.max(0, capacity - used - 1);
  const toWrite = Math.min(frameCount, free);
  const source = new Float32Array(samples.buffer, samples.byteOffset, toWrite * channels);

  for (let frame = 0; frame < toWrite; frame++) {
    const targetBase = ((write + frame) % capacity) * channels;
    const sourceBase = frame * channels;
    cadenceAudioBridge.samples[targetBase] = source[sourceBase] || 0;
    cadenceAudioBridge.samples[targetBase + 1] = source[sourceBase + 1] || 0;
  }

  Atomics.store(cadenceAudioBridge.indices, 1, (write + toWrite) % capacity);
  return toWrite;
}

export function cadenceAudioBridgeTakeError() {
  if (!cadenceAudioBridge) {
    return null;
  }
  const error = cadenceAudioBridge.lastError;
  cadenceAudioBridge.lastError = null;
  return error;
}
"#)]
extern "C" {
    #[wasm_bindgen(catch, js_name = cadencePrepareAudioBridge)]
    async fn prepare_audio_bridge_js() -> Result<JsValue, JsValue>;

    #[wasm_bindgen(catch, js_name = cadenceResumeAudioBridge)]
    async fn resume_audio_bridge_js() -> Result<JsValue, JsValue>;

    #[wasm_bindgen(js_name = cadenceAudioBridgeCurrentTime)]
    fn audio_bridge_current_time_js() -> f64;

    #[wasm_bindgen(js_name = cadenceAudioBridgeSampleRate)]
    fn audio_bridge_sample_rate_js() -> u32;

    #[wasm_bindgen(js_name = cadenceAudioBridgeAvailableFrames)]
    fn audio_bridge_available_frames_js() -> u32;

    #[wasm_bindgen(catch, js_name = cadenceAudioBridgePush)]
    fn audio_bridge_push_js(
        samples: &Float32Array,
        frame_count: u32,
        channels: u32,
    ) -> Result<u32, JsValue>;

    #[wasm_bindgen(js_name = cadenceAudioBridgeTakeError)]
    fn audio_bridge_take_error_js() -> Option<String>;
}

struct RendererSlot {
    renderer_id: u64,
    renderer: AudioRenderer,
    scratch: Vec<Frame>,
    interleaved: Vec<f32>,
}

impl RendererSlot {
    fn new(renderer_id: u64, renderer: AudioRenderer) -> Self {
        Self {
            renderer_id,
            renderer,
            scratch: Vec::new(),
            interleaved: Vec::new(),
        }
    }

    fn render_chunk(&mut self, frame_count: usize) -> Result<(), JsValue> {
        if frame_count == 0 {
            return Ok(());
        }

        if self.scratch.len() != frame_count {
            self.scratch.resize(frame_count, Frame::ZERO);
        }
        if self.interleaved.len() != frame_count * CHANNELS {
            self.interleaved.resize(frame_count * CHANNELS, 0.0);
        }

        self.renderer.render(self.scratch.as_mut_slice());
        for (index, frame) in self.scratch.iter().enumerate() {
            let base = index * CHANNELS;
            self.interleaved[base] = frame.left.clamp(-1.0, 1.0);
            self.interleaved[base + 1] = frame.right.clamp(-1.0, 1.0);
        }

        let samples = Float32Array::from(self.interleaved.as_slice());
        let written = audio_bridge_push_js(&samples, frame_count as u32, CHANNELS as u32)?;
        if written != frame_count as u32 {
            return Err(JsValue::from_str("audio ring write underflow"));
        }
        Ok(())
    }
}

struct WebAudioBridge {
    sample_rate: u32,
    renderer_slot: Rc<RefCell<Option<RendererSlot>>>,
    pump_interval_id: i32,
    #[allow(dead_code)]
    pump_closure: Closure<dyn FnMut()>,
}

impl WebAudioBridge {
    fn new(sample_rate: u32) -> Result<Self, String> {
        let renderer_slot = Rc::new(RefCell::new(None::<RendererSlot>));
        let slot_for_closure = Rc::clone(&renderer_slot);
        let closure = Closure::wrap(Box::new(move || {
            let available_frames = audio_bridge_available_frames_js() as usize;
            if available_frames == 0 {
                return;
            }

            let mut borrowed = slot_for_closure.borrow_mut();
            let Some(slot) = borrowed.as_mut() else {
                return;
            };

            let chunk = available_frames.min(512);
            let _ = slot.render_chunk(chunk);
        }) as Box<dyn FnMut()>);

        let window = web_sys::window().ok_or_else(|| "window unavailable".to_string())?;
        let interval_id = window
            .set_interval_with_callback_and_timeout_and_arguments_0(
                closure.as_ref().unchecked_ref(),
                PUMP_INTERVAL_MS,
            )
            .map_err(js_error)?;

        Ok(Self {
            sample_rate,
            renderer_slot,
            pump_interval_id: interval_id,
            pump_closure: closure,
        })
    }

    fn install_renderer(
        &self,
        queue_capacity: usize,
    ) -> Result<(WebAudioBackend, AudioControl), WebAudioBackendError> {
        let (audio, mut renderer) =
            AudioRenderer::split(AudioRendererSettings::new(self.sample_rate, queue_capacity))?;
        renderer.on_start_processing();

        let renderer_id = NEXT_RENDERER_ID.with(|next| {
            let id = next.get();
            next.set(id.saturating_add(1));
            id
        });
        *self.renderer_slot.borrow_mut() = Some(RendererSlot::new(renderer_id, renderer));

        Ok((WebAudioBackend { renderer_id }, audio))
    }
}

impl Drop for WebAudioBridge {
    fn drop(&mut self) {
        if let Some(window) = web_sys::window() {
            window.clear_interval_with_handle(self.pump_interval_id);
        }
    }
}

pub(crate) struct WebAudioBackend {
    renderer_id: u64,
}

impl WebAudioBackend {
    pub(crate) fn new(queue_capacity: usize) -> Result<(Self, AudioControl), WebAudioBackendError> {
        AUDIO_BRIDGE.with(|slot| {
            let bridge = slot.borrow();
            let bridge = bridge.as_ref().ok_or(WebAudioBackendError::Unavailable)?;
            bridge.install_renderer(queue_capacity)
        })
    }
}

impl Drop for WebAudioBackend {
    fn drop(&mut self) {
        AUDIO_BRIDGE.with(|slot| {
            if let Some(bridge) = slot.borrow().as_ref() {
                let mut renderer_slot = bridge.renderer_slot.borrow_mut();
                if renderer_slot
                    .as_ref()
                    .map(|slot| slot.renderer_id == self.renderer_id)
                    .unwrap_or(false)
                {
                    renderer_slot.take();
                }
            }
        });
    }
}

pub(crate) async fn ensure_audio_bridge_ready() -> Result<u32, String> {
    let value = prepare_audio_bridge_js().await.map_err(js_error)?;
    let sample_rate = value
        .as_f64()
        .map(|value| value.round() as u32)
        .unwrap_or_else(audio_bridge_sample_rate_js);
    AUDIO_BRIDGE.with(|slot| {
        let mut bridge = slot.borrow_mut();
        if bridge.is_none() {
            *bridge = Some(WebAudioBridge::new(sample_rate)?);
        }
        Ok(sample_rate)
    })
}

pub(crate) async fn prime_audio_from_gesture() -> Result<u32, String> {
    let value = resume_audio_bridge_js().await.map_err(js_error)?;
    let sample_rate = value
        .as_f64()
        .map(|value| value.round() as u32)
        .unwrap_or_else(audio_bridge_sample_rate_js);
    AUDIO_BRIDGE.with(|slot| {
        let mut bridge = slot.borrow_mut();
        if bridge.is_none() {
            *bridge = Some(WebAudioBridge::new(sample_rate)?);
        }
        Ok(sample_rate)
    })
}

pub(crate) fn audio_time() -> f64 {
    audio_bridge_current_time_js()
}

pub(crate) fn take_last_error() -> Option<String> {
    audio_bridge_take_error_js()
}

#[derive(Debug, thiserror::Error)]
pub(crate) enum WebAudioBackendError {
    #[error("web audio backend unavailable; call ensure_runtime_ready after a user gesture")]
    Unavailable,
    #[error("failed to create audio renderer: {0}")]
    Renderer(#[from] AudioRendererError),
}

fn js_error(error: JsValue) -> String {
    error
        .as_string()
        .or_else(|| {
            error
                .dyn_ref::<js_sys::Error>()
                .map(|value| value.message().into())
        })
        .or_else(|| {
            js_sys::Reflect::get(&error, &JsValue::from_str("message"))
                .ok()
                .and_then(|value| value.as_string())
        })
        .unwrap_or_else(|| "browser audio bridge failed".to_string())
}
