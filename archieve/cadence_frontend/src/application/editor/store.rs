use std::collections::BTreeMap;

use serde_json::Value;

use crate::adapter::{
    CadenceGraphTarget, DiagnosticsSnapshotDto, GraphCompilePreviewDto, GraphPickTargetParamDto,
    GridPos, HistoryStatusDto, InitStageSnapshotDto, ParamDef, PieceDef, ProjectCompilePreviewDto,
    ProjectViewDto, RuntimeStatusDto, TileSide, backend, runtime,
};
use crate::domain::{DEFAULT_GRID_COLS, DEFAULT_GRID_ROWS, GraphView};

pub use super::service::{
    EditorCommandTrace, EditorLifecycleState, ReadinessState, SubsystemReadiness,
};

pub const SAMPLE_ALIAS_PLACEHOLDER: &str = r#"{"gtr":"gtr/0001_cleanC.wav"}"#;
pub const BACKEND_REQUIRED_MESSAGE: &str =
    "Native backend unavailable. Launch the Cadence desktop app to load and edit a project.";
pub const SAMPLE_LIBRARY_FOCUS_RULES: [(&str, &[&str]); 17] = [
    ("Kick", &["kick", "kicks", "bd"]),
    ("Snare", &["snare", "snares"]),
    ("Hat", &["hat", "hats", "hihat", "hihats", "oh"]),
    ("Clap", &["clap", "claps"]),
    ("Drum", &["drum", "drums"]),
    ("Percussion", &["perc", "percs", "percussion"]),
    ("Rim", &["rim", "rims"]),
    ("Crash", &["crash", "crashes"]),
    ("808", &["808"]),
    ("Vocal", &["vocal", "vocals", "vox"]),
    ("Bass", &["bass", "sub"]),
    ("FX", &["fx"]),
    ("Riser", &["riser", "risers"]),
    ("Texture", &["texture", "textures"]),
    ("Pad", &["pad", "pads"]),
    ("Bell", &["bell", "bells"]),
    ("Pluck", &["pluck", "plucks"]),
];

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum WorkspaceMode {
    Runtime,
    Init,
    Trick { trick_id: String, name: String },
}

