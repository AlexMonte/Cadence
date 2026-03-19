use super::*;

pub(super) fn is_tauri_unavailable(error: &str) -> bool {
    error.contains("tauri invoke unavailable")
}

pub(super) fn empty_project_view() -> ProjectViewDto {
    ProjectViewDto {
        name: "No Project".to_string(),
        schema_version: 3,
        node_count: 0,
        edge_count: 0,
        dirty: false,
        path: None,
    }
}

pub(super) fn empty_graph() -> GraphView {
    GraphView {
        name: "runtime".to_string(),
        cols: DEFAULT_GRID_COLS,
        rows: DEFAULT_GRID_ROWS,
        nodes: Vec::new(),
        edges: Vec::new(),
    }
}

pub(super) fn empty_catalog() -> Vec<PieceDef> {
    Vec::new()
}

pub(super) fn empty_init_stage() -> InitStageSnapshotDto {
    InitStageSnapshotDto {
        cps_expr: None,
        sample_loads: Vec::new(),
        tricks: Vec::new(),
    }
}

pub(super) fn empty_project_preview() -> ProjectCompilePreviewDto {
    ProjectCompilePreviewDto {
        can_render: false,
        can_play: false,
        code: None,
        diagnostics: Vec::new(),
        compile_meta: tauri::CompileMetaDto::default(),
    }
}

pub(super) fn empty_graph_preview() -> GraphCompilePreviewDto {
    GraphCompilePreviewDto {
        can_compile: false,
        code: None,
        exprs: Vec::new(),
        diagnostics: Vec::new(),
        eval_order: Vec::new(),
        terminals: Vec::new(),
        compile_meta: tauri::CompileMetaDto::default(),
    }
}

pub(super) fn default_history_status() -> HistoryStatusDto {
    HistoryStatusDto {
        can_undo: false,
        can_redo: false,
        past_len: 0,
        future_len: 0,
    }
}

pub(super) fn default_runtime_status() -> RuntimeStatusDto {
    RuntimeStatusDto {
        rev: 0,
        playing: false,
        has_program: false,
        last_error: None,
        play_elapsed_ms: 0,
    }
}

pub(super) fn default_diagnostics_snapshot() -> DiagnosticsSnapshotDto {
    DiagnosticsSnapshotDto {
        entries: Vec::new(),
        mini_console_visible: false,
        devtools_visible: false,
    }
}

pub(super) fn default_runtime_boot_status() -> runtime::RuntimeBootStatus {
    runtime::RuntimeBootStatus {
        phase: runtime::RuntimeBootPhase::Idle,
        detail: None,
    }
}

pub(super) fn default_sample_readiness() -> runtime::RuntimeSampleReadiness {
    runtime::RuntimeSampleReadiness {
        ready: false,
        attempted: 0,
        loaded: 0,
        failed: 0,
        failures: Vec::new(),
    }
}

pub(super) fn default_sample_cache_status() -> runtime::RuntimeSampleCacheStatus {
    runtime::RuntimeSampleCacheStatus {
        name: "cadence-sample-fetch-v1".to_string(),
        installed: false,
        available: false,
        ready: false,
        hits: 0,
        misses: 0,
        writes: 0,
        last_error: None,
    }
}
