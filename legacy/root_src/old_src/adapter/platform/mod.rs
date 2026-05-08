use bevy::prelude::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CadencePlatformTarget {
    NativeDesktop,
    BrowserWasm,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CadenceRenderPath {
    NativeWgpu,
    BrowserWebGl2First,
}

#[derive(Debug, Resource, Clone)]
pub struct PlatformCapabilities {
    pub target: CadencePlatformTarget,
    pub render_path: CadenceRenderPath,
    pub native_file_dialogs: bool,
    pub native_audio_host: bool,
    pub asset_hot_reload: bool,
    pub notes: &'static str,
}

impl PlatformCapabilities {
    #[cfg(target_arch = "wasm32")]
    fn detect() -> Self {
        Self {
            target: CadencePlatformTarget::BrowserWasm,
            render_path: CadenceRenderPath::BrowserWebGl2First,
            native_file_dialogs: false,
            native_audio_host: false,
            asset_hot_reload: false,
            notes: "wasm build prefers broad browser compatibility over native-only rendering assumptions",
        }
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn detect() -> Self {
        Self {
            target: CadencePlatformTarget::NativeDesktop,
            render_path: CadenceRenderPath::NativeWgpu,
            native_file_dialogs: true,
            native_audio_host: true,
            asset_hot_reload: true,
            notes: "native desktop can use the full Bevy renderer and host-backed runtime integrations",
        }
    }
}

pub struct PlatformPlugin;

impl Plugin for PlatformPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(PlatformCapabilities::detect())
            .add_systems(Startup, log_platform_capabilities);
    }
}

fn log_platform_capabilities(capabilities: Res<PlatformCapabilities>) {
    info!(
        target: "cadence::platform",
        "target={:?} render_path={:?} file_dialogs={} native_audio_host={} note={}",
        capabilities.target,
        capabilities.render_path,
        capabilities.native_file_dialogs,
        capabilities.native_audio_host,
        capabilities.notes
    );
}
