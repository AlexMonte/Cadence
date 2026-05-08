use super::{
    RuntimeBootPhase, RuntimeBootStatus, RuntimeInitSampleStatus, RuntimeProgramPayload,
    RuntimeSampleCacheStatus, RuntimeSampleReadiness,
};

pub async fn prime_audio_from_gesture() -> Result<(), String> {
    Ok(())
}

pub async fn ensure_runtime_ready() -> Result<(), String> {
    Ok(())
}

pub fn runtime_boot_status() -> Result<RuntimeBootStatus, String> {
    Ok(RuntimeBootStatus {
        phase: RuntimeBootPhase::Ready,
        detail: Some("cadence_core backend active".to_string()),
    })
}

pub fn runtime_sample_readiness() -> Result<RuntimeSampleReadiness, String> {
    Ok(RuntimeSampleReadiness {
        ready: true,
        attempted: 0,
        loaded: 0,
        failed: 0,
        failures: Vec::new(),
    })
}

pub fn runtime_sample_cache_status() -> Result<RuntimeSampleCacheStatus, String> {
    Ok(RuntimeSampleCacheStatus {
        name: "cadence_core-backend".to_string(),
        installed: true,
        available: true,
        ready: true,
        hits: 0,
        misses: 0,
        writes: 0,
        last_error: None,
    })
}

pub fn runtime_init_sample_status() -> Result<Vec<RuntimeInitSampleStatus>, String> {
    Ok(Vec::new())
}

pub async fn run_cadence_program(_program: &RuntimeProgramPayload) -> Result<(), String> {
    Ok(())
}

pub async fn stop_program() -> Result<(), String> {
    Ok(())
}

pub fn get_runtime_audio_time() -> Result<f64, String> {
    Ok(0.0)
}

pub fn install_menu_bridge() -> Result<(), String> {
    Ok(())
}

pub fn take_menu_actions() -> Result<Vec<String>, String> {
    Ok(Vec::new())
}

pub async fn sleep_ms(ms: u32) -> Result<(), String> {
    tokio::time::sleep(std::time::Duration::from_millis(ms as u64)).await;
    Ok(())
}
