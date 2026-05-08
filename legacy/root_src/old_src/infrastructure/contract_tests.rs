use serde_json::json;

use crate::infrastructure::dto::{
    DiagnosticsSnapshotDto, EditorSyncSnapshotDto, GraphCompilePreviewDto, GraphSnapshotDto,
    HistoryStatusDto, InitStageSnapshotDto, PreviewDocumentDto, ProjectCompilePreviewDto,
    ProjectViewDto, RationalTimeDto, RuntimeCommitArgs, RuntimeCommitDto, RuntimeProgramStateDto,
    RuntimeStatusDto,
};

#[test]
fn project_preview_uses_preview_document_contract() {
    let dto = ProjectCompilePreviewDto {
        can_render: true,
        can_play: false,
        diagnostics: Vec::new(),
        preview: PreviewDocumentDto {
            delay_edges: Vec::new(),
            domain_bridges: Vec::new(),
            output_lanes: Vec::new(),
            preview_events: Vec::new(),
            debug_text: Some("stack(s(\"bd\"))".to_string()),
        },
    };

    let value = serde_json::to_value(dto).expect("serialize preview dto");
    assert!(value.get("preview").is_some());
    assert!(value.get("compile_meta").is_none());
    assert!(value.get("code").is_none());
}

#[test]
fn runtime_commit_args_drop_legacy_text_fields() {
    let value = serde_json::to_value(RuntimeCommitArgs {
        cpm: Some(120.0),
        force: Some(true),
        playing: Some(true),
        request_id: Some(9),
    })
    .expect("serialize runtime commit args");

    assert_eq!(
        value,
        json!({
            "cpm": 120.0,
            "force": true,
            "playing": true,
            "request_id": 9
        })
    );
    assert!(value.get("code_override").is_none());
    assert!(value.get("terminal_strategy").is_none());
}

#[test]
fn runtime_commit_dto_reports_status_not_runtime_code() {
    let dto = RuntimeCommitDto {
        success: true,
        rev: 3,
        changed: true,
        status: RuntimeStatusDto {
            rev: 3,
            playing: true,
            program_state: RuntimeProgramStateDto::Current,
            has_program: true,
            last_error: None,
            play_elapsed_ms: 42,
            cycle_position: RationalTimeDto {
                numerator: 1,
                denominator: 4,
            },
            cps: RationalTimeDto {
                numerator: 2,
                denominator: 1,
            },
        },
        sample_loads: Vec::new(),
        output_count: 2,
        request_id: Some(10),
        diagnostics: Vec::new(),
        error: None,
    };

    let value = serde_json::to_value(dto).expect("serialize runtime dto");
    assert!(value.get("status").is_some());
    assert!(value.get("runtime_code").is_none());
    assert!(value.get("voice_count").is_none());
}

#[test]
fn editor_sync_snapshot_exposes_canonical_hydration_fields() {
    let dto = EditorSyncSnapshotDto {
        project: ProjectViewDto {
            name: "demo".to_string(),
            schema_version: 4,
            node_count: 2,
            edge_count: 1,
            dirty: false,
            path: None,
        },
        init_stage: InitStageSnapshotDto {
            cps_expr: Some("120/60".to_string()),
            sample_loads: Vec::new(),
            tricks: Vec::new(),
        },
        graph: GraphSnapshotDto {
            nodes: Vec::new(),
            edges: Vec::new(),
            name: "runtime".to_string(),
            cols: 12,
            rows: 8,
        },
        catalog: Vec::new(),
        graph_preview: GraphCompilePreviewDto {
            can_compile: true,
            diagnostics: Vec::new(),
            eval_order: Vec::new(),
            outputs: Vec::new(),
            preview: PreviewDocumentDto::default(),
        },
        project_preview: ProjectCompilePreviewDto {
            can_render: true,
            can_play: true,
            diagnostics: Vec::new(),
            preview: PreviewDocumentDto::default(),
        },
        history_status: HistoryStatusDto {
            can_undo: false,
            can_redo: false,
            past_len: 0,
            future_len: 0,
        },
        runtime_status: RuntimeStatusDto {
            rev: 1,
            playing: false,
            program_state: RuntimeProgramStateDto::None,
            has_program: false,
            last_error: None,
            play_elapsed_ms: 0,
            cycle_position: RationalTimeDto {
                numerator: 0,
                denominator: 1,
            },
            cps: RationalTimeDto {
                numerator: 2,
                denominator: 1,
            },
        },
        diagnostics: DiagnosticsSnapshotDto {
            entries: Vec::new(),
            mini_console_visible: false,
            devtools_visible: false,
        },
        recovery_path: None,
        sample_library: None,
    };

    let value = serde_json::to_value(dto).expect("serialize editor sync dto");
    assert!(value.get("project").is_some());
    assert!(value.get("graph_preview").is_some());
    assert!(value.get("project_preview").is_some());
    assert!(value.get("runtime_status").is_some());
    assert!(value.get("diagnostics").is_some());
}
