use super::{
    RuntimeBootPhase, RuntimeBootStatus, RuntimeInitSampleStatus, RuntimeProgramPayload,
    RuntimeSampleCacheStatus, RuntimeSampleReadiness,
};
use crate::adapter::backend::web::state;

#[cfg(target_arch = "wasm32")]
mod js_bridge {
    use wasm_bindgen::prelude::*;

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

pub async fn prime_audio_from_gesture() -> Result<(), String> {
    cadence::web::prime_audio_from_gesture(state()).await
}

pub async fn ensure_runtime_ready() -> Result<(), String> {
    cadence::web::ensure_runtime_ready(state()).await
}

pub fn runtime_boot_status() -> Result<RuntimeBootStatus, String> {
    cadence::web::runtime_boot_status(state()).map(convert_boot_status)
}

pub fn runtime_sample_readiness() -> Result<RuntimeSampleReadiness, String> {
    cadence::web::runtime_sample_readiness(state()).map(convert_sample_readiness)
}

pub fn runtime_sample_cache_status() -> Result<RuntimeSampleCacheStatus, String> {
    cadence::web::runtime_sample_cache_status(state()).map(convert_sample_cache_status)
}

pub fn runtime_init_sample_status() -> Result<Vec<RuntimeInitSampleStatus>, String> {
    cadence::web::runtime_init_sample_status(state()).map(|items| {
        items
            .into_iter()
            .map(convert_init_sample_status)
            .collect::<Vec<_>>()
    })
}

pub async fn run_cadence_program(program: &RuntimeProgramPayload) -> Result<(), String> {
    let payload = cadence::web::RuntimeProgramPayload {
        cps_expr: program.cps_expr.clone(),
        sample_loads: serde_json::from_value(
            serde_json::to_value(&program.sample_loads).map_err(|err| err.to_string())?,
        )
        .map_err(|err| err.to_string())?,
        output_count: program.output_count,
        debug_text: program.debug_text.clone(),
    };
    cadence::web::run_cadence_program(state(), &payload).await
}

pub async fn stop_program() -> Result<(), String> {
    cadence::web::stop_program(state()).await
}

pub fn get_runtime_audio_time() -> Result<f64, String> {
    cadence::web::get_runtime_audio_time(state())
}

pub fn install_menu_bridge() -> Result<(), String> {
    js_bridge::install_menu_bridge();
    Ok(())
}

pub fn take_menu_actions() -> Result<Vec<String>, String> {
    serde_wasm_bindgen::from_value(js_bridge::take_menu_actions()).map_err(|err| err.to_string())
}

pub async fn sleep_ms(ms: u32) -> Result<(), String> {
    js_bridge::sleep_ms(ms).await.map_err(|err| {
        err.as_string()
            .unwrap_or_else(|| "sleep bridge failed".to_string())
    })
}

fn convert_boot_status(status: cadence::web::RuntimeBootStatus) -> RuntimeBootStatus {
    RuntimeBootStatus {
        phase: match status.phase {
            cadence::web::RuntimeBootPhase::Idle => RuntimeBootPhase::Idle,
            cadence::web::RuntimeBootPhase::Booting => RuntimeBootPhase::Booting,
            cadence::web::RuntimeBootPhase::Ready => RuntimeBootPhase::Ready,
            cadence::web::RuntimeBootPhase::Error => RuntimeBootPhase::Error,
        },
        detail: status.detail,
    }
}

fn convert_sample_readiness(
    readiness: cadence::web::RuntimeSampleReadiness,
) -> RuntimeSampleReadiness {
    RuntimeSampleReadiness {
        ready: readiness.ready,
        attempted: readiness.attempted,
        loaded: readiness.loaded,
        failed: readiness.failed,
        failures: readiness.failures,
    }
}

fn convert_sample_cache_status(
    status: cadence::web::RuntimeSampleCacheStatus,
) -> RuntimeSampleCacheStatus {
    RuntimeSampleCacheStatus {
        name: status.name,
        installed: status.installed,
        available: status.available,
        ready: status.ready,
        hits: status.hits,
        misses: status.misses,
        writes: status.writes,
        last_error: status.last_error,
    }
}

fn convert_init_sample_status(
    status: cadence::web::RuntimeInitSampleStatus,
) -> RuntimeInitSampleStatus {
    RuntimeInitSampleStatus {
        id: status.id,
        source: status.source,
        aliases: status.aliases,
        status: status.status,
        error: status.error,
    }
}