impl WorkspaceMode {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Runtime => "Song",
            Self::Init => "Setup",
            Self::Trick { .. } => "Pattern",
        }
    }

    pub fn picker_allowed(&self) -> bool {
        !matches!(self, Self::Init)
    }

    pub fn graph_target(&self) -> CadenceGraphTarget {
        match self {
            Self::Runtime | Self::Init => CadenceGraphTarget::Runtime,
            Self::Trick { trick_id, .. } => CadenceGraphTarget::Trick {
                trick_id: trick_id.clone(),
            },
        }
    }

    pub fn workspace_mode_attr(&self) -> &'static str {
        match self {
            Self::Init => "init",
            _ => "runtime",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ActivityTab {
    Issues,
    Preview,
    Diagnostics,
}

#[derive(Clone, Debug, PartialEq)]
pub enum DragSession {
    PickerPiece {
        piece_id: String,
    },
    CanvasMarquee {
        pointer_id: i32,
        board_page_origin: PointerPoint,
        anchor: PointerPoint,
        current: PointerPoint,
        additive: bool,
    },
    NodeMove {
        from: GridPos,
        pointer_id: i32,
        board_page_origin: PointerPoint,
        current_page: PointerPoint,
    },
    EdgeFrom {
        from: GridPos,
        pointer_id: i32,
        board_page_origin: PointerPoint,
        current_page: PointerPoint,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DragHoverStatus {
    Valid,
    Invalid,
    Swap,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DragHoverReason {
    OccupiedTarget,
    OutsideGrid,
    Collision,
    MoveOnly,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AutoConnectOutcomeKind {
    Connected,
    Unchanged,
    Blocked,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AutoConnectFeedback {
    pub position: GridPos,
    pub kind: AutoConnectOutcomeKind,
    pub headline: String,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DragHover {
    pub position: GridPos,
    pub status: DragHoverStatus,
    pub reason: Option<DragHoverReason>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DragPreviewKind {
    PickerPlacement,
    NodeMove,
    GroupMove,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DragPreviewOutcomeKind {
    Connect,
    MoveOnly,
    Blocked,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DragPreviewFeedback {
    pub position: GridPos,
    pub drag_kind: DragPreviewKind,
    pub kind: DragPreviewOutcomeKind,
    pub target: Option<GridPos>,
    pub target_label: Option<String>,
    pub headline: String,
    pub detail: String,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PointerPoint {
    pub x: f64,
    pub y: f64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct InlineControlDrag {
    pub position: GridPos,
    pub param_id: String,
    pub pointer_id: i32,
    pub start_page_y: f64,
    pub start_value: f64,
    pub min: Option<f64>,
    pub max: Option<f64>,
    pub step: Option<f64>,
    pub active: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PendingProjectAction {
    NewProject,
    OpenProject,
    QuitApp,
    CloseWindow,
}

#[derive(Clone, Debug, PartialEq)]
pub enum ArmedInteraction {
    CanvasMarquee {
        pointer_id: i32,
        origin: PointerPoint,
        board_page_origin: PointerPoint,
        additive: bool,
    },
    NodeBody {
        position: GridPos,
        pointer_id: i32,
        origin: PointerPoint,
        board_page_origin: PointerPoint,
    },
    EdgeHandle {
        from: GridPos,
        pointer_id: i32,
        origin: PointerPoint,
        board_page_origin: PointerPoint,
    },
}

#[derive(Clone, Debug, PartialEq)]
pub enum InteractionState {
    Armed(ArmedInteraction),
    Dragging(DragSession),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ResolvedTileKind {
    Input,
    Transform,
    Atom,
    Ouput,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ResolvedParamAuthoringMode {
    PatternPort,
    ControlValue,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ResolvedTargetParamBinding {
    pub target_piece_id: String,
    pub target_piece_label: String,
    pub target_param_id: String,
    pub target_param_label: String,
    pub slot_label: Option<String>,
    pub mode: ResolvedParamAuthoringMode,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ResolvedRegularTileView {
    pub has_runtime_feedback: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct InputEditorState {
    pub position: GridPos,
    pub piece_label: String,
    pub value_param: Option<String>,
    pub param: Option<ParamDef>,
    pub draft: String,
    pub context_note: Option<String>,
    pub node_state: Option<Value>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BundleSlotView {
    pub label: String,
    pub filled: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ContextualLibraryMode {
    Tiles,
    Atoms,
}

#[derive(Clone, Debug, PartialEq)]
pub struct InspectorState {
    pub open: bool,
    pub label_input: String,
    pub param_inputs: BTreeMap<String, String>,
    pub pending_probe: Option<GraphPickTargetParamDto>,
    pub show_advanced: bool,
}

impl Default for InspectorState {
    fn default() -> Self {
        Self {
            open: false,
            label_input: String::new(),
            param_inputs: BTreeMap::new(),
            pending_probe: None,
            show_advanced: false,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct EditorState {
    pub loading: bool,
    pub backend_available: bool,
    pub lifecycle: EditorLifecycleState,
    pub command_trace: EditorCommandTrace,
    pub project: ProjectViewDto,
    pub project_name_input: String,
    pub cpm_input: String,
    pub workspace_mode: WorkspaceMode,
    pub graph: GraphView,
    pub catalog: Vec<PieceDef>,
    pub init_stage: InitStageSnapshotDto,
    pub sample_library: backend::SampleLibrarySnapshotDto,
    pub project_preview: ProjectCompilePreviewDto,
    pub graph_preview: GraphCompilePreviewDto,
    pub history_status: HistoryStatusDto,
    pub runtime_status: RuntimeStatusDto,
    pub diagnostics: DiagnosticsSnapshotDto,
    pub runtime_boot: runtime::RuntimeBootStatus,
    pub sample_readiness: runtime::RuntimeSampleReadiness,
    pub sample_cache: runtime::RuntimeSampleCacheStatus,
    pub init_sample_status: Vec<runtime::RuntimeInitSampleStatus>,
    pub selected_cell: Option<GridPos>,
    pub selected_node: Option<GridPos>,
    pub selected_nodes: Vec<GridPos>,
    pub picker_open: bool,
    pub picker_target: Option<GridPos>,
    pub selected_container_insertion_index: Option<usize>,
    pub contextual_library_open: bool,
    pub contextual_library_mode: ContextualLibraryMode,
    pub contextual_library_anchor: Option<PointerPoint>,
    pub picker_query: String,
    pub picker_category: Option<String>,
    pub picker_focus_nonce: u64,
    pub command_palette_open: bool,
    pub command_palette_query: String,
    pub command_palette_selected: usize,
    pub inspector: InspectorState,
    pub compile_open: bool,
    pub interaction_state: Option<InteractionState>,
    pub drag_hover: Option<DragHover>,
    pub drag_preview: Option<DragPreviewFeedback>,
    pub drag_preview_nonce: u64,
    pub auto_connect_feedback: Option<AutoConnectFeedback>,
    pub inline_control_drag: Option<InlineControlDrag>,
    pub init_cps_input: String,
    pub sample_library_query_input: String,
    pub sample_id_input: String,
    pub sample_source_input: String,
    pub sample_aliases_input: String,
    pub trick_name_input: String,
    pub pending_project_action: Option<PendingProjectAction>,
    pub recovery_path: Option<String>,
    pub recovery_open: bool,
    pub recovery_checked: bool,
    pub recovery_generation: u64,
    pub activity_tab: ActivityTab,
    pub show_activity_diagnostics: bool,
    pub status_message: Option<String>,
    pub input_editor: Option<InputEditorState>,
}

impl Default for EditorState {
    fn default() -> Self {
        let project = empty_project_view();
        Self {
            loading: true,
            backend_available: false,
            lifecycle: EditorLifecycleState::default(),
            command_trace: EditorCommandTrace::default(),
            project_name_input: project.name.clone(),
            cpm_input: "120".to_string(),
            workspace_mode: WorkspaceMode::Runtime,
            graph: empty_graph(),
            catalog: empty_catalog(),
            init_stage: empty_init_stage(),
            sample_library: empty_sample_library(),
            project_preview: empty_project_preview(),
            graph_preview: empty_graph_preview(),
            history_status: default_history_status(),
            runtime_status: default_runtime_status(),
            diagnostics: default_diagnostics_snapshot(),
            runtime_boot: default_runtime_boot_status(),
            sample_readiness: default_sample_readiness(),
            sample_cache: default_sample_cache_status(),
            init_sample_status: Vec::new(),
            selected_cell: None,
            selected_node: None,
            selected_nodes: Vec::new(),
            picker_open: true,
            picker_target: None,
            selected_container_insertion_index: None,
            contextual_library_open: false,
            contextual_library_mode: ContextualLibraryMode::Tiles,
            contextual_library_anchor: None,
            picker_query: String::new(),
            picker_category: None,
            picker_focus_nonce: 0,
            command_palette_open: false,
            command_palette_query: String::new(),
            command_palette_selected: 0,
            compile_open: false,
            interaction_state: None,
            drag_hover: None,
            drag_preview: None,
            drag_preview_nonce: 0,
            auto_connect_feedback: None,
            inline_control_drag: None,
            init_cps_input: String::new(),
            sample_library_query_input: String::new(),
            sample_id_input: String::new(),
            sample_source_input: String::new(),
            sample_aliases_input: String::new(),
            trick_name_input: String::new(),
            inspector: InspectorState::default(),
            pending_project_action: None,
            recovery_path: None,
            recovery_open: false,
            recovery_checked: false,
            recovery_generation: 0,
            activity_tab: ActivityTab::Issues,
            show_activity_diagnostics: false,
            status_message: Some("Connecting to live backend…".to_string()),
            input_editor: None,
            project,
        }
    }
}

pub type EditorShellState = EditorState;

#[derive(Clone)]
pub struct LoadedSnapshot {
    pub backend_available: bool,
    pub project: ProjectViewDto,
    pub graph: GraphView,
    pub catalog: Vec<PieceDef>,
    pub init_stage: InitStageSnapshotDto,
    pub sample_library: Option<backend::SampleLibrarySnapshotDto>,
    pub project_preview: ProjectCompilePreviewDto,
    pub graph_preview: GraphCompilePreviewDto,
    pub history_status: HistoryStatusDto,
    pub runtime_status: RuntimeStatusDto,
    pub diagnostics: DiagnosticsSnapshotDto,
    pub recovery_path: Option<String>,
}

pub fn is_backend_unavailable(error: &str) -> bool {
    error.contains("native backend unavailable") || error.contains("live backend unavailable")
}

pub fn empty_project_view() -> ProjectViewDto {
    ProjectViewDto {
        name: "No Project".to_string(),
        schema_version: 0,
        node_count: 0,
        edge_count: 0,
        dirty: false,
        path: None,
    }
}

pub fn empty_graph() -> GraphView {
    GraphView {
        name: "runtime".to_string(),
        cols: DEFAULT_GRID_COLS,
        rows: DEFAULT_GRID_ROWS,
        nodes: Vec::new(),
        edges: Vec::new(),
    }
}

pub fn empty_catalog() -> Vec<PieceDef> {
    Vec::new()
}

pub fn empty_init_stage() -> InitStageSnapshotDto {
    InitStageSnapshotDto {
        cps_expr: None,
        sample_loads: Vec::new(),
        tricks: Vec::new(),
    }
}

pub fn empty_sample_library() -> backend::SampleLibrarySnapshotDto {
    backend::SampleLibrarySnapshotDto {
        available: false,
        source_kind: backend::SampleLibrarySourceKind::Unavailable,
        source_label: "Sample library unavailable".to_string(),
        source_path: None,
        entries: Vec::new(),
        error: None,
    }
}

pub fn empty_project_preview() -> ProjectCompilePreviewDto {
    ProjectCompilePreviewDto {
        can_render: false,
        can_play: false,
        diagnostics: Vec::new(),
        preview: backend::PreviewDocumentDto::default(),
    }
}

pub fn empty_graph_preview() -> GraphCompilePreviewDto {
    GraphCompilePreviewDto {
        can_compile: false,
        diagnostics: Vec::new(),
        eval_order: Vec::new(),
        outputs: Vec::new(),
        preview: backend::PreviewDocumentDto::default(),
    }
}

pub fn default_history_status() -> HistoryStatusDto {
    HistoryStatusDto {
        can_undo: false,
        can_redo: false,
        past_len: 0,
        future_len: 0,
    }
}

pub fn default_runtime_status() -> RuntimeStatusDto {
    RuntimeStatusDto {
        rev: 0,
        playing: false,
        program_state: backend::RuntimeProgramStateDto::None,
        has_program: false,
        last_error: None,
        play_elapsed_ms: 0,
        cycle_position: backend::RationalTimeDto {
            numerator: 0,
            denominator: 1,
        },
        cps: backend::RationalTimeDto {
            numerator: 0,
            denominator: 1,
        },
    }
}

pub fn default_diagnostics_snapshot() -> DiagnosticsSnapshotDto {
    DiagnosticsSnapshotDto {
        entries: Vec::new(),
        mini_console_visible: false,
        devtools_visible: false,
    }
}

pub fn default_runtime_boot_status() -> runtime::RuntimeBootStatus {
    runtime::RuntimeBootStatus {
        phase: runtime::RuntimeBootPhase::Idle,
        detail: None,
    }
}

pub fn default_sample_readiness() -> runtime::RuntimeSampleReadiness {
    runtime::RuntimeSampleReadiness {
        ready: false,
        attempted: 0,
        loaded: 0,
        failed: 0,
        failures: Vec::new(),
    }
}

pub fn default_sample_cache_status() -> runtime::RuntimeSampleCacheStatus {
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

pub fn tile_side_to_string(side: TileSide) -> &'static str {
    match side {
        TileSide::Top => "top",
        TileSide::Bottom => "bottom",
        TileSide::Left => "left",
        TileSide::Right => "right",
    }
}
