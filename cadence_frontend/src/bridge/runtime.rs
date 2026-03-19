use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
#[cfg(target_arch = "wasm32")]
use serde_json::Value;

use crate::bridge::tauri::CadenceSampleLoadDto;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RuntimeProgramPayload {
    pub cps_expr: Option<String>,
    pub sample_loads: Vec<CadenceSampleLoadDto>,
    pub declaration_code: Vec<String>,
    pub runtime_code: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RuntimeBootPhase {
    Idle,
    Booting,
    Ready,
    Error,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeBootStatus {
    pub phase: RuntimeBootPhase,
    pub detail: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RuntimeSampleReadiness {
    pub ready: bool,
    pub attempted: usize,
    pub loaded: usize,
    pub failed: usize,
    pub failures: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RuntimeSampleCacheStatus {
    pub name: String,
    pub installed: bool,
    pub available: bool,
    pub ready: bool,
    pub hits: usize,
    pub misses: usize,
    pub writes: usize,
    pub last_error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RuntimeInitSampleStatus {
    pub id: String,
    pub source: String,
    #[serde(default)]
    pub aliases: BTreeMap<String, String>,
    pub status: String,
    pub error: Option<String>,
}

#[cfg(target_arch = "wasm32")]
mod js_bridge {
    use wasm_bindgen::prelude::*;

    #[wasm_bindgen(module = "/src/bridge/strudel.js")]
    extern "C" {
        #[wasm_bindgen(catch, js_name = primeAudioFromGesture)]
        pub async fn prime_audio_from_gesture() -> Result<(), JsValue>;

        #[wasm_bindgen(catch, js_name = ensureRuntimeReady)]
        pub async fn ensure_runtime_ready() -> Result<(), JsValue>;

        #[wasm_bindgen(catch, js_name = runtimeBootState)]
        pub fn runtime_boot_state() -> Result<JsValue, JsValue>;

        #[wasm_bindgen(catch, js_name = runtimeSampleReadiness)]
        pub fn runtime_sample_readiness() -> Result<JsValue, JsValue>;

        #[wasm_bindgen(catch, js_name = runtimeSampleCacheStatus)]
        pub fn runtime_sample_cache_status() -> Result<JsValue, JsValue>;

        #[wasm_bindgen(catch, js_name = runtimeInitSampleStatus)]
        pub fn runtime_init_sample_status() -> Result<JsValue, JsValue>;

        #[wasm_bindgen(catch, js_name = runCadenceProgram)]
        pub async fn run_cadence_program(program: JsValue) -> Result<(), JsValue>;

        #[wasm_bindgen(catch, js_name = stopProgram)]
        pub async fn stop_program() -> Result<(), JsValue>;

        #[wasm_bindgen(catch, js_name = getRuntimeAudioTime)]
        pub fn get_runtime_audio_time() -> Result<f64, JsValue>;
    }

    #[wasm_bindgen(inline_js = r#"
let cadenceMenuInstalled = false;
let cadenceMenuQueue = [];

export function cadenceInstallMenuBridge() {
  if (cadenceMenuInstalled) {
    return;
  }
  cadenceMenuInstalled = true;
  window.__CADENCE_MENU_ACTION = function(action) {
    cadenceMenuQueue.push(String(action ?? ""));
  };
}

export function cadenceTakeMenuActions() {
  const queued = cadenceMenuQueue.slice();
  cadenceMenuQueue.length = 0;
  return queued;
}

export async function cadenceSleep(ms) {
  await new Promise((resolve) => window.setTimeout(resolve, ms));
}
"#)]
    extern "C" {
        #[wasm_bindgen(js_name = cadenceInstallMenuBridge)]
        pub fn install_menu_bridge();

        #[wasm_bindgen(js_name = cadenceTakeMenuActions)]
        pub fn take_menu_actions() -> JsValue;

        #[wasm_bindgen(catch, js_name = cadenceSleep)]
        pub async fn sleep_ms(ms: u32) -> Result<(), JsValue>;
    }
}

#[cfg(target_arch = "wasm32")]
fn js_error_message(value: wasm_bindgen::JsValue) -> String {
    value
        .as_string()
        .or_else(|| {
            serde_wasm_bindgen::from_value::<Value>(value)
                .ok()
                .map(|raw| raw.to_string())
        })
        .unwrap_or_else(|| "runtime bridge failed".to_string())
}

#[cfg(not(target_arch = "wasm32"))]
fn non_wasm_runtime_error() -> String {
    "runtime bridge unavailable on non-wasm target".to_string()
}

pub async fn prime_audio_from_gesture() -> Result<(), String> {
    #[cfg(target_arch = "wasm32")]
    {
        return js_bridge::prime_audio_from_gesture()
            .await
            .map_err(js_error_message);
    }

    #[cfg(not(target_arch = "wasm32"))]
    {
        Err(non_wasm_runtime_error())
    }
}

pub async fn ensure_runtime_ready() -> Result<(), String> {
    #[cfg(target_arch = "wasm32")]
    {
        return js_bridge::ensure_runtime_ready()
            .await
            .map_err(js_error_message);
    }

    #[cfg(not(target_arch = "wasm32"))]
    {
        Err(non_wasm_runtime_error())
    }
}

pub fn runtime_boot_status() -> Result<RuntimeBootStatus, String> {
    #[cfg(target_arch = "wasm32")]
    {
        let raw = js_bridge::runtime_boot_state().map_err(js_error_message)?;
        let value = raw
            .as_string()
            .unwrap_or_else(|| "error: invalid boot state".to_string());
        let trimmed = value.trim().to_string();
        if trimmed == "idle" {
            return Ok(RuntimeBootStatus {
                phase: RuntimeBootPhase::Idle,
                detail: None,
            });
        }
        if trimmed == "booting" {
            return Ok(RuntimeBootStatus {
                phase: RuntimeBootPhase::Booting,
                detail: None,
            });
        }
        if trimmed == "ready" {
            return Ok(RuntimeBootStatus {
                phase: RuntimeBootPhase::Ready,
                detail: None,
            });
        }
        if let Some(detail) = trimmed.strip_prefix("error: ") {
            return Ok(RuntimeBootStatus {
                phase: RuntimeBootPhase::Error,
                detail: Some(detail.to_string()),
            });
        }
        return Ok(RuntimeBootStatus {
            phase: RuntimeBootPhase::Error,
            detail: Some(trimmed),
        });
    }

    #[cfg(not(target_arch = "wasm32"))]
    {
        Err(non_wasm_runtime_error())
    }
}

pub fn runtime_sample_readiness() -> Result<RuntimeSampleReadiness, String> {
    #[cfg(target_arch = "wasm32")]
    {
        let raw = js_bridge::runtime_sample_readiness().map_err(js_error_message)?;
        return serde_wasm_bindgen::from_value(raw).map_err(|err| err.to_string());
    }

    #[cfg(not(target_arch = "wasm32"))]
    {
        Err(non_wasm_runtime_error())
    }
}

pub fn runtime_sample_cache_status() -> Result<RuntimeSampleCacheStatus, String> {
    #[cfg(target_arch = "wasm32")]
    {
        let raw = js_bridge::runtime_sample_cache_status().map_err(js_error_message)?;
        return serde_wasm_bindgen::from_value(raw).map_err(|err| err.to_string());
    }

    #[cfg(not(target_arch = "wasm32"))]
    {
        Err(non_wasm_runtime_error())
    }
}

pub fn runtime_init_sample_status() -> Result<Vec<RuntimeInitSampleStatus>, String> {
    #[cfg(target_arch = "wasm32")]
    {
        let raw = js_bridge::runtime_init_sample_status().map_err(js_error_message)?;
        return serde_wasm_bindgen::from_value(raw).map_err(|err| err.to_string());
    }

    #[cfg(not(target_arch = "wasm32"))]
    {
        Err(non_wasm_runtime_error())
    }
}

pub async fn run_cadence_program(program: &RuntimeProgramPayload) -> Result<(), String> {
    #[cfg(target_arch = "wasm32")]
    {
        let payload = serde_wasm_bindgen::to_value(program).map_err(|err| err.to_string())?;
        return js_bridge::run_cadence_program(payload)
            .await
            .map_err(js_error_message);
    }

    #[cfg(not(target_arch = "wasm32"))]
    {
        let _ = program;
        Err(non_wasm_runtime_error())
    }
}

pub async fn stop_program() -> Result<(), String> {
    #[cfg(target_arch = "wasm32")]
    {
        return js_bridge::stop_program().await.map_err(js_error_message);
    }

    #[cfg(not(target_arch = "wasm32"))]
    {
        Err(non_wasm_runtime_error())
    }
}

pub fn get_runtime_audio_time() -> Result<f64, String> {
    #[cfg(target_arch = "wasm32")]
    {
        return js_bridge::get_runtime_audio_time().map_err(js_error_message);
    }

    #[cfg(not(target_arch = "wasm32"))]
    {
        Err(non_wasm_runtime_error())
    }
}

pub fn install_menu_bridge() -> Result<(), String> {
    #[cfg(target_arch = "wasm32")]
    {
        js_bridge::install_menu_bridge();
        return Ok(());
    }

    #[cfg(not(target_arch = "wasm32"))]
    {
        Err(non_wasm_runtime_error())
    }
}

pub fn take_menu_actions() -> Result<Vec<String>, String> {
    #[cfg(target_arch = "wasm32")]
    {
        let raw = js_bridge::take_menu_actions();
        return serde_wasm_bindgen::from_value(raw).map_err(|err| err.to_string());
    }

    #[cfg(not(target_arch = "wasm32"))]
    {
        Err(non_wasm_runtime_error())
    }
}

pub async fn sleep_ms(ms: u32) -> Result<(), String> {
    #[cfg(target_arch = "wasm32")]
    {
        return js_bridge::sleep_ms(ms).await.map_err(js_error_message);
    }

    #[cfg(not(target_arch = "wasm32"))]
    {
        let _ = ms;
        Err(non_wasm_runtime_error())
    }
}
