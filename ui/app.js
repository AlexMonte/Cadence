import {
  ensureRuntimeReady,
  primeAudioFromGesture,
  runtimeBootState,
  runtimeInitSampleStatus,
  runtimeSampleReadiness,
  stopProgram,
  runCadenceProgram,
} from "./src/bridge/strudel.js";

const controls = {
  canvas: document.getElementById("canvas"),
  canvasGrid: document.getElementById("canvas-grid"),
  canvasLayer: document.getElementById("canvas-layer"),
  edgeLayer: document.getElementById("edge-layer"),
  addNodeButton: document.getElementById("add-node"),
  selectedNodeLabel: document.getElementById("selected-node-label"),
  selectedRoleLabel: document.getElementById("selected-role-label"),
  tilePreview: document.getElementById("tile-preview"),
  nodeCodeEditorHost: document.getElementById("node-code-editor"),
  textScriptInput: document.getElementById("text-script-input"),
  textRefreshButton: document.getElementById("text-refresh"),
  projectTitle: document.getElementById("project-title"),
  projectMeta: document.getElementById("project-meta"),
  projectChip: document.getElementById("project-chip"),
  modeChip: document.getElementById("mode-chip"),
  workspaceRuntimeButton: document.getElementById("workspace-runtime"),
  workspaceInitButton: document.getElementById("workspace-init"),
  workspaceBackButton: document.getElementById("workspace-back"),
  compileButton: document.getElementById("compile-toggle"),
  deleteNodeButton: document.getElementById("delete-node"),
  playButton: document.getElementById("play-toggle"),
  stopButton: document.getElementById("stop-toggle"),
  cpmInput: document.getElementById("cpm-value"),
  miniConsole: document.getElementById("mini-console"),
  miniConsoleOutput: document.getElementById("mini-console-output"),
  miniConsoleToggle: document.getElementById("mini-console-toggle"),
  textDiagnosticsSection: document.getElementById("text-diagnostics-section"),
  textImportSummary: document.getElementById("text-import-summary"),
  textImportTableBody: document.getElementById("text-import-table-body"),
  compileBanner: document.getElementById("compile-banner"),
  gridPiecePicker: document.getElementById("grid-piece-picker"),
  gridPiecePickerSearch: document.getElementById("grid-piece-picker-search"),
  gridPiecePickerList: document.getElementById("grid-piece-picker-list"),
  statusHost: document.getElementById("status-host"),
  statusPlayback: document.getElementById("status-playback"),
  statusSelection: document.getElementById("status-selection"),
  statusProject: document.getElementById("status-project"),
  // Pixel modal controls
  tileInspectorModal: document.getElementById("tile-inspector-modal"),
  modalInspectorTitle: document.getElementById("modal-inspector-title"),
  modalInspectorBody: document.getElementById("modal-inspector-body"),
  modalInspectorClose: document.getElementById("modal-inspector-close"),
  compileModal: document.getElementById("compile-modal"),
  modalCompileClose: document.getElementById("modal-compile-close"),
  gridWindow: document.getElementById("grid-window"),
  initWorkspace: document.getElementById("init-workspace"),
  initCpsInput: document.getElementById("init-cps-input"),
  initCpsSave: document.getElementById("init-cps-save"),
  initCpsClear: document.getElementById("init-cps-clear"),
  initSampleId: document.getElementById("init-sample-id"),
  initSampleSource: document.getElementById("init-sample-source"),
  initSampleAliases: document.getElementById("init-sample-aliases"),
  initSampleSave: document.getElementById("init-sample-save"),
  initSampleList: document.getElementById("init-sample-list"),
  initSampleSummary: document.getElementById("init-sample-summary"),
  initTrickName: document.getElementById("init-trick-name"),
  initTrickCreate: document.getElementById("init-trick-create"),
  initTrickList: document.getElementById("init-trick-list"),
  initTrickSummary: document.getElementById("init-trick-summary"),
};

const CELL_W = 64;
const CELL_H = 64;
const GRID_ORIGIN_X = 32;
const GRID_ORIGIN_Y = 32;
const NODE_W = 60;
const NODE_H = 60;
const OUTPUT_HANDLE_SIZE = 28;
const OUTPUT_HANDLE_OFFSET = OUTPUT_HANDLE_SIZE / 2;
const DEFAULT_GRID_COLS = 16;
const DEFAULT_GRID_ROWS = 16;
const DIRECTION_CYCLE = ["north", "east", "south", "west"];

function patternSchema() {
  return {
    kind: "custom",
    port_type: "pattern",
    value_kind: "text",
    can_inline: false,
    inline_mode: "raw",
    default: null,
    min: null,
    max: null,
  };
}

const FALLBACK_CATALOG = [
  {
    id: "strudel.note",
    label: "note",
    category: "generator",
    params: [{ id: "value", side: "south", schema: { kind: "text", can_inline: true } }],
    output_type: "pattern",
    output_side: "east",
    description: "Create a note pattern via note().",
  },
  {
    id: "strudel.n",
    label: "n",
    category: "generator",
    params: [{ id: "value", side: "south", schema: { kind: "text", can_inline: true } }],
    output_type: "pattern",
    output_side: "east",
    description: "Create a pitch-index pattern via n().",
  },
  {
    id: "strudel.sound",
    label: "s",
    category: "generator",
    params: [{ id: "value", side: "south", schema: { kind: "text", can_inline: true } }],
    output_type: "pattern",
    output_side: "east",
    description: "Create a sample pattern via s().",
  },
  {
    id: "strudel.stack",
    label: "stack",
    category: "generator",
    params: [
      { id: "in_w", side: "west", schema: patternSchema(), variadic_group: "patterns", required: true },
      { id: "in_n", side: "north", schema: patternSchema(), variadic_group: "patterns" },
      { id: "in_s", side: "south", schema: patternSchema(), variadic_group: "patterns" },
      { id: "in_e", side: "east", schema: patternSchema(), variadic_group: "patterns" },
    ],
    output_type: "pattern",
    output_side: "east",
    description: "Stack multiple patterns in parallel via stack(...).",
  },
  {
    id: "strudel.cat",
    label: "cat",
    category: "generator",
    params: [
      { id: "in_w", side: "west", schema: patternSchema(), variadic_group: "patterns", required: true },
      { id: "in_n", side: "north", schema: patternSchema(), variadic_group: "patterns" },
      { id: "in_s", side: "south", schema: patternSchema(), variadic_group: "patterns" },
      { id: "in_e", side: "east", schema: patternSchema(), variadic_group: "patterns" },
    ],
    output_type: "pattern",
    output_side: "east",
    description: "Concatenate multiple patterns in sequence via cat(...).",
  },
  {
    id: "strudel.fast",
    label: "fast",
    category: "transform",
    params: [
      { id: "pattern", side: "west", schema: patternSchema() },
      { id: "factor", side: "south", schema: { kind: "number", can_inline: true } },
    ],
    output_type: "pattern",
    output_side: "east",
    description: "Speed up a pattern by a factor.",
  },
  {
    id: "strudel.slow",
    label: "slow",
    category: "transform",
    params: [
      { id: "pattern", side: "west", schema: patternSchema() },
      { id: "factor", side: "south", schema: { kind: "number", can_inline: true } },
    ],
    output_type: "pattern",
    output_side: "east",
    description: "Slow down a pattern by a factor.",
  },
  {
    id: "strudel.rev",
    label: "rev",
    category: "transform",
    params: [{ id: "pattern", side: "west", schema: patternSchema(), required: true }],
    output_type: "pattern",
    output_side: "east",
    description: "Reverse event order in a pattern.",
  },
  {
    id: "strudel.gain",
    label: "gain",
    category: "transform",
    params: [
      { id: "pattern", side: "west", schema: patternSchema() },
      { id: "amount", side: "south", schema: { kind: "number", can_inline: true } },
    ],
    output_type: "pattern",
    output_side: "east",
    description: "Set output gain.",
  },
  {
    id: "strudel.pan",
    label: "pan",
    category: "transform",
    params: [
      { id: "pattern", side: "west", schema: patternSchema() },
      { id: "amount", side: "south", schema: { kind: "number", can_inline: true } },
    ],
    output_type: "pattern",
    output_side: "east",
    description: "Set stereo pan.",
  },
  {
    id: "strudel.room",
    label: "room",
    category: "transform",
    params: [
      { id: "pattern", side: "west", schema: patternSchema() },
      { id: "amount", side: "south", schema: { kind: "number", can_inline: true } },
    ],
    output_type: "pattern",
    output_side: "east",
    description: "Set reverb room amount.",
  },
  {
    id: "strudel.size",
    label: "size",
    category: "transform",
    params: [
      { id: "pattern", side: "west", schema: patternSchema() },
      { id: "amount", side: "south", schema: { kind: "number", can_inline: true } },
    ],
    output_type: "pattern",
    output_side: "east",
    description: "Set reverb size.",
  },
  {
    id: "strudel.mask",
    label: "mask",
    category: "transform",
    params: [
      { id: "pattern", side: "west", schema: patternSchema(), required: true },
      { id: "by", side: "south", schema: patternSchema(), required: true },
    ],
    output_type: "pattern",
    output_side: "east",
    description: "Gate one pattern by another pattern.",
  },
  {
    id: "strudel.number",
    label: "number",
    category: "constant",
    params: [{ id: "value", side: "south", schema: { kind: "number", can_inline: true } }],
    output_type: "number",
    output_side: "north",
    description: "Numeric constant.",
  },
  {
    id: "strudel.text",
    label: "text",
    category: "constant",
    params: [{ id: "value", side: "south", schema: { kind: "text", can_inline: true } }],
    output_type: "text",
    output_side: "north",
    description: "Text constant.",
  },
  {
    id: "strudel.output",
    label: "play",
    category: "output",
    params: [{ id: "pattern", side: "west", schema: patternSchema() }],
    output_type: null,
    output_side: null,
    description: "Terminal output node.",
  },
];

const state = {
  project: null,
  initStage: null,
  graph: null,
  semantic: null,
  compilePreview: null,
  lastGoodCode: "",
  catalog: [],
  catalogById: new Map(),
  selectedPos: null,
  selectedEdgeId: null,
  nodeDragFrom: null,
  dragHoverCell: null,
  edgeAnimReady: false,
  lastEdgeKeys: new Set(),
  newEdgeUntil: new Map(),
  playback: "stopped",
  busy: false,
  miniConsoleVisible: false,
  logLines: [],
  uiLastIssue: "",
  requestSeq: 0,
  gridPickerPos: null,
  gridPickerQuery: "",
  editorMode: "runtime",
  selectedTrickId: null,
};

function activeGraphTarget() {
  if (state.editorMode === "trick" && state.selectedTrickId) {
    return {
      kind: "trick",
      trick_id: state.selectedTrickId,
    };
  }
  return { kind: "runtime" };
}

function isRuntimeMode() {
  return state.editorMode === "runtime";
}

function isInitMode() {
  return state.editorMode === "init";
}

function isTrickMode() {
  return state.editorMode === "trick";
}

function selectedTrick() {
  return state.initStage?.tricks?.find((trick) => trick.id === state.selectedTrickId) ?? null;
}

function modalIsOpen(modal) {
  return !!modal && !modal.classList.contains("is-hidden");
}

function setModalOpen(modal, open) {
  modal?.classList.toggle("is-hidden", !open);
}

function toggleTileInspectorModal(force) {
  const open = typeof force === "boolean" ? force : !modalIsOpen(controls.tileInspectorModal);
  setModalOpen(controls.tileInspectorModal, open);
  if (open) {
    renderInspector();
  }
}

function toggleCompileModal(force) {
  const open = typeof force === "boolean" ? force : !modalIsOpen(controls.compileModal);
  setModalOpen(controls.compileModal, open);
  if (open) {
    renderCompileView();
  }
}

function previewCanRender(preview) {
  return !!(preview?.can_render || preview?.can_compile);
}

function slugifyIdentifier(value, fallback = "item") {
  const base = String(value ?? "")
    .trim()
    .toLowerCase()
    .replace(/[^a-z0-9_$]+/g, "_")
    .replace(/^_+|_+$/g, "");
  return base || fallback;
}

function runtimeStatusCode() {
  const bootState = runtimeBootState();
  if (state.busy) {
    return "BUS";
  }
  if (bootState === "ready") {
    return "RDY";
  }
  if (bootState === "booting") {
    return "BOT";
  }
  if (bootState.startsWith("error:")) {
    return "ERR";
  }
  return "IDL";
}

function setBusy(nextBusy) {
  state.busy = nextBusy;
  if (controls.statusHost) {
    controls.statusHost.textContent = `H:${runtimeStatusCode()}`;
  }
}

function logLine(level, text) {
  const line = `[${new Date().toLocaleTimeString()}] ${level.toUpperCase()} ${text}`;
  state.logLines.push(line);
  if (state.logLines.length > 200) {
    state.logLines.shift();
  }
  if (controls.miniConsoleOutput) {
    controls.miniConsoleOutput.textContent = state.logLines.join("\n");
    controls.miniConsoleOutput.scrollTop = controls.miniConsoleOutput.scrollHeight;
  }
}

function setUiIssue(text) {
  state.uiLastIssue = text ? String(text) : "";
  if (controls.statusSelection) {
    renderProjectMeta();
  }
}

function clearCellDragClasses(cell) {
  if (!(cell instanceof HTMLElement)) {
    return;
  }
  cell.classList.remove("is-drop-valid", "is-drop-invalid", "is-move-valid", "is-move-swap", "is-move-invalid");
}

function clearDragHover() {
  if (state.dragHoverCell instanceof HTMLElement) {
    clearCellDragClasses(state.dragHoverCell);
  }
  state.dragHoverCell = null;
}

function setDragHover(cell, kind, status) {
  if (!(cell instanceof HTMLElement)) {
    clearDragHover();
    return;
  }
  if (state.dragHoverCell && state.dragHoverCell !== cell) {
    clearCellDragClasses(state.dragHoverCell);
  }
  clearCellDragClasses(cell);
  if (!status) {
    state.dragHoverCell = cell;
    return;
  }
  if (kind === "piece" || kind === "edge_from") {
    cell.classList.add(status === "valid" ? "is-drop-valid" : "is-drop-invalid");
  } else if (kind === "node_move_from") {
    if (status === "valid") {
      cell.classList.add("is-move-valid");
    } else if (status === "swap") {
      cell.classList.add("is-move-swap");
    } else if (status === "invalid") {
      cell.classList.add("is-move-invalid");
    }
  }
  state.dragHoverCell = cell;
}

function clearDragState() {
  clearDragHover();
  state.nodeDragFrom = null;
}

function gridCellElementAt(position) {
  if (!position || !controls.canvasGrid) {
    return null;
  }
  return controls.canvasGrid.querySelector(`.grid-cell[data-grid-pos="${nodeKey(position)}"]`);
}

function outputHandleRectForSide(nodeRect, side) {
  if (!nodeRect || !side) {
    return null;
  }
  const half = OUTPUT_HANDLE_OFFSET;
  const centerX = side === "east"
    ? nodeRect.right
    : side === "west"
    ? nodeRect.left
    : nodeRect.left + nodeRect.width / 2;
  const centerY = side === "south"
    ? nodeRect.bottom
    : side === "north"
    ? nodeRect.top
    : nodeRect.top + nodeRect.height / 2;
  return {
    left: centerX - half,
    right: centerX + half,
    top: centerY - half,
    bottom: centerY + half,
  };
}

function pointInRect(clientX, clientY, rect) {
  return !!rect
    && clientX >= rect.left
    && clientX <= rect.right
    && clientY >= rect.top
    && clientY <= rect.bottom;
}

function pointerHitsOutputHandle(nodeEl, side, clientX, clientY) {
  if (!(nodeEl instanceof HTMLElement)) {
    return false;
  }
  return pointInRect(clientX, clientY, outputHandleRectForSide(nodeEl.getBoundingClientRect(), side));
}

// ---------------------------------------------------------------------------
// Pointer-event drag system (replaces HTML5 Drag and Drop which is unreliable
// in WKWebView / Tauri).
// ---------------------------------------------------------------------------

const DRAG_THRESHOLD = 5;
let _ptrDrag = null;
let _suppressNextClick = false;

// Capture-phase click suppressor — prevents the click that follows a
// completed drag from selecting/toggling the source element.
document.addEventListener(
  "click",
  (event) => {
    if (_suppressNextClick) {
      event.stopPropagation();
      event.preventDefault();
      _suppressNextClick = false;
    }
  },
  { capture: true },
);

/**
 * Call from `pointerdown` on any drag source.
 * The actual drag will only begin once the pointer moves past DRAG_THRESHOLD.
 */
function beginPossibleDrag(event, kind, data) {
  if (event.button !== 0 || _ptrDrag) return;
  _ptrDrag = {
    kind,
    pieceId: data.pieceId ?? null,
    fromPos: data.fromPos ?? null,
    ghost: null,
    startX: event.clientX,
    startY: event.clientY,
    started: false,
  };
  document.addEventListener("pointermove", onPtrDragMove, { capture: true });
  document.addEventListener("pointerup", onPtrDragEnd, { capture: true });
  document.addEventListener("pointercancel", onPtrDragEnd, { capture: true });
}

function commitDragStart() {
  if (!_ptrDrag || _ptrDrag.started) return;
  _ptrDrag.started = true;
  document.body.classList.add("is-pointer-dragging");
  state.nodeDragFrom = _ptrDrag.kind === "node_move_from" ? _ptrDrag.fromPos : null;

  const ghost = document.createElement("div");
  ghost.className = "drag-ghost";
  if (_ptrDrag.kind === "piece") {
    const def = pieceDef(_ptrDrag.pieceId);
    ghost.textContent = compactTileLabel(def, null) || def?.label || _ptrDrag.pieceId || "tile";
  } else if (_ptrDrag.kind === "node_move_from") {
    const entry = _ptrDrag.fromPos ? nodeByPos(_ptrDrag.fromPos) : null;
    const def = entry ? pieceDef(entry.node.piece_id) : null;
    ghost.textContent = compactTileLabel(def, entry);
  } else {
    ghost.textContent = "connect";
  }
  ghost.style.left = `${_ptrDrag.startX}px`;
  ghost.style.top = `${_ptrDrag.startY}px`;
  document.body.append(ghost);
  _ptrDrag.ghost = ghost;

  closeGridPiecePicker();
  renderCanvas();
}

function onPtrDragMove(event) {
  if (!_ptrDrag) return;

  if (!_ptrDrag.started) {
    const dx = event.clientX - _ptrDrag.startX;
    const dy = event.clientY - _ptrDrag.startY;
    if (Math.abs(dx) < DRAG_THRESHOLD && Math.abs(dy) < DRAG_THRESHOLD) return;
    commitDragStart();
  }

  event.preventDefault();
  event.stopPropagation();

  if (_ptrDrag.ghost) {
    _ptrDrag.ghost.style.left = `${event.clientX}px`;
    _ptrDrag.ghost.style.top = `${event.clientY}px`;
  }

  const position = clientPointToGridPos(event.clientX, event.clientY);
  if (!position) {
    clearDragHover();
    return;
  }

  const hoverCell = gridCellElementAt(position);
  const isOccupied = !!nodeByPos(position);

  if (_ptrDrag.kind === "piece") {
    setDragHover(hoverCell, "piece", isOccupied ? "invalid" : "valid");
  } else if (_ptrDrag.kind === "node_move_from") {
    setDragHover(hoverCell, "node_move_from", nodeMoveDropStatus(_ptrDrag.fromPos, position));
  } else if (_ptrDrag.kind === "edge_from") {
    setDragHover(hoverCell, "edge_from", isOccupied ? edgeDropStatus(_ptrDrag.fromPos, position) : null);
  }
}

function onPtrDragEnd(event) {
  if (!_ptrDrag) return;

  const drag = _ptrDrag;
  _ptrDrag = null;

  document.removeEventListener("pointermove", onPtrDragMove, { capture: true });
  document.removeEventListener("pointerup", onPtrDragEnd, { capture: true });
  document.removeEventListener("pointercancel", onPtrDragEnd, { capture: true });

  if (drag.ghost) {
    drag.ghost.remove();
  }
  document.body.classList.remove("is-pointer-dragging");

  if (!drag.started) {
    // Pointer never moved past threshold — let the normal click fire.
    return;
  }

  _suppressNextClick = true;

  const position = clientPointToGridPos(event.clientX, event.clientY);
  clearDragState();

  if (!position) {
    setUiIssue("drop/out_of_bounds");
    renderCanvas();
    return;
  }

  const isOccupied = !!nodeByPos(position);

  if (drag.kind === "piece" && drag.pieceId) {
    if (!isOccupied) {
      void placePieceAt(drag.pieceId, position);
    } else {
      logLine("warn", `node_place: cell occupied (${position.col}, ${position.row})`);
      setUiIssue("node_place/occupied");
      renderCanvas();
    }
    return;
  }

  if (drag.kind === "node_move_from" && drag.fromPos) {
    if (!posEquals(drag.fromPos, position)) {
      const op = isOccupied
        ? { op: "node_swap", a: drag.fromPos, b: position }
        : { op: "node_move", from: drag.fromPos, to: position };
      void applyOps(isOccupied ? "node_swap_drag" : "node_move_drag", [op]);
    } else {
      renderCanvas();
    }
    return;
  }

  if (drag.kind === "edge_from" && drag.fromPos) {
    if (isOccupied) {
      void connectNodes(drag.fromPos, position, "edge_connect_drag");
    } else {
      setUiIssue("edge_connect/no_target");
      renderCanvas();
    }
    return;
  }

  renderCanvas();
}

function invokeHandle() {
  return window.__TAURI__?.core?.invoke ?? window.__TAURI_INTERNALS__?.invoke;
}

async function invokeTauri(command, args) {
  const invoke = invokeHandle();
  if (typeof invoke !== "function") {
    throw new Error("Tauri invoke API unavailable. Start with `cargo run -p src-tauri`.");
  }
  return args === undefined ? invoke(command) : invoke(command, { args });
}

function normalizeGraph(graph) {
  let nodes = [];
  if (Array.isArray(graph?.nodes)) {
    nodes = graph.nodes;
  } else if (graph?.nodes && typeof graph.nodes === "object") {
    nodes = Object.entries(graph.nodes).map(([key, node]) => ({
      position: parseNodeKey(String(key).replace(",", ":")),
      node,
    }));
  }
  const edges = Array.isArray(graph?.edges)
    ? graph.edges
    : graph?.edges && typeof graph.edges === "object"
    ? Object.values(graph.edges)
    : [];
  const normalizedNodes = nodes
    .filter((entry) => entry && entry.position && Number.isFinite(entry.position.col) && Number.isFinite(entry.position.row))
    .map((entry) => ({
      position: { col: Number(entry.position.col), row: Number(entry.position.row) },
      node: {
        piece_id: String(entry.node?.piece_id ?? ""),
        inline_params: entry.node?.inline_params && typeof entry.node.inline_params === "object"
          ? entry.node.inline_params
          : {},
        input_sides: entry.node?.input_sides && typeof entry.node.input_sides === "object"
          ? entry.node.input_sides
          : {},
        output_side: typeof entry.node?.output_side === "string" ? entry.node.output_side : null,
      },
    }));
  const normalizedEdges = edges
    .filter((edge) => edge && edge.from && edge.to_node)
    .map((edge) => ({
      id: edge.id,
      from: { col: Number(edge.from.col), row: Number(edge.from.row) },
      to_node: { col: Number(edge.to_node.col), row: Number(edge.to_node.row) },
      to_param: String(edge.to_param ?? ""),
    }));
  return {
    name: typeof graph?.name === "string" ? graph.name : "",
    nodes: normalizedNodes,
    edges: normalizedEdges,
    cols: Number.isFinite(Number(graph?.cols)) ? Number(graph.cols) : DEFAULT_GRID_COLS,
    rows: Number.isFinite(Number(graph?.rows)) ? Number(graph.rows) : DEFAULT_GRID_ROWS,
  };
}

function normalizeSemantic(semantic) {
  return {
    diagnostics: Array.isArray(semantic?.diagnostics) ? semantic.diagnostics : [],
    eval_order: Array.isArray(semantic?.eval_order) ? semantic.eval_order : [],
    terminals: Array.isArray(semantic?.terminals) ? semantic.terminals : [],
  };
}

function normalizeDiagnosticsPayload(raw) {
  if (Array.isArray(raw)) {
    return raw;
  }
  if (!raw) {
    return [];
  }
  if (typeof raw === "string") {
    const trimmed = raw.trim();
    if (!trimmed) {
      return [];
    }
    try {
      const parsed = JSON.parse(trimmed);
      return Array.isArray(parsed) ? parsed : [];
    } catch {
      return [];
    }
  }
  if (typeof raw === "object") {
    if (Array.isArray(raw.diagnostics)) {
      return raw.diagnostics;
    }
  }
  return [];
}

function diagnosticsFromInvokeError(error) {
  if (error instanceof Error) {
    return normalizeDiagnosticsPayload(error.message);
  }
  return normalizeDiagnosticsPayload(error);
}

function isErrorDiagnostic(diag) {
  const severity = String(diag?.severity ?? "error").toLowerCase();
  return severity === "error";
}

function setCatalog(defs) {
  state.catalog = Array.isArray(defs) ? defs : [];
  state.catalog.sort((left, right) => String(left.label).localeCompare(String(right.label)));
  state.catalogById = new Map(state.catalog.map((item) => [item.id, item]));
}

function compilePreviewFromApplyResult(result) {
  const semantic = normalizeSemantic(result?.semantic);
  const diagnostics = semantic.diagnostics;
  const code = typeof result?.preview_code === "string" && result.preview_code.length > 0
    ? result.preview_code
    : null;
  return {
    can_compile: !diagnostics.some(isErrorDiagnostic) && code !== null,
    code,
    exprs: [],
    diagnostics,
    eval_order: semantic.eval_order,
    terminals: semantic.terminals,
  };
}

function nodeKey(pos) {
  return `${pos.col}:${pos.row}`;
}

function edgeKey(edgeId) {
  if (typeof edgeId === "string") {
    return edgeId;
  }
  if (edgeId == null) {
    return "";
  }
  try {
    return JSON.stringify(edgeId);
  } catch {
    return String(edgeId);
  }
}

function updateEdgeAnimationState() {
  const edges = Array.isArray(state.graph?.edges) ? state.graph.edges : [];
  const current = new Set(edges.map((edge) => edgeKey(edge.id)));
  const now = Date.now();

  if (!state.edgeAnimReady) {
    state.lastEdgeKeys = current;
    state.newEdgeUntil.clear();
    state.edgeAnimReady = true;
    return;
  }

  for (const id of current) {
    if (!state.lastEdgeKeys.has(id)) {
      state.newEdgeUntil.set(id, now + 700);
    }
  }
  for (const [id, until] of state.newEdgeUntil.entries()) {
    if (!current.has(id) || until <= now) {
      state.newEdgeUntil.delete(id);
    }
  }

  state.lastEdgeKeys = current;
}

function isNewEdge(edgeId) {
  const until = state.newEdgeUntil.get(edgeKey(edgeId));
  return typeof until === "number" && until > Date.now();
}

function parseNodeKey(key) {
  const [colRaw, rowRaw] = String(key).split(":");
  return {
    col: Number(colRaw),
    row: Number(rowRaw),
  };
}

function normalizedCategory(value) {
  return String(value ?? "unknown").toLowerCase();
}

function compactTileLabel(def, entry) {
  const raw = String(def?.label ?? entry?.node?.piece_id ?? "?")
    .trim()
    .toLowerCase();
  if (!raw) {
    return "?";
  }
  if (raw.length <= 4) {
    return raw;
  }
  const noVowels = `${raw[0]}${raw.slice(1).replace(/[aeiou]/g, "")}`;
  if (noVowels.length >= 2 && noVowels.length <= 4) {
    return noVowels;
  }
  const initials = raw
    .split(/[^a-z0-9]+/)
    .filter(Boolean)
    .map((part) => part[0])
    .join("");
  if (initials.length >= 2 && initials.length <= 4) {
    return initials;
  }
  return raw.slice(0, 4);
}

function compactProjectName(name, dirty) {
  const base = String(name ?? "Untitled").trim() || "Untitled";
  return `${base}${dirty ? "*" : ""}`;
}

function displayLabel(value, fallback = "unknown") {
  if (value == null || value === "") {
    return fallback;
  }
  return String(value)
    .split("_")
    .filter(Boolean)
    .map((part) => part.charAt(0).toUpperCase() + part.slice(1))
    .join(" ");
}

function appendMetaRow(container, label, value) {
  const row = document.createElement("div");
  const key = document.createElement("span");
  key.textContent = String(label);
  const current = document.createElement("strong");
  current.textContent = String(value);
  row.append(key, current);
  container.append(row);
}

function schemaPortType(schema) {
  if (!schema) {
    return null;
  }
  if (schema.kind === "custom") {
    return String(schema.port_type ?? "");
  }
  if (schema.kind === "enum") {
    return "text";
  }
  return String(schema.kind ?? "");
}

function schemaValueKind(schema) {
  if (!schema) {
    return "unknown";
  }
  if (schema.kind === "custom") {
    return String(schema.value_kind ?? "text");
  }
  if (schema.kind === "enum") {
    return "text";
  }
  return String(schema.kind ?? "unknown");
}

function schemaKindLabel(schema) {
  return displayLabel(schemaPortType(schema) || schema?.kind || "unknown");
}

function describeTileUsage(def) {
  const category = normalizedCategory(def?.category);
  switch (category) {
    case "generator":
      return "Place it as a source tile, set inline params, then drag from output to downstream inputs.";
    case "transform":
      return "Feed a source into required inputs, tune params, then route transformed output onward.";
    case "constant":
      return "Set inline constant values and connect to compatible number/text/bool/signal parameters.";
    case "output":
      return "Use as a terminal tile. Connect playable pattern/control signals into required inputs.";
    case "control":
      return "Use trigger/signal flow to drive gating, switching, and timing across neighboring tiles.";
    default:
      return "Connect adjacent tiles by compatible side and type. Use inline params for quick local values.";
  }
}

function renderTileData(def, entry, container) {
  if (!container) {
    return;
  }
  container.replaceChildren();

  const title = document.createElement("h3");
  title.className = "tile-section-title";
  title.textContent = "What It Does";
  const description = document.createElement("p");
  description.className = "tile-empty-message";
  description.textContent = String(
    def?.description
      ?? "No description yet. This tile is available in the current graph registry.",
  );

  const usageTitle = document.createElement("h3");
  usageTitle.className = "tile-section-title";
  usageTitle.textContent = "How To Use";
  const usage = document.createElement("p");
  usage.className = "tile-empty-message";
  usage.textContent = describeTileUsage(def);

  const portsTitle = document.createElement("h3");
  portsTitle.className = "tile-section-title";
  portsTitle.textContent = "Ports";

  const ports = document.createElement("div");
  ports.className = "tile-meta-grid";

  for (const param of def?.params ?? []) {
    const currentSide = entry ? nodeInputSide(entry, param) : normalizeSide(param.side);
    const required = param.required ? "required" : "optional";
    const group = param.variadic_group ? ` • group ${param.variadic_group}` : "";
    appendMetaRow(
      ports,
      `in ${param.id}`,
      `${schemaKindLabel(param.schema)} • ${displayLabel(currentSide)} • ${required}${group}`,
    );
  }

  if (def?.output_type) {
    const outSide = entry ? nodeOutputSide(entry, def) : normalizeSide(def.output_side);
    appendMetaRow(
      ports,
      "out",
      `${displayLabel(def.output_type)} • ${displayLabel(outSide ?? "none")}`,
    );
  } else {
    appendMetaRow(ports, "out", "none (terminal)");
  }

  const nodeRef = document.createElement("p");
  nodeRef.className = "tile-empty-message";
  nodeRef.textContent = entry
    ? `${entry.node.piece_id} @ (${entry.position.col}, ${entry.position.row})`
    : String(def?.id ?? "");

  container.append(title, description, usageTitle, usage, portsTitle, ports, nodeRef);
}

function posEquals(left, right) {
  return left && right && left.col === right.col && left.row === right.row;
}

function sortedNodes() {
  if (!state.graph) {
    return [];
  }
  return [...state.graph.nodes].sort((left, right) => {
    return left.position.col - right.position.col || left.position.row - right.position.row;
  });
}

function nodeByPos(pos) {
  return sortedNodes().find((entry) => posEquals(entry.position, pos)) ?? null;
}

function edgeById(edgeId) {
  if (!edgeId || !Array.isArray(state.graph?.edges)) {
    return null;
  }
  const key = edgeKey(edgeId);
  return state.graph.edges.find((edge) => edgeKey(edge.id) === key) ?? null;
}

function selectedNodeEntry() {
  if (!state.selectedPos) {
    return null;
  }
  return nodeByPos(state.selectedPos);
}

function pieceDef(pieceId) {
  return state.catalogById.get(pieceId) ?? null;
}

function edgesForNode(pos) {
  if (!state.graph) {
    return [];
  }
  return state.graph.edges.filter((edge) => posEquals(edge.to_node, pos) || posEquals(edge.from, pos));
}

function incomingEdgeForParam(pos, paramId) {
  if (!state.graph) {
    return null;
  }
  return state.graph.edges.find((edge) => posEquals(edge.to_node, pos) && edge.to_param === paramId) ?? null;
}

function sideFromToNode(toNodePos, fromNodePos) {
  if (fromNodePos.col === toNodePos.col + 1 && fromNodePos.row === toNodePos.row) {
    return "east";
  }
  if (fromNodePos.col === toNodePos.col - 1 && fromNodePos.row === toNodePos.row) {
    return "west";
  }
  if (fromNodePos.col === toNodePos.col && fromNodePos.row === toNodePos.row - 1) {
    return "north";
  }
  if (fromNodePos.col === toNodePos.col && fromNodePos.row === toNodePos.row + 1) {
    return "south";
  }
  return null;
}

function sourceOutputType(fromPos) {
  const fromNode = nodeByPos(fromPos);
  if (!fromNode) {
    return null;
  }
  const fromDef = pieceDef(fromNode.node.piece_id);
  return fromDef?.output_type ?? null;
}

function normalizeSide(value) {
  return String(value ?? "").toLowerCase();
}

function sidesFace(left, right) {
  return (
    (left === "east" && right === "west")
    || (left === "west" && right === "east")
    || (left === "north" && right === "south")
    || (left === "south" && right === "north")
  );
}

function nodeInputSide(nodeEntry, param) {
  const override = nodeEntry?.node?.input_sides?.[param.id];
  return normalizeSide(override ?? param.side);
}

function nodeOutputSide(nodeEntry, def) {
  if (!def?.output_type) {
    return null;
  }
  const override = nodeEntry?.node?.output_side;
  return normalizeSide(override ?? def.output_side);
}

function sourceOutputSide(fromPos) {
  const fromNode = nodeByPos(fromPos);
  if (!fromNode) {
    return null;
  }
  const fromDef = pieceDef(fromNode.node.piece_id);
  return nodeOutputSide(fromNode, fromDef);
}

function schemaAcceptsType(schema, sourceType) {
  if (!schema || !sourceType) {
    return false;
  }
  if (sourceType === "any") {
    return true;
  }
  if (schema.kind === "bool") {
    return sourceType === "bool" || sourceType === "number";
  }
  const expected = schemaPortType(schema);
  return !!expected && expected === sourceType;
}

function nextDirectionInCycle(currentSide, isDefaultState) {
  if (isDefaultState) {
    return DIRECTION_CYCLE[0];
  }
  const normalized = normalizeSide(currentSide);
  const index = DIRECTION_CYCLE.indexOf(normalized);
  if (index < 0) {
    return DIRECTION_CYCLE[0];
  }
  if (index === DIRECTION_CYCLE.length - 1) {
    return null;
  }
  return DIRECTION_CYCLE[index + 1];
}

function cycleInputDirection(nodeEntry, param) {
  const hasOverride = Object.prototype.hasOwnProperty.call(nodeEntry.node.input_sides ?? {}, param.id);
  const currentSide = nodeInputSide(nodeEntry, param);
  const next = nextDirectionInCycle(currentSide, !hasOverride);
  if (!next) {
    void applyOps("param_clear_side_cycle", [
      {
        op: "param_clear_side",
        position: nodeEntry.position,
        param_id: param.id,
      },
    ]);
    return;
  }
  void applyOps("param_set_side_cycle", [
    {
      op: "param_set_side",
      position: nodeEntry.position,
      param_id: param.id,
      side: next,
    },
  ]);
}

function cycleOutputDirection(nodeEntry, def) {
  if (!def?.output_type) {
    return;
  }
  const hasOverride = nodeEntry.node.output_side != null;
  const currentSide = nodeOutputSide(nodeEntry, def);
  const next = nextDirectionInCycle(currentSide, !hasOverride);
  if (!next) {
    void applyOps("output_clear_side_cycle", [
      {
        op: "output_clear_side",
        position: nodeEntry.position,
      },
    ]);
    return;
  }
  void applyOps("output_set_side_cycle", [
    {
      op: "output_set_side",
      position: nodeEntry.position,
      side: next,
    },
  ]);
}

function pickTargetParamLocal(fromPos, toPos) {
  const toNode = nodeByPos(toPos);
  if (!toNode) {
    return null;
  }
  const toDef = pieceDef(toNode.node.piece_id);
  if (!toDef?.params) {
    return null;
  }
  const expectedSide = sideFromToNode(toPos, fromPos);
  if (!expectedSide) {
    return null;
  }

  const fromType = sourceOutputType(fromPos);
  const fromSide = sourceOutputSide(fromPos);
  if (!fromSide || !sidesFace(fromSide, expectedSide)) {
    return null;
  }
  const sideCandidates = toDef.params.filter((param) => nodeInputSide(toNode, param) === expectedSide);
  const openCandidates = sideCandidates.filter((param) => !incomingEdgeForParam(toPos, param.id));
  const typeMatched = openCandidates.filter((param) => schemaAcceptsType(param.schema, fromType));
  return typeMatched.length > 0 ? typeMatched[0].id : null;
}

function probeReasonLabel(reason) {
  switch (String(reason ?? "")) {
    case "unknown_source_node":
      return "missing source node";
    case "unknown_target_node":
      return "missing target node";
    case "unknown_source_piece":
      return "unknown source tile type";
    case "unknown_target_piece":
      return "unknown target tile type";
    case "unknown_target_param":
      return "unknown target parameter";
    case "not_adjacent":
      return "source and target must be adjacent";
    case "side_mismatch":
      return "source output side must face target side";
    case "output_from_terminal":
      return "source tile cannot output";
    case "no_param_on_target_side":
      return "target has no input on this side";
    case "target_param_occupied":
      return "target input is already connected";
    case "type_mismatch":
      return "type mismatch";
    case "no_compatible_param":
      return "no compatible target parameter";
    default:
      return "connection rejected";
  }
}

async function pickTargetParamBackend(fromPos, toPos) {
  const localFallback = pickTargetParamLocal(fromPos, toPos);
  try {
    const raw = await invokeTauri("graph_pick_target_param", {
      from: { col: fromPos.col, row: fromPos.row },
      to_node: { col: toPos.col, row: toPos.row },
      target: activeGraphTarget(),
    });
    if (!raw || typeof raw !== "object") {
      return {
        to_param: localFallback,
        reason: localFallback ? null : "no_compatible_param",
        detail: "probe returned unexpected payload",
      };
    }
    return {
      to_param:
        typeof raw.to_param === "string" && raw.to_param.length > 0
          ? raw.to_param
          : null,
      reason: typeof raw.reason === "string" ? raw.reason : null,
      detail: typeof raw.detail === "string" ? raw.detail : null,
    };
  } catch (error) {
    return {
      to_param: localFallback,
      reason: localFallback ? null : "no_compatible_param",
      detail: `probe unavailable (${error instanceof Error ? error.message : String(error)})`,
    };
  }
}

async function connectNodes(fromPos, toPos, reason) {
  if (!isWithinGrid(fromPos) || !isWithinGrid(toPos)) {
    logLine("warn", `${reason}: out-of-bounds connection`);
    return false;
  }
  if (posEquals(fromPos, toPos)) {
    return false;
  }
  if (!sideFromToNode(toPos, fromPos)) {
    logLine("warn", `${reason}: source and target must be adjacent`);
    return false;
  }
  const probe = await pickTargetParamBackend(fromPos, toPos);
  const paramId = probe?.to_param ?? null;
  if (!paramId) {
    const reasonLabel = probeReasonLabel(probe?.reason);
    const detail = probe?.detail ? ` (${probe.detail})` : "";
    logLine("warn", `${reason}: ${reasonLabel} at ${nodeKey(toPos)}${detail}`);
    return false;
  }
  return applyOps(reason, [
    {
      op: "edge_connect",
      from: { col: fromPos.col, row: fromPos.row },
      to_node: { col: toPos.col, row: toPos.row },
      to_param: paramId,
    },
  ]);
}

function edgeDropStatus(fromPos, toPos) {
  if (!fromPos || !toPos || posEquals(fromPos, toPos)) {
    return null;
  }
  if (!sideFromToNode(toPos, fromPos)) {
    return null;
  }
  if (!nodeByPos(toPos)) {
    return null;
  }
  return pickTargetParamLocal(fromPos, toPos) ? "valid" : "invalid";
}

function nodeMoveDropStatus(fromPos, toPos) {
  if (!fromPos || !toPos) {
    return null;
  }
  if (posEquals(fromPos, toPos)) {
    return null;
  }
  if (!isWithinGrid(toPos)) {
    return "invalid";
  }
  return nodeByPos(toPos) ? "swap" : "valid";
}

function invalidEdgeKeys() {
  const invalidKinds = new Set([
    "type_mismatch",
    "side_mismatch",
    "not_adjacent",
    "output_from_terminal",
    "duplicate_connection",
  ]);
  const keys = new Set();
  for (const diag of graphDiagnostics()) {
    const kind = String(diag?.kind?.kind ?? "");
    if (!invalidKinds.has(kind) || diag?.edge_id == null) {
      continue;
    }
    keys.add(edgeKey(diag.edge_id));
  }
  return keys;
}

function appendPortIndicators(nodeEl, def, nodeEntry) {
  if (!def) {
    return;
  }

  const isSelectedNode = posEquals(state.selectedPos, nodeEntry.position);
  const slots = { north: 0, south: 0, east: 0, west: 0 };
  const pushIndicator = (sideRaw, markerType, title, onClick) => {
    const side = String(sideRaw || "").toLowerCase();
    if (!(side in slots)) {
      return;
    }
    const marker = document.createElement("span");
    marker.className = `port-indicator side-${side} ${markerType}`;
    marker.style.setProperty("--slot-index", String(slots[side]));
    marker.title = title;
    if (isSelectedNode) {
      marker.classList.add("is-editable");
      marker.setAttribute("role", "button");
      marker.tabIndex = 0;
      marker.addEventListener("click", (event) => {
        event.stopPropagation();
        onClick?.();
      });
      marker.addEventListener("keydown", (event) => {
        if (event.key !== "Enter" && event.key !== " ") {
          return;
        }
        event.preventDefault();
        event.stopPropagation();
        onClick?.();
      });
    }
    slots[side] += 1;
    nodeEl.append(marker);
  };

  for (const param of def.params ?? []) {
    const schemaKind = String(param?.schema?.kind ?? "unknown");
    const hasOverride = Object.prototype.hasOwnProperty.call(nodeEntry.node.input_sides ?? {}, param.id);
    const currentSide = nodeInputSide(nodeEntry, param);
    const directionMode = hasOverride ? "override" : "default";
    pushIndicator(
      currentSide,
      "input",
      `${param.id} (${schemaKind}) • ${directionMode} • click to rotate`,
      () => cycleInputDirection(nodeEntry, param),
    );
  }
  const outSide = nodeOutputSide(nodeEntry, def);
  if (def.output_type && outSide) {
    const hasOverride = nodeEntry.node.output_side != null;
    const directionMode = hasOverride ? "override" : "default";
    pushIndicator(
      outSide,
      "output",
      `out (${def.output_type}) • ${directionMode} • click to rotate`,
      () => cycleOutputDirection(nodeEntry, def),
    );
  }
}

function adjacentInDirection(pos, side) {
  switch (side) {
    case "north":
      return { col: pos.col, row: pos.row - 1 };
    case "south":
      return { col: pos.col, row: pos.row + 1 };
    case "east":
      return { col: pos.col + 1, row: pos.row };
    case "west":
      return { col: pos.col - 1, row: pos.row };
    default:
      return { col: pos.col, row: pos.row };
  }
}

function diagToText(diag) {
  if (!diag?.kind || typeof diag.kind !== "object") {
    return "unknown diagnostic";
  }
  const kind = String(diag.kind.kind ?? "unknown");
  switch (kind) {
    case "unknown_piece":
      return `UnknownPiece: ${diag.kind.piece_id}`;
    case "unknown_node":
      return `UnknownNode at (${diag.kind.pos?.col}, ${diag.kind.pos?.row})`;
    case "unknown_param":
      return `UnknownParam: ${diag.kind.piece_id}.${diag.kind.param}`;
    case "invalid_operation":
      return `InvalidOperation: ${diag.kind.reason}`;
    case "duplicate_connection":
      return `DuplicateConnection: ${diag.kind.to_param}`;
    case "cycle":
      return `Cycle: ${JSON.stringify(diag.kind.involved ?? [])}`;
    case "no_terminal_node":
      return "NoTerminalNode";
    case "multiple_terminal_nodes":
      return `MultipleTerminalNodes: ${JSON.stringify(diag.kind.positions ?? [])}`;
    case "unreachable_node":
      return `UnreachableNode: (${diag.kind.position?.col}, ${diag.kind.position?.row})`;
    case "type_mismatch":
      return `TypeMismatch: expected=${diag.kind.expected} got=${diag.kind.got} param=${diag.kind.param}`;
    case "side_mismatch":
      return `SideMismatch: expected_side=${diag.kind.expected_side}`;
    case "not_adjacent":
      return "NotAdjacent";
    case "output_from_terminal":
      return `OutputFromTerminal: (${diag.kind.position?.col}, ${diag.kind.position?.row})`;
    case "missing_required_param":
      return `MissingRequiredParam: ${diag.kind.param}`;
    case "inline_not_allowed":
      return `InlineNotAllowed: ${diag.kind.param}`;
    case "inline_type_mismatch":
      return `InlineTypeMismatch: expected=${diag.kind.expected} param=${diag.kind.param}`;
    default:
      return JSON.stringify(diag.kind);
  }
}

function diagnosticSeverity(diag) {
  const kind = String(diag?.kind?.kind ?? "unknown");
  switch (kind) {
    case "type_mismatch":
    case "side_mismatch":
    case "not_adjacent":
    case "duplicate_connection":
    case "output_from_terminal":
      return { label: "connection", className: "status-connection" };
    case "missing_required_param":
    case "inline_not_allowed":
    case "inline_type_mismatch":
      return { label: "param", className: "status-param" };
    default:
      return { label: "structural", className: "status-structural" };
  }
}

function diagnosticTarget(diag) {
  if (diag?.site && Number.isFinite(diag.site.col) && Number.isFinite(diag.site.row)) {
    return diag.site;
  }
  const kind = diag?.kind ?? {};
  const candidates = [
    kind.position,
    kind.to_node,
    kind.to_pos,
    kind.from_pos,
    kind.pos,
  ];
  return candidates.find((pos) => pos && Number.isFinite(pos.col) && Number.isFinite(pos.row)) ?? null;
}

function jumpToDiagnostic(diag, index) {
  const target = diagnosticTarget(diag);
  if (!target) {
    logLine("warn", `${index + 1}: ${diagToText(diag)}`);
    return;
  }
  const found = nodeByPos(target);
  if (!found) {
    logLine("warn", `${index + 1}: ${diagToText(diag)} (site has no node)`);
    return;
  }
  selectNode(target);
  toggleTileInspectorModal(true);
  logLine("warn", `${index + 1}: ${diagToText(diag)}`);
}

function graphDiagnostics() {
  return Array.isArray(state.semantic?.diagnostics) ? state.semantic.diagnostics : [];
}

function currentDiagnostics() {
  if (Array.isArray(state.compilePreview?.diagnostics) && state.compilePreview.diagnostics.length > 0) {
    return state.compilePreview.diagnostics;
  }
  return graphDiagnostics();
}

function updateLastGoodCode() {
  if (previewCanRender(state.compilePreview) && typeof state.compilePreview.code === "string") {
    state.lastGoodCode = state.compilePreview.code;
  }
}

function normalizeSelectionState() {
  if (state.selectedPos && !nodeByPos(state.selectedPos)) {
    state.selectedPos = null;
  }
  if (state.selectedEdgeId && !edgeById(state.selectedEdgeId)) {
    state.selectedEdgeId = null;
  }
}

function nextRequestId(label) {
  state.requestSeq += 1;
  return `${Date.now()}-${state.requestSeq}-${label}`;
}

function gridToPx(pos) {
  return {
    x: GRID_ORIGIN_X + pos.col * CELL_W,
    y: GRID_ORIGIN_Y + pos.row * CELL_H,
  };
}

function gridCols() {
  return Number.isFinite(state.graph?.cols) ? state.graph.cols : DEFAULT_GRID_COLS;
}

function gridRows() {
  return Number.isFinite(state.graph?.rows) ? state.graph.rows : DEFAULT_GRID_ROWS;
}

function pickerIsOpen() {
  return !!controls.gridPiecePicker && !controls.gridPiecePicker.classList.contains("is-hidden");
}

function gridPosToClientPoint(position) {
  if (!controls.canvas) {
    return {
      x: Math.round(window.innerWidth / 2),
      y: 96,
    };
  }
  const rect = controls.canvas.getBoundingClientRect();
  const pt = gridToPx(position);
  return {
    x: Math.round(rect.left + pt.x - controls.canvas.scrollLeft + CELL_W / 2),
    y: Math.round(rect.top + pt.y - controls.canvas.scrollTop + CELL_H / 2),
  };
}

function clientPointToGridPos(clientX, clientY) {
  if (!controls.canvas) {
    return null;
  }
  const rect = controls.canvas.getBoundingClientRect();
  const localX = clientX - rect.left + controls.canvas.scrollLeft;
  const localY = clientY - rect.top + controls.canvas.scrollTop;
  const col = Math.floor((localX - GRID_ORIGIN_X) / CELL_W);
  const row = Math.floor((localY - GRID_ORIGIN_Y) / CELL_H);
  const position = { col, row };
  return isWithinGrid(position) ? position : null;
}

function pickerFilteredCatalog() {
  const query = String(state.gridPickerQuery ?? "").trim().toLowerCase();
  return state.catalog.filter((def) => {
    if (!query) {
      return true;
    }
    return (
      String(def.id ?? "").toLowerCase().includes(query)
      || String(def.label ?? "").toLowerCase().includes(query)
      || String(def.description ?? "").toLowerCase().includes(query)
      || normalizedCategory(def.category).includes(query)
    );
  });
}

function closeGridPiecePicker() {
  if (!controls.gridPiecePicker) {
    return;
  }
  controls.gridPiecePicker.classList.add("is-hidden");
  state.gridPickerPos = null;
  state.gridPickerQuery = "";
  if (controls.gridPiecePickerSearch) {
    controls.gridPiecePickerSearch.value = "";
  }
  if (controls.gridPiecePickerList) {
    controls.gridPiecePickerList.replaceChildren();
  }
}

function renderGridPiecePicker() {
  if (!controls.gridPiecePickerList) {
    return;
  }
  controls.gridPiecePickerList.replaceChildren();
  const target = state.gridPickerPos;
  if (!target) {
    return;
  }
  const defs = pickerFilteredCatalog();
  if (defs.length === 0) {
    const empty = document.createElement("p");
    empty.className = "tile-empty-message";
    empty.textContent = "No tiles match this search.";
    controls.gridPiecePickerList.append(empty);
    return;
  }

  const sorted = [...defs].sort((a, b) => String(a.label).localeCompare(String(b.label)));
  for (const def of sorted) {
    const item = document.createElement("button");
    item.type = "button";
    item.className = "grid-piece-picker-item";
    item.dataset.pieceId = String(def.id);
    if (def.category) {
      item.classList.add(`piece-${normalizedCategory(def.category)}`);
    }
    item.textContent = `${def.label} / ${def.id}`;
    item.title = String(def.description ?? def.id);
    item.addEventListener("pointerdown", (event) => {
      beginPossibleDrag(event, "piece", { pieceId: def.id });
    });
    item.addEventListener("click", () => {
      const placePos = state.gridPickerPos;
      closeGridPiecePicker();
      if (placePos) {
        void placePieceAt(def.id, placePos);
      }
    });
    controls.gridPiecePickerList.append(item);
  }
}

function openGridPiecePicker(position, clientX, clientY) {
  if (!controls.gridPiecePicker || !controls.gridPiecePickerSearch) {
    return;
  }
  if (nodeByPos(position)) {
    return;
  }
  state.gridPickerPos = { col: position.col, row: position.row };
  state.gridPickerQuery = "";
  controls.gridPiecePickerSearch.value = "";
  controls.gridPiecePicker.classList.remove("is-hidden");

  const anchor = Number.isFinite(clientX) && Number.isFinite(clientY)
    ? { x: clientX, y: clientY }
    : gridPosToClientPoint(position);
  const estimatedWidth = 480;
  const estimatedHeight = 600;
  const left = Math.min(Math.max(16, anchor.x), window.innerWidth - estimatedWidth - 16);
  const top = Math.min(Math.max(56, anchor.y), window.innerHeight - estimatedHeight - 16);
  controls.gridPiecePicker.style.left = `${left}px`;
  controls.gridPiecePicker.style.top = `${top}px`;

  renderGridPiecePicker();
  controls.gridPiecePickerSearch.focus();
}

function preferredPickerPosition() {
  if (state.selectedPos && isWithinGrid(state.selectedPos) && !nodeByPos(state.selectedPos)) {
    return { col: state.selectedPos.col, row: state.selectedPos.row };
  }
  return suggestedPlacement(state.selectedPos);
}

function openPickerFromSelection() {
  const position = preferredPickerPosition();
  if (!position) {
    logLine("warn", "grid full: no empty cell available");
    return;
  }
  state.selectedPos = { col: position.col, row: position.row };
  state.selectedEdgeId = null;
  renderAll();
  const anchor = gridPosToClientPoint(position);
  openGridPiecePicker(position, anchor.x, anchor.y);
}

function selectNode(pos) {
  state.selectedPos = { col: Number(pos.col), row: Number(pos.row) };
  state.selectedEdgeId = null;
  renderAll();
}

function isWithinGrid(pos) {
  return pos.col >= 0 && pos.col < gridCols() && pos.row >= 0 && pos.row < gridRows();
}

function firstFreePosition() {
  const occupied = new Set(sortedNodes().map((entry) => nodeKey(entry.position)));
  for (let row = 0; row < gridRows(); row += 1) {
    for (let col = 0; col < gridCols(); col += 1) {
      const key = `${col}:${row}`;
      if (!occupied.has(key)) {
        return { col, row };
      }
    }
  }
  return null;
}

function suggestedPlacement(basePos) {
  if (!basePos) {
    return firstFreePosition();
  }
  const occupied = new Set(sortedNodes().map((entry) => nodeKey(entry.position)));
  const candidates = [
    { col: basePos.col + 1, row: basePos.row },
    { col: basePos.col, row: basePos.row + 1 },
    { col: basePos.col - 1, row: basePos.row },
    { col: basePos.col, row: basePos.row - 1 },
  ];
  for (const candidate of candidates) {
    if (isWithinGrid(candidate) && !occupied.has(nodeKey(candidate))) {
      return candidate;
    }
  }
  return firstFreePosition();
}

async function refreshProjectSnapshot() {
  state.project = await invokeTauri("project_snapshot");
  state.initStage = await invokeTauri("project_init_snapshot");
  if (isTrickMode() && !selectedTrick()) {
    state.editorMode = "init";
    state.selectedTrickId = null;
  }
  state.compilePreview = await invokeTauri("project_compile_preview");
  updateLastGoodCode();
}

async function refreshGraphForActiveTarget() {
  if (isInitMode()) {
    return;
  }
  const target = activeGraphTarget();
  state.graph = normalizeGraph(await invokeTauri("graph_snapshot", { target }));
  updateEdgeAnimationState();
  const preview = await invokeTauri("graph_compile_preview", { target });
  state.semantic = {
    diagnostics: Array.isArray(preview?.diagnostics) ? preview.diagnostics : [],
    eval_order: Array.isArray(preview?.eval_order) ? preview.eval_order : [],
    terminals: Array.isArray(preview?.terminals) ? preview.terminals : [],
  };
  normalizeSelectionState();

  if (!state.selectedPos) {
    const first = sortedNodes()[0];
    state.selectedPos = first ? first.position : null;
  } else if (!nodeByPos(state.selectedPos)) {
    const first = sortedNodes()[0];
    state.selectedPos = first ? first.position : null;
  }

  if (state.gridPickerPos && nodeByPos(state.gridPickerPos)) {
    closeGridPiecePicker();
  }
}

async function refreshProjectAndGraph() {
  await refreshProjectSnapshot();
  await refreshGraphForActiveTarget();
}

async function loadPieceCatalog() {
  const defs = await invokeTauri("graph_piece_catalog", { target: activeGraphTarget() });
  if (!Array.isArray(defs)) {
    throw new Error("graph_piece_catalog returned non-array payload");
  }
  setCatalog(defs);
}

async function ensureProjectLoaded() {
  try {
    await invokeTauri("project_snapshot");
  } catch {
    await invokeTauri("project_new", { name: "Untitled" });
  }
}

async function applyOps(label, ops) {
  if (!Array.isArray(ops) || ops.length === 0) {
    return true;
  }
  setBusy(true);
  try {
    const result = await invokeTauri("graph_apply_ops", {
      ops,
      request_id: nextRequestId(label),
      target: activeGraphTarget(),
    });
    state.graph = normalizeGraph(result?.graph);
    state.semantic = normalizeSemantic(result?.semantic);
    updateEdgeAnimationState();
    normalizeSelectionState();
    clearDragState();
    setUiIssue("");
    await refreshProjectSnapshot();
    logLine("info", `${label}: applied`);
    const removedEdgeCount = Array.isArray(result?.removed_edges) ? result.removed_edges.length : 0;
    if (removedEdgeCount > 0) {
      logLine("warn", `${label}: removed ${removedEdgeCount} non-adjacent edge(s)`);
    }
    renderAll();
    return true;
  } catch (error) {
    const diagnostics = diagnosticsFromInvokeError(error);
    if (diagnostics.length > 0) {
      clearDragState();
      state.semantic = {
        diagnostics,
        eval_order: Array.isArray(state.semantic?.eval_order) ? state.semantic.eval_order : [],
        terminals: Array.isArray(state.semantic?.terminals) ? state.semantic.terminals : [],
      };
      await refreshProjectSnapshot();
      logLine("error", `${label}: ${diagnostics.map(diagToText).join("; ")}`);
      setUiIssue(`${label}/${String(diagnostics[0]?.kind?.kind ?? "diagnostic")}`);
      renderAll();
      return false;
    }
    clearDragState();
    logLine("error", `${label}: ${error instanceof Error ? error.message : String(error)}`);
    setUiIssue(`${label}/invoke_error`);
    return false;
  } finally {
    setBusy(false);
  }
}

async function applyInitOps(label, ops) {
  if (!Array.isArray(ops) || ops.length === 0) {
    return true;
  }
  setBusy(true);
  try {
    state.initStage = await invokeTauri("project_init_apply", { ops });
    await refreshProjectSnapshot();
    if (isRuntimeMode()) {
      await loadPieceCatalog();
    }
    if (isTrickMode()) {
      const trick = selectedTrick();
      if (!trick) {
        state.editorMode = "init";
        state.selectedTrickId = null;
      } else {
        await refreshGraphForActiveTarget();
      }
    }
    setUiIssue("");
    logLine("info", `${label}: applied`);
    renderAll();
    return true;
  } catch (error) {
    logLine("error", `${label}: ${error instanceof Error ? error.message : String(error)}`);
    setUiIssue(`${label}/invoke_error`);
    return false;
  } finally {
    setBusy(false);
  }
}

function nodePlacePrecondition(pieceId, position) {
  const def = pieceDef(pieceId);
  if (!def) {
    return {
      ok: false,
      reason: "unknown_piece",
      detail: `unknown piece '${pieceId}'`,
    };
  }
  if (!isWithinGrid(position)) {
    return {
      ok: false,
      reason: "out_of_bounds",
      detail: `out of bounds (${position.col}, ${position.row})`,
    };
  }
  if (nodeByPos(position)) {
    return {
      ok: false,
      reason: "occupied",
      detail: `cell occupied (${position.col}, ${position.row})`,
    };
  }
  return { ok: true, reason: null, detail: null };
}

async function placePieceAt(pieceId, position) {
  const precondition = nodePlacePrecondition(pieceId, position);
  if (!precondition.ok) {
    logLine("warn", `node_place[${precondition.reason}]: ${precondition.detail}`);
    setUiIssue(`node_place/${precondition.reason}`);
    return false;
  }
  const ok = await applyOps("node_place", [
    {
      op: "node_place",
      position,
      piece_id: pieceId,
    },
  ]);
  if (ok) {
    state.selectedPos = position;
    state.selectedEdgeId = null;
    renderAll();
  }
  return ok;
}

function renderCanvas() {
  if (!controls.canvasLayer || !controls.edgeLayer || !controls.canvas) {
    return;
  }
  controls.canvasGrid?.replaceChildren();
  controls.canvasLayer.replaceChildren();
  controls.edgeLayer.replaceChildren();
  if (controls.canvasGrid) {
    controls.canvasGrid.style.setProperty("--grid-cell-w", `${CELL_W}px`);
    controls.canvasGrid.style.setProperty("--grid-cell-h", `${CELL_H}px`);
    controls.canvasGrid.style.setProperty("--grid-origin-x", `${GRID_ORIGIN_X}px`);
    controls.canvasGrid.style.setProperty("--grid-origin-y", `${GRID_ORIGIN_Y}px`);
  }

  const nodes = sortedNodes();
  const posToCenter = new Map();
  const occupied = new Set(nodes.map((entry) => nodeKey(entry.position)));
  const categoryByPos = new Map(
    nodes.map((entry) => [nodeKey(entry.position), normalizedCategory(pieceDef(entry.node.piece_id)?.category)]),
  );

  let maxX = 0;
  let maxY = 0;

  for (let row = 0; row < gridRows(); row += 1) {
    for (let col = 0; col < gridCols(); col += 1) {
      const position = { col, row };
      const pt = gridToPx(position);
      const key = nodeKey(position);
      const isOccupied = occupied.has(key);

      const cell = document.createElement("button");
      cell.type = "button";
      cell.className = "grid-cell";
      if (isOccupied) {
        cell.classList.add("is-occupied");
        const category = categoryByPos.get(key);
        if (category) {
          cell.classList.add(`piece-${category.toLowerCase()}`);
        }
      }
      if (posEquals(state.selectedPos, position)) {
        cell.classList.add("is-selected");
      }
      cell.style.left = `${pt.x}px`;
      cell.style.top = `${pt.y}px`;
      cell.style.width = `${CELL_W}px`;
      cell.style.height = `${CELL_H}px`;
      cell.dataset.gridPos = nodeKey(position);
      cell.title = `Cell ${col},${row}`;
      cell.addEventListener("click", () => {
        closeGridPiecePicker();
        const existing = nodeByPos(position);
        if (existing) {
          selectNode(position);
          toggleTileInspectorModal(true);
          return;
        }
        state.selectedPos = position;
        state.selectedEdgeId = null;
        renderAll();
      });
      cell.addEventListener("contextmenu", (event) => {
        if (isOccupied) {
          return;
        }
        event.preventDefault();
        event.stopPropagation();
        openGridPiecePicker(position, event.clientX, event.clientY);
      });
      controls.canvasGrid?.append(cell);
    }
  }

  for (const entry of nodes) {
    const def = pieceDef(entry.node.piece_id);
    const pt = gridToPx(entry.position);
    maxX = Math.max(maxX, pt.x + NODE_W + GRID_ORIGIN_X);
    maxY = Math.max(maxY, pt.y + NODE_H + GRID_ORIGIN_Y);

    const node = document.createElement("div");
    node.className = "voice-node";
    node.setAttribute("role", "button");
    node.tabIndex = 0;
    if (def?.category) {
      node.classList.add(`piece-${normalizedCategory(def.category)}`);
    }
    if (posEquals(state.selectedPos, entry.position)) {
      node.classList.add("is-selected");
    }
    if (state.nodeDragFrom && posEquals(state.nodeDragFrom, entry.position)) {
      node.classList.add("is-dragging-source");
    }
    node.style.position = "absolute";
    node.style.left = `${pt.x}px`;
    node.style.top = `${pt.y}px`;
    node.style.width = `${NODE_W}px`;
    node.style.height = `${NODE_H}px`;
    node.style.minHeight = `${NODE_H}px`;
    node.dataset.gridPos = nodeKey(entry.position);
    node.title = [
      `${def?.label ?? entry.node.piece_id} • ${entry.node.piece_id}`,
      def?.description ?? "No description available.",
      `Cell ${entry.position.col},${entry.position.row}`,
    ].join("\n");

    const spriteId = typeof def?.sprite_id === "string" && def.sprite_id.length > 0
      ? def.sprite_id
      : typeof def?.sprite === "string" && def.sprite.length > 0
      ? def.sprite
      : null;
    if (spriteId) {
      node.dataset.sprite = spriteId;
    }

    const tileLabel = document.createElement("strong");
    tileLabel.className = "grid-tile-label";
    tileLabel.textContent = compactTileLabel(def, entry);

    node.append(tileLabel);
    node.dataset.pieceId = entry.node.piece_id;
    const sourcePos = { col: entry.position.col, row: entry.position.row };
    const outSide = nodeOutputSide(entry, def);

    node.addEventListener("click", () => {
      closeGridPiecePicker();
      selectNode(entry.position);
      toggleTileInspectorModal(true);
    });
    node.addEventListener("keydown", (event) => {
      if (event.key !== "Enter" && event.key !== " ") {
        return;
      }
      event.preventDefault();
      closeGridPiecePicker();
      selectNode(entry.position);
      toggleTileInspectorModal(true);
    });
    node.addEventListener("pointerdown", (event) => {
      const target = event.target;
      const dragFromOutput = !!def?.output_type
        && !!outSide
        && (
          (target instanceof Element && !!target.closest(".node-output-handle"))
          || pointerHitsOutputHandle(node, outSide, event.clientX, event.clientY)
        );
      if (dragFromOutput) {
        event.preventDefault();
      }
      beginPossibleDrag(event, dragFromOutput ? "edge_from" : "node_move_from", {
        fromPos: sourcePos,
      });
    });

    appendPortIndicators(node, def, entry);
    if (def?.output_type && outSide) {
      const outHandle = document.createElement("span");
      outHandle.className = `node-output-handle side-${outSide}`;
      outHandle.setAttribute("role", "button");
      outHandle.tabIndex = 0;
      outHandle.title = "Drag to connect output";
      outHandle.addEventListener("click", (event) => {
        event.stopPropagation();
        closeGridPiecePicker();
        state.selectedPos = entry.position;
        state.selectedEdgeId = null;
        renderAll();
        toggleTileInspectorModal(true);
      });
      outHandle.addEventListener("keydown", (event) => {
        if (event.key !== "Enter" && event.key !== " ") {
          return;
        }
        event.preventDefault();
        event.stopPropagation();
        closeGridPiecePicker();
        state.selectedPos = entry.position;
        state.selectedEdgeId = null;
        renderAll();
        toggleTileInspectorModal(true);
      });
      outHandle.addEventListener("pointerdown", (event) => {
        event.preventDefault();
        event.stopPropagation();
        beginPossibleDrag(event, "edge_from", {
          fromPos: sourcePos,
        });
      });
      node.append(outHandle);
    }

    controls.canvasLayer.append(node);

    posToCenter.set(nodeKey(entry.position), {
      x: pt.x + NODE_W / 2,
      y: pt.y + NODE_H / 2,
    });
  }

  const gridPt = gridToPx({ col: gridCols() - 1, row: gridRows() - 1 });
  const fixedWidth = gridPt.x + NODE_W + GRID_ORIGIN_X;
  const fixedHeight = gridPt.y + NODE_H + GRID_ORIGIN_Y;
  const canvasWidth = Math.max(fixedWidth, maxX);
  const canvasHeight = Math.max(fixedHeight, maxY);
  if (controls.canvasGrid) {
    controls.canvasGrid.style.width = `${canvasWidth}px`;
    controls.canvasGrid.style.height = `${canvasHeight}px`;
  }
  if (controls.canvasLayer) {
    controls.canvasLayer.style.width = `${canvasWidth}px`;
    controls.canvasLayer.style.height = `${canvasHeight}px`;
  }
  if (controls.edgeLayer) {
    controls.edgeLayer.style.width = `${canvasWidth}px`;
    controls.edgeLayer.style.height = `${canvasHeight}px`;
    controls.edgeLayer.setAttribute("width", String(canvasWidth));
    controls.edgeLayer.setAttribute("height", String(canvasHeight));
    controls.edgeLayer.setAttribute("viewBox", `0 0 ${canvasWidth} ${canvasHeight}`);
  }

  const edges = Array.isArray(state.graph?.edges) ? state.graph.edges : [];
  const invalidEdges = invalidEdgeKeys();
  for (const edge of edges) {
    const key = edgeKey(edge.id);
    const from = posToCenter.get(nodeKey(edge.from));
    const to = posToCenter.get(nodeKey(edge.to_node));
    if (!from || !to) {
      continue;
    }

    const line = document.createElementNS("http://www.w3.org/2000/svg", "line");
    line.classList.add("graph-edge-line");
    if (key === edgeKey(state.selectedEdgeId)) {
      line.classList.add("is-selected");
    }
    if (invalidEdges.has(key)) {
      line.classList.add("is-invalid");
    }
    if (isNewEdge(edge.id)) {
      line.classList.add("is-new");
    }
    line.setAttribute("x1", String(from.x));
    line.setAttribute("y1", String(from.y));
    line.setAttribute("x2", String(to.x));
    line.setAttribute("y2", String(to.y));
    line.addEventListener("click", (event) => {
      event.stopPropagation();
      state.selectedEdgeId = key;
      state.selectedPos = null;
      renderAll();
      toggleTileInspectorModal(true);
    });
    controls.edgeLayer.append(line);

    if (invalidEdges.has(key)) {
      const marker = document.createElementNS("http://www.w3.org/2000/svg", "circle");
      marker.classList.add("graph-edge-error");
      marker.setAttribute("cx", String((from.x + to.x) / 2));
      marker.setAttribute("cy", String((from.y + to.y) / 2));
      marker.setAttribute("r", "3");
      marker.setAttribute("pointer-events", "none");
      controls.edgeLayer.append(marker);
    }
  }
}

function buildParamEditor(param, nodeEntry) {
  const schema = param?.schema ?? {};
  const schemaTypeLabel = schemaKindLabel(schema);
  const valueKind = schemaValueKind(schema);
  const currentSide = nodeInputSide(nodeEntry, param);
  const defaultSide = normalizeSide(param.side);
  const wrap = document.createElement("section");
  wrap.className = "tile-param-card";

  const head = document.createElement("header");
  head.className = "tile-param-head";
  const title = document.createElement("strong");
  title.className = "tile-param-title";
  title.textContent = String(param.label ?? param.id ?? "param");
  const controlsRow = document.createElement("div");
  controlsRow.className = "tile-param-head-controls";
  const sideSelect = document.createElement("select");
  sideSelect.className = "tile-side-select";
  for (const side of ["north", "south", "east", "west"]) {
    const option = document.createElement("option");
    option.value = side;
    option.textContent = displayLabel(side);
    sideSelect.append(option);
  }
  sideSelect.value = currentSide;
  sideSelect.addEventListener("change", () => {
    if (sideSelect.value === defaultSide) {
      void applyOps("param_clear_side", [
        {
          op: "param_clear_side",
          position: nodeEntry.position,
          param_id: param.id,
        },
      ]);
      return;
    }
    void applyOps("param_set_side", [
      {
        op: "param_set_side",
        position: nodeEntry.position,
        param_id: param.id,
        side: sideSelect.value,
      },
    ]);
  });
  controlsRow.append(sideSelect);
  head.append(title, controlsRow);
  wrap.append(head);

  const paramId = document.createElement("p");
  paramId.className = "tile-param-id";
  paramId.textContent = `${param.id} • ${schemaTypeLabel} • ${param.required ? "required" : "optional"}`;
  wrap.append(paramId);

  const incoming = incomingEdgeForParam(nodeEntry.position, param.id);
  const connection = document.createElement("div");
  connection.className = "tile-param-connection";
  const connectionText = document.createElement("span");
  connectionText.className = "tile-param-connection-text";
  connection.append(connectionText);

  if (incoming) {
    connectionText.textContent = `Connected from (${incoming.from.col}, ${incoming.from.row})`;
    const disconnect = document.createElement("button");
    disconnect.type = "button";
    disconnect.className = "tile-action-btn";
    disconnect.textContent = "Disconnect";
    disconnect.addEventListener("click", () => {
      void applyOps("edge_disconnect", [{ op: "edge_disconnect", edge_id: incoming.id }]);
    });
    connection.append(disconnect);
  } else {
    const expected = adjacentInDirection(nodeEntry.position, currentSide);
    const expectedNode = nodeByPos(expected);
    if (!expectedNode) {
      connectionText.textContent = `Need source at (${expected.col}, ${expected.row}).`;
    } else {
      const fromType = sourceOutputType(expected);
      const fromSide = sourceOutputSide(expected);
      if (!fromType || !fromSide) {
        connectionText.textContent = `Neighbor (${expected.col}, ${expected.row}) cannot output.`;
      } else if (!sidesFace(fromSide, currentSide)) {
        connectionText.textContent = `Source side ${displayLabel(fromSide)} must face ${displayLabel(currentSide)}.`;
      } else {
        if (!schemaAcceptsType(schema, fromType)) {
          connectionText.textContent =
            `Type hint mismatch (backend will decide): got ${displayLabel(fromType)}, need ${schemaTypeLabel}.`;
        } else {
          connectionText.textContent = `Ready to connect from (${expected.col}, ${expected.row}).`;
        }
        const connect = document.createElement("button");
        connect.type = "button";
        connect.className = "tile-action-btn";
        connect.textContent = "Connect";
        connect.addEventListener("click", () => {
          void applyOps("edge_connect", [
            {
              op: "edge_connect",
              from: expected,
              to_node: nodeEntry.position,
              to_param: param.id,
            },
          ]);
        });
        connection.append(connect);
      }
    }
  }
  wrap.append(connection);

  if (schema.can_inline) {
    const inlineRow = document.createElement("div");
    inlineRow.className = "tile-inline-row";

    const inlineMap = nodeEntry.node.inline_params ?? {};
    const currentInline = Object.prototype.hasOwnProperty.call(inlineMap, param.id)
      ? inlineMap[param.id]
      : Object.prototype.hasOwnProperty.call(schema, "default")
      ? schema.default
      : null;

    let input;
    if (schema.kind === "enum") {
      input = document.createElement("select");
      input.className = "tile-side-select";
      for (const optionValue of Array.isArray(schema.options) ? schema.options : []) {
        const option = document.createElement("option");
        option.value = String(optionValue);
        option.textContent = String(optionValue);
        input.append(option);
      }
      input.value = String(currentInline ?? schema.default ?? "");
    } else if (valueKind === "bool") {
      input = document.createElement("input");
      input.className = "tile-inline-input";
      input.type = "checkbox";
      input.checked = Boolean(currentInline);
    } else if (valueKind === "json") {
      input = document.createElement("textarea");
      input.className = "tile-inline-input init-textarea";
      input.spellcheck = false;
      input.placeholder = '{"value":true}';
      input.value = currentInline == null ? "" : JSON.stringify(currentInline, null, 2);
    } else {
      input = document.createElement("input");
      input.className = "tile-inline-input";
      input.type = valueKind === "number" ? "number" : "text";
      input.placeholder = valueKind === "number" ? "0" : "value";
      if (valueKind === "number") {
        input.step = "0.1";
        if (Number.isFinite(schema.min)) {
          input.min = String(schema.min);
        }
        if (Number.isFinite(schema.max)) {
          input.max = String(schema.max);
        }
      }
      if (currentInline != null) {
        input.value = String(currentInline);
      }
    }

    const buttons = document.createElement("div");
    buttons.className = "tile-inline-actions";

    const save = document.createElement("button");
    save.type = "button";
    save.className = "tile-action-btn";
    save.textContent = "Set";
    save.addEventListener("click", () => {
      let value;
      if (schema.kind === "enum") {
        value = String(input.value);
      } else if (valueKind === "bool") {
        value = Boolean(input.checked);
      } else if (valueKind === "json") {
        try {
          value = input.value.trim() ? JSON.parse(input.value) : null;
        } catch (error) {
          logLine("warn", `inline value for '${param.id}' must be valid JSON`);
          return;
        }
      } else if (valueKind === "number") {
        value = Number(input.value);
        if (!Number.isFinite(value)) {
          logLine("warn", `inline value for '${param.id}' must be a number`);
          return;
        }
        if (Number.isFinite(schema.min) && value < schema.min) {
          value = schema.min;
        }
        if (Number.isFinite(schema.max) && value > schema.max) {
          value = schema.max;
        }
      } else {
        value = String(input.value);
      }
      void applyOps("param_set_inline", [
        {
          op: "param_set_inline",
          position: nodeEntry.position,
          param_id: param.id,
          value: value ?? null,
        },
      ]);
    });

    const clear = document.createElement("button");
    clear.type = "button";
    clear.className = "tile-action-btn";
    clear.textContent = "Clear";
    clear.addEventListener("click", () => {
      void applyOps("param_clear_inline", [
        {
          op: "param_clear_inline",
          position: nodeEntry.position,
          param_id: param.id,
        },
      ]);
    });

    buttons.append(save, clear);
    inlineRow.append(input, buttons);
    wrap.append(inlineRow);
  } else {
    const hint = document.createElement("p");
    hint.className = "tile-param-hint";
    hint.textContent = "Inline editing disabled. Connect from adjacent tile.";
    wrap.append(hint);
  }

  return wrap;
}

function renderInspector() {
  const entry = selectedNodeEntry();
  const def = entry ? pieceDef(entry.node.piece_id) : null;
  const selectedEdge = edgeById(state.selectedEdgeId);

  if (controls.selectedNodeLabel) {
    controls.selectedNodeLabel.textContent = selectedEdge
      ? `Edge: ${selectedEdge.id}`
      : entry
      ? `Node: (${entry.position.col}, ${entry.position.row})`
      : "Node: none";
  }
  if (controls.selectedRoleLabel) {
    controls.selectedRoleLabel.textContent = selectedEdge
      ? `From (${selectedEdge.from.col}, ${selectedEdge.from.row}) -> (${selectedEdge.to_node.col}, ${selectedEdge.to_node.row})`
      : entry
      ? `Piece: ${entry.node.piece_id}`
      : "Piece: none";
  }

  if (controls.tilePreview) {
    controls.tilePreview.className = "tile-preview-box";
    controls.tilePreview.replaceChildren();
    const label = document.createElement("span");
    label.className = "tile-preview-label";
    const sub = document.createElement("span");
    sub.className = "tile-preview-sub";
    if (selectedEdge) {
      label.textContent = "Edge Selected";
      sub.textContent = `${selectedEdge.from.col},${selectedEdge.from.row} -> ${selectedEdge.to_node.col},${selectedEdge.to_node.row}`;
      controls.tilePreview.classList.add("is-edge");
    } else if (entry && def) {
      label.textContent = String(def.label ?? entry.node.piece_id);
      sub.textContent = `${displayLabel(def.category)} tile @ (${entry.position.col}, ${entry.position.row})`;
      controls.tilePreview.classList.add(`piece-${normalizedCategory(def.category)}`);
    } else {
      label.textContent = "No Tile Selected";
      sub.textContent = "Pick or place a tile on the grid.";
    }
    controls.tilePreview.append(label, sub);
  }

  if (controls.nodeCodeEditorHost) {
    controls.nodeCodeEditorHost.classList.add("editor-host-active");
    controls.nodeCodeEditorHost.replaceChildren();
    const controlsStack = document.createElement("section");
    controlsStack.className = "tile-controls-stack";
    const dataScroll = document.createElement("section");
    dataScroll.className = "tile-data-scroll";
    if (selectedEdge) {
      const info = document.createElement("section");
      info.className = "tile-info-card";
      const title = document.createElement("h3");
      title.className = "tile-section-title";
      title.textContent = "Edge Controls";
      const details = document.createElement("div");
      details.className = "tile-meta-grid";
      appendMetaRow(details, "From", `(${selectedEdge.from.col}, ${selectedEdge.from.row})`);
      appendMetaRow(details, "To", `(${selectedEdge.to_node.col}, ${selectedEdge.to_node.row})`);
      appendMetaRow(details, "Param", selectedEdge.to_param);
      const disconnect = document.createElement("button");
      disconnect.type = "button";
      disconnect.className = "tile-action-btn";
      disconnect.textContent = "Disconnect Edge";
      disconnect.addEventListener("click", () => {
        void applyOps("edge_disconnect", [{ op: "edge_disconnect", edge_id: selectedEdge.id }]);
      });
      info.append(title, details, disconnect);
      controlsStack.append(info);
      const edgeHelp = document.createElement("p");
      edgeHelp.className = "tile-empty-message";
      edgeHelp.textContent = "Edge controls are shown above. Select a tile to see tile data and usage details.";
      dataScroll.append(edgeHelp);
    } else if (!entry || !def) {
      const placeholder = document.createElement("p");
      placeholder.className = "tile-empty-message";
      placeholder.textContent = "Select a tile to edit value and side directions.";
      dataScroll.append(placeholder);
    } else {
      renderTileData(def, entry, dataScroll);

      if (def.output_type) {
        const outputCard = document.createElement("section");
        outputCard.className = "tile-param-card";
        const outputHeader = document.createElement("header");
        outputHeader.className = "tile-param-head";
        const outputTitle = document.createElement("strong");
        outputTitle.className = "tile-param-title";
        outputTitle.textContent = "Output Direction";
        const outputSelect = document.createElement("select");
        outputSelect.className = "tile-side-select";
        for (const side of ["north", "south", "east", "west"]) {
          const option = document.createElement("option");
          option.value = side;
          option.textContent = displayLabel(side);
          outputSelect.append(option);
        }
        const currentOutput = nodeOutputSide(entry, def) ?? "east";
        const defaultOutput = normalizeSide(def.output_side ?? "east");
        outputSelect.value = currentOutput;
        outputSelect.addEventListener("change", () => {
          if (outputSelect.value === defaultOutput) {
            void applyOps("output_clear_side", [
              {
                op: "output_clear_side",
                position: entry.position,
              },
            ]);
            return;
          }
          void applyOps("output_set_side", [
            {
              op: "output_set_side",
              position: entry.position,
              side: outputSelect.value,
            },
          ]);
        });
        outputHeader.append(outputTitle, outputSelect);
        const outputHint = document.createElement("p");
        outputHint.className = "tile-param-hint";
        outputHint.textContent = `Drag from ${displayLabel(currentOutput)} side to connect output.`;
        outputCard.append(outputHeader, outputHint);
        controlsStack.append(outputCard);
      }

      const paramTitle = document.createElement("h3");
      paramTitle.className = "tile-section-title";
      paramTitle.textContent = "Parameters";
      controlsStack.append(paramTitle);

      for (const param of def.params ?? []) {
        controlsStack.append(buildParamEditor(param, entry));
      }
      if ((def.params ?? []).length === 0) {
        const noParams = document.createElement("p");
        noParams.className = "tile-empty-message";
        noParams.textContent = "This tile has no parameters.";
        controlsStack.append(noParams);
      }
    }
    if (controlsStack.childElementCount > 0) {
      controls.nodeCodeEditorHost.append(controlsStack);
    }
    if (dataScroll.childElementCount > 0) {
      controls.nodeCodeEditorHost.append(dataScroll);
    }
  }
  if (controls.deleteNodeButton) {
    controls.deleteNodeButton.disabled = !entry && !selectedEdge;
    controls.deleteNodeButton.textContent = selectedEdge ? "Delete Edge" : "Delete Node";
  }

  if (controls.modalInspectorTitle) {
    controls.modalInspectorTitle.textContent = selectedEdge
      ? "Edge"
      : entry && def
      ? compactTileLabel(def, entry)
      : "Tile";
  }

  if (controls.projectTitle) {
    if (entry && def) {
      controls.projectTitle.textContent = `Tile: ${def.label ?? entry.node.piece_id}`;
    } else if (selectedEdge) {
      controls.projectTitle.textContent = "Tile Inspector: Edge";
    } else {
      controls.projectTitle.textContent = "Tile Inspector";
    }
  }
}

function renderProjectMeta() {
  const nodeCount = sortedNodes().length;
  const edgeCount = Array.isArray(state.graph?.edges) ? state.graph.edges.length : 0;
  const projectName = compactProjectName(state.project?.name, state.project?.dirty);
  const trick = selectedTrick();
  const sampleLoads = Array.isArray(state.initStage?.sample_loads) ? state.initStage.sample_loads : [];
  const readySamples = runtimeInitSampleStatus().filter((entry) => entry.status === "ready").length;
  const defaultSamples = runtimeSampleReadiness();

  if (controls.projectMeta) {
    controls.projectMeta.textContent = [
      `Nodes ${nodeCount}`,
      `Edges ${edgeCount}`,
      `Samples ${readySamples}/${sampleLoads.length}`,
      `Default ${defaultSamples.loaded}/${defaultSamples.attempted}`,
      trick ? `Trick ${trick.name}` : currentModeLabel(),
    ].join(" | ");
  }
  if (controls.projectChip) {
    controls.projectChip.textContent = projectName;
  }
  if (controls.modeChip) {
    controls.modeChip.textContent = currentModeLabel();
  }
  if (controls.statusHost) {
    controls.statusHost.textContent = `H:${runtimeStatusCode()}`;
  }

  if (controls.statusPlayback) {
    const mode = state.playback === "playing" ? "PLY" : "STP";
    controls.statusPlayback.textContent = `${mode} ${Number(controls.cpmInput?.value ?? 120).toFixed(0)}`;
  }
  if (controls.statusSelection) {
    controls.statusSelection.textContent = state.selectedEdgeId
      ? "S:E"
      : state.selectedPos
      ? `S:${state.selectedPos.col},${state.selectedPos.row}`
      : "S:-";
    controls.statusSelection.title = state.uiLastIssue ? state.uiLastIssue : "";
  }
  if (controls.statusProject) {
    controls.statusProject.textContent = `${projectName} N:${nodeCount} E:${edgeCount} S:${readySamples}/${sampleLoads.length} D:${defaultSamples.loaded}/${defaultSamples.attempted}`;
    controls.statusProject.title = String(state.project?.name ?? "Untitled");
  }
}

function renderCompileView() {
  if (!controls.textScriptInput) {
    return;
  }
  controls.textScriptInput.readOnly = true;

  if (previewCanRender(state.compilePreview) && state.compilePreview.code) {
    controls.textScriptInput.classList.remove("is-stale");
    if (controls.compileBanner) {
      controls.compileBanner.classList.add("is-hidden");
      controls.compileBanner.textContent = "";
    }
    controls.textScriptInput.value = state.compilePreview.code;
  } else {
    const diagnostics = currentDiagnostics();
    const body = diagnostics.map(diagToText).join(" | ") || "Compile blocked: unknown error.";
    if (state.lastGoodCode) {
      controls.textScriptInput.classList.add("is-stale");
      controls.textScriptInput.value = state.lastGoodCode;
      if (controls.compileBanner) {
        controls.compileBanner.classList.remove("is-hidden");
        controls.compileBanner.textContent = `Compile failed. Showing last valid code. ${body}`;
      }
    } else {
      controls.textScriptInput.classList.remove("is-stale");
      controls.textScriptInput.value = `// compile blocked\n${body}`;
      if (controls.compileBanner) {
        controls.compileBanner.classList.remove("is-hidden");
        controls.compileBanner.textContent = body;
      }
    }
  }

  const diagnostics = currentDiagnostics();
  if (controls.textImportSummary) {
    controls.textImportSummary.textContent = `${diagnostics.length} diagnostics`;
  }
  if (controls.textImportTableBody) {
    controls.textImportTableBody.replaceChildren();
    diagnostics.forEach((diag, index) => {
      const row = document.createElement("tr");
      const kind = document.createElement("td");
      kind.textContent = String(diag?.kind?.kind ?? "unknown");
      const status = document.createElement("td");
      const severity = diagnosticSeverity(diag);
      const pill = document.createElement("span");
      pill.className = `text-status-pill ${severity.className}`;
      pill.textContent = severity.label;
      status.append(pill);
      const count = document.createElement("td");
      count.textContent = "1";
      const jump = document.createElement("td");
      const btn = document.createElement("button");
      btn.type = "button";
      btn.className = "text-diag-jump";
      btn.textContent = "Show";
      btn.addEventListener("click", () => {
        jumpToDiagnostic(diag, index);
      });
      jump.append(btn);
      row.append(kind, status, count, jump);
      controls.textImportTableBody.append(row);
    });
  }
}

function currentModeLabel() {
  if (isTrickMode()) {
    const trick = selectedTrick();
    return trick ? `TRK ${trick.name}` : "TRK";
  }
  return isInitMode() ? "INIT" : "RUN";
}

function sampleStatusById() {
  return new Map(runtimeInitSampleStatus().map((entry) => [entry.id, entry]));
}

function renderInitWorkspace() {
  if (!controls.initWorkspace || !controls.gridWindow) {
    return;
  }

  const showInit = isInitMode();
  controls.initWorkspace.classList.toggle("is-hidden", !showInit);
  controls.gridWindow.classList.toggle("is-hidden", showInit);
  controls.addNodeButton?.classList.toggle("is-hidden", showInit);
  controls.workspaceBackButton?.classList.toggle("is-hidden", !isTrickMode());
  controls.workspaceRuntimeButton?.classList.toggle("is-active", isRuntimeMode());
  controls.workspaceInitButton?.classList.toggle("is-active", showInit);

  if (!showInit) {
    return;
  }

  if (controls.initCpsInput) {
    controls.initCpsInput.value = state.initStage?.cps_expr ?? "";
  }

  const runtimeSamples = runtimeSampleReadiness();
  controls.initWorkspace.querySelector(".init-runtime-note")?.remove();
  const runtimeNote = document.createElement("p");
  runtimeNote.className = "tile-empty-message init-runtime-note";
  if (runtimeSamples.failed > 0) {
    runtimeNote.textContent = `Default sample maps failed: ${runtimeSamples.failures.join(" | ")}`;
  } else if (runtimeSamples.attempted > 0) {
    runtimeNote.textContent = `Default sample maps ready: ${runtimeSamples.loaded}/${runtimeSamples.attempted}`;
  } else {
    runtimeNote.textContent = "Default sample maps have not reported readiness yet.";
  }
  controls.initWorkspace.prepend(runtimeNote);

  const sampleLoads = Array.isArray(state.initStage?.sample_loads) ? state.initStage.sample_loads : [];
  const sampleStatuses = sampleStatusById();
  if (controls.initSampleSummary) {
    controls.initSampleSummary.textContent = `${sampleLoads.length}`;
  }
  if (controls.initSampleList) {
    controls.initSampleList.replaceChildren();
    if (sampleLoads.length === 0) {
      const empty = document.createElement("p");
      empty.className = "tile-empty-message";
      empty.textContent = "No sample loads configured.";
      controls.initSampleList.append(empty);
    }
    for (const load of sampleLoads) {
      const card = document.createElement("section");
      card.className = "init-card";

      const head = document.createElement("div");
      head.className = "init-card-head";
      const title = document.createElement("strong");
      title.textContent = load.id;
      const status = document.createElement("span");
      const sampleStatus = sampleStatuses.get(load.id) ?? { status: "idle", error: null };
      status.className = `init-status is-${sampleStatus.status ?? "idle"}`;
      status.textContent = String(sampleStatus.status ?? "idle").toUpperCase();
      head.append(title, status);

      const meta = document.createElement("div");
      meta.className = "init-card-meta";
      appendMetaRow(meta, "source", load.source);
      appendMetaRow(meta, "aliases", JSON.stringify(load.aliases ?? {}));
      if (sampleStatus?.error) {
        appendMetaRow(meta, "error", sampleStatus.error);
      }

      const actions = document.createElement("div");
      actions.className = "init-card-actions";
      const edit = document.createElement("button");
      edit.type = "button";
      edit.textContent = "Edit";
      edit.addEventListener("click", () => {
        if (controls.initSampleId) controls.initSampleId.value = load.id;
        if (controls.initSampleSource) controls.initSampleSource.value = load.source;
        if (controls.initSampleAliases) {
          controls.initSampleAliases.value = JSON.stringify(load.aliases ?? {}, null, 2);
        }
      });
      const remove = document.createElement("button");
      remove.type = "button";
      remove.textContent = "Remove";
      remove.addEventListener("click", () => {
        void applyInitOps("sample_load_remove", [{ op: "sample_load_remove", id: load.id }]);
      });
      actions.append(edit, remove);

      card.append(head, meta, actions);
      controls.initSampleList.append(card);
    }
  }

  const tricks = Array.isArray(state.initStage?.tricks) ? state.initStage.tricks : [];
  if (controls.initTrickSummary) {
    controls.initTrickSummary.textContent = `${tricks.length}`;
  }
  if (controls.initTrickList) {
    controls.initTrickList.replaceChildren();
    if (tricks.length === 0) {
      const empty = document.createElement("p");
      empty.className = "tile-empty-message";
      empty.textContent = "No tricks defined yet.";
      controls.initTrickList.append(empty);
    }
    for (const trick of tricks) {
      const card = document.createElement("section");
      card.className = "init-card";

      const head = document.createElement("div");
      head.className = "init-card-head";
      const title = document.createElement("strong");
      title.textContent = trick.name;
      const badge = document.createElement("span");
      badge.className = "node-chip";
      badge.textContent = `${trick.node_count}N ${trick.edge_count}E`;
      head.append(title, badge);

      const meta = document.createElement("div");
      meta.className = "init-card-meta";
      appendMetaRow(meta, "id", trick.id);

      const actions = document.createElement("div");
      actions.className = "init-card-actions";
      const open = document.createElement("button");
      open.type = "button";
      open.textContent = "Open";
      open.addEventListener("click", () => {
        void setEditorMode("trick", trick.id);
      });
      const rename = document.createElement("button");
      rename.type = "button";
      rename.textContent = "Rename";
      rename.addEventListener("click", () => {
        const nextName = window.prompt("Rename trick", trick.name);
        if (!nextName || !nextName.trim()) {
          return;
        }
        void applyInitOps("trick_rename", [{ op: "trick_rename", id: trick.id, name: nextName.trim() }]);
      });
      const remove = document.createElement("button");
      remove.type = "button";
      remove.textContent = "Delete";
      remove.addEventListener("click", () => {
        if (isTrickMode() && state.selectedTrickId === trick.id) {
          state.selectedTrickId = null;
        }
        void applyInitOps("trick_delete", [{ op: "trick_delete", id: trick.id }]);
      });
      actions.append(open, rename, remove);

      card.append(head, meta, actions);
      controls.initTrickList.append(card);
    }
  }
}

async function setEditorMode(nextMode, trickId = null) {
  const mode = nextMode === "trick" ? "trick" : nextMode === "init" ? "init" : "runtime";
  state.editorMode = mode;
  state.selectedTrickId = mode === "trick" ? trickId : null;
  if (mode === "init") {
    toggleTileInspectorModal(false);
  }

  if (mode === "trick" && !selectedTrick()) {
    state.editorMode = "init";
    state.selectedTrickId = null;
    renderAll();
    return;
  }

  try {
    if (mode !== "init") {
      await loadPieceCatalog();
      await refreshGraphForActiveTarget();
    } else {
      clearDragState();
    }
  } catch (error) {
    logLine("error", `workspace switch failed: ${error instanceof Error ? error.message : String(error)}`);
  } finally {
    renderAll();
  }
}

async function refreshEverything(options = {}) {
  if (options.resetEditorContext) {
    state.editorMode = "runtime";
    state.selectedTrickId = null;
    state.selectedPos = null;
    state.selectedEdgeId = null;
    closeGridPiecePicker();
    clearDragState();
    toggleTileInspectorModal(false);
  }
  setBusy(true);
  try {
    await refreshProjectAndGraph();
    await loadPieceCatalog();
  } catch (error) {
    logLine("error", error instanceof Error ? error.message : String(error));
  } finally {
    renderAll();
    setBusy(false);
  }
}

function renderAll() {
  renderInitWorkspace();
  if (!isInitMode()) {
    renderCanvas();
  }
  renderInspector();
  renderCompileView();
  renderProjectMeta();
  if (pickerIsOpen()) {
    renderGridPiecePicker();
  }
}

async function handlePlay() {
  setBusy(true);
  try {
    await primeAudioFromGesture();
    const defaultSamples = runtimeSampleReadiness();
    if (defaultSamples.failed > 0) {
      logLine("warn", `default sample maps failed: ${defaultSamples.failures.join(" | ")}`);
      setUiIssue("runtime/default_sample_maps_failed");
    }
    const commit = await invokeTauri("runtime_commit", {
      cpm: Number(controls.cpmInput?.value ?? 120),
      force: false,
      playing: true,
    });
    if (!commit.success) {
      const details = Array.isArray(commit.diagnostics)
        ? commit.diagnostics.map(diagToText).join("; ")
        : commit.error || "unknown compile failure";
      if (commit.playing && state.lastGoodCode) {
        logLine("warn", `runtime_commit failed; keeping last valid playback: ${details}`);
        state.playback = "playing";
      } else {
        logLine("error", `runtime_commit failed: ${details}`);
        state.playback = "stopped";
      }
      return;
    }
    await runCadenceProgram({
      cpsExpr: commit.cps_expr,
      sampleLoads: commit.sample_loads,
      declarationCode: commit.declaration_code,
      runtimeCode: commit.runtime_code,
    });
    state.playback = "playing";
    logLine("info", "runtime playing");
  } catch (error) {
    logLine("error", `play failed: ${error instanceof Error ? error.message : String(error)}`);
    state.playback = "stopped";
  } finally {
    renderProjectMeta();
    setBusy(false);
  }
}

async function handleStop() {
  setBusy(true);
  try {
    await invokeTauri("runtime_stop");
    await stopProgram();
    state.playback = "stopped";
    logLine("info", "runtime stopped");
  } catch (error) {
    logLine("error", `stop failed: ${error instanceof Error ? error.message : String(error)}`);
  } finally {
    renderProjectMeta();
    setBusy(false);
  }
}

async function handleExport() {
  const path = window.prompt("Export song path", "song.strudel.js");
  if (!path) {
    return;
  }
  const result = await invokeTauri("export_song", { path });
  if (!result.exported) {
    const detail = result.diagnostics?.map(diagToText).join("; ") || result.message;
    logLine("error", `export failed: ${detail}`);
    return;
  }
  logLine("info", result.message);
}

async function maybeSaveBeforeDangerousAction() {
  if (!state.project?.dirty) {
    return true;
  }
  const proceed = window.confirm("Project has unsaved changes. Continue without saving?");
  return proceed;
}

async function handleMenuAction(action) {
  switch (action) {
    case "file.new": {
      if (!(await maybeSaveBeforeDangerousAction())) {
        return;
      }
      await invokeTauri("project_new", { name: "Untitled" });
      await refreshEverything({ resetEditorContext: true });
      return;
    }
    case "file.open": {
      if (!(await maybeSaveBeforeDangerousAction())) {
        return;
      }
      try {
        const path = await invokeTauri("project_pick_open_path");
        if (path) {
          await invokeTauri("project_open_path", { path });
          await refreshEverything({ resetEditorContext: true });
        }
      } catch (error) {
        logLine("error", `Failed to open project: ${error instanceof Error ? error.message : String(error)}`);
      }
      return;
    }
    case "file.save": {
      try {
        await invokeTauri("project_save", { path: null });
      } catch {
        const path = await invokeTauri("project_pick_save_path");
        if (path) {
          await invokeTauri("project_save_as", { path });
        }
      }
      await refreshEverything();
      return;
    }
    case "file.save_as": {
      const path = await invokeTauri("project_pick_save_path");
      if (path) {
        await invokeTauri("project_save_as", { path });
        await refreshEverything();
      }
      return;
    }
    case "file.export_song": {
      await handleExport();
      return;
    }
    case "edit.undo": {
      await invokeTauri("history_undo");
      await refreshEverything();
      return;
    }
    case "edit.redo": {
      await invokeTauri("history_redo");
      await refreshEverything();
      return;
    }
    case "view.toggle_mini_console": {
      state.miniConsoleVisible = !state.miniConsoleVisible;
      controls.miniConsole?.classList.toggle("is-hidden", !state.miniConsoleVisible);
      await invokeTauri("ui_set_mini_console_visible", { visible: state.miniConsoleVisible });
      return;
    }
    default:
      return;
  }
}

function isTypingTarget(target) {
  if (!(target instanceof HTMLElement)) {
    return false;
  }
  const tag = target.tagName.toLowerCase();
  return tag === "input" || tag === "textarea" || target.isContentEditable;
}

async function deleteSelection() {
  if (state.selectedEdgeId) {
    const edgeId = edgeById(state.selectedEdgeId)?.id ?? state.selectedEdgeId;
    await applyOps("edge_disconnect", [
      {
        op: "edge_disconnect",
        edge_id: edgeId,
      },
    ]);
    return;
  }
  if (!state.selectedPos || !nodeByPos(state.selectedPos)) {
    return;
  }
  await applyOps("node_remove", [
    {
      op: "node_remove",
      position: state.selectedPos,
    },
  ]);
}

function bindEvents() {
  controls.addNodeButton?.addEventListener("click", () => {
    openPickerFromSelection();
  });

  controls.workspaceRuntimeButton?.addEventListener("click", () => {
    void setEditorMode("runtime");
  });

  controls.workspaceInitButton?.addEventListener("click", () => {
    void setEditorMode("init");
  });

  controls.workspaceBackButton?.addEventListener("click", () => {
    void setEditorMode("init");
  });

  controls.compileButton?.addEventListener("click", () => {
    toggleCompileModal();
  });

  controls.initCpsSave?.addEventListener("click", () => {
    const expr = String(controls.initCpsInput?.value ?? "").trim();
    void applyInitOps("init_set_cps", [{ op: "set_cps", expr: expr || null }]);
  });

  controls.initCpsClear?.addEventListener("click", () => {
    if (controls.initCpsInput) {
      controls.initCpsInput.value = "";
    }
    void applyInitOps("init_clear_cps", [{ op: "set_cps", expr: null }]);
  });

  controls.initSampleSave?.addEventListener("click", () => {
    const id = String(controls.initSampleId?.value ?? "").trim();
    const source = String(controls.initSampleSource?.value ?? "").trim();
    const aliasesText = String(controls.initSampleAliases?.value ?? "").trim();
    if (!id || !source) {
      logLine("warn", "sample load requires both id and source");
      return;
    }
    let aliases = {};
    if (aliasesText) {
      try {
        const parsed = JSON.parse(aliasesText);
        if (!parsed || Array.isArray(parsed) || typeof parsed !== "object") {
          throw new Error("aliases must be a JSON object");
        }
        aliases = parsed;
      } catch (error) {
        logLine("warn", `sample load aliases must be valid JSON: ${error instanceof Error ? error.message : String(error)}`);
        return;
      }
    }
    void applyInitOps("sample_load_upsert", [{ op: "sample_load_upsert", id, source, aliases }]);
  });

  controls.initTrickCreate?.addEventListener("click", () => {
    const name = String(controls.initTrickName?.value ?? "").trim();
    if (!name) {
      logLine("warn", "trick name cannot be empty");
      return;
    }
    const trickId = `${slugifyIdentifier(name, "trick")}_${Date.now().toString(36)}`;
    void applyInitOps("trick_create", [{ op: "trick_create", id: trickId, name }]).then((ok) => {
      if (ok && controls.initTrickName) {
        controls.initTrickName.value = "";
      }
    });
  });

  controls.gridPiecePickerSearch?.addEventListener("input", () => {
    state.gridPickerQuery = String(controls.gridPiecePickerSearch?.value ?? "");
    renderGridPiecePicker();
  });
  controls.gridPiecePickerSearch?.addEventListener("keydown", (event) => {
    if (event.key === "Escape") {
      event.preventDefault();
      closeGridPiecePicker();
      return;
    }
    if (event.key === "Enter") {
      event.preventDefault();
      const first = pickerFilteredCatalog()[0];
      const target = state.gridPickerPos;
      if (first && target) {
        closeGridPiecePicker();
        void placePieceAt(first.id, target);
      }
    }
  });

  controls.deleteNodeButton?.addEventListener("click", () => {
    if (state.selectedEdgeId) {
      const edgeId = edgeById(state.selectedEdgeId)?.id ?? state.selectedEdgeId;
      void applyOps("edge_disconnect", [
        {
          op: "edge_disconnect",
          edge_id: edgeId,
        },
      ]);
      return;
    }
    if (!state.selectedPos) {
      return;
    }
    void applyOps("node_remove", [
      {
        op: "node_remove",
        position: state.selectedPos,
      },
    ]);
  });

  controls.canvas?.addEventListener("contextmenu", (event) => {
    const target = event.target;
    if (target instanceof Element && target.closest(".grid-cell, .voice-node")) {
      return;
    }
    const position = clientPointToGridPos(event.clientX, event.clientY);
    if (!position || nodeByPos(position)) {
      return;
    }
    event.preventDefault();
    openGridPiecePicker(position, event.clientX, event.clientY);
  });

  controls.textRefreshButton?.addEventListener("click", () => {
    void refreshEverything();
  });

  controls.modalInspectorClose?.addEventListener("click", () => {
    toggleTileInspectorModal(false);
  });

  controls.modalCompileClose?.addEventListener("click", () => {
    toggleCompileModal(false);
  });

  controls.playButton?.addEventListener("click", () => {
    void handlePlay();
  });

  controls.stopButton?.addEventListener("click", () => {
    void handleStop();
  });

  controls.modeChip?.addEventListener("click", () => {
    toggleCompileModal();
  });

  controls.projectChip?.addEventListener("click", () => {
    if (isInitMode()) {
      return;
    }
    if (state.selectedPos || state.selectedEdgeId) {
      toggleTileInspectorModal();
    }
  });

  controls.miniConsoleToggle?.addEventListener("click", () => {
    state.miniConsoleVisible = !state.miniConsoleVisible;
    controls.miniConsole?.classList.toggle("is-hidden", !state.miniConsoleVisible);
  });

  window.addEventListener("keydown", (event) => {
    const key = event.key.toLowerCase();
    const typing = isTypingTarget(event.target);

    if (event.key === "Escape") {
      if (pickerIsOpen()) {
        closeGridPiecePicker();
        return;
      }
      if (modalIsOpen(controls.tileInspectorModal) || modalIsOpen(controls.compileModal)) {
        toggleTileInspectorModal(false);
        toggleCompileModal(false);
      }
      return;
    }

    if (typing) {
      return;
    }

    if (event.key === "Tab" && state.selectedPos && !nodeByPos(state.selectedPos)) {
      event.preventDefault();
      if (!pickerIsOpen()) {
        const anchor = gridPosToClientPoint(state.selectedPos);
        openGridPiecePicker(state.selectedPos, anchor.x, anchor.y);
      }
      return;
    }

    if (key === "i") {
      event.preventDefault();
      toggleTileInspectorModal();
      return;
    }

    if (key === "c") {
      event.preventDefault();
      toggleCompileModal();
      return;
    }

    if (event.key === "Delete" || event.key === "Backspace") {
      event.preventDefault();
      void deleteSelection();
    }
  });
  window.addEventListener("pointerdown", (event) => {
    if (!pickerIsOpen() || !controls.gridPiecePicker) {
      return;
    }
    const target = event.target;
    if (target instanceof Node && controls.gridPiecePicker.contains(target)) {
      return;
    }
    closeGridPiecePicker();
  });

  window.__CADENCE_MENU_ACTION = (action) => {
    void handleMenuAction(action);
  };
}

async function boot() {
  bindEvents();
  setBusy(true);
  try {
    if (window.__CADENCE_TEST__ !== true) {
      await ensureRuntimeReady();
      const defaultSamples = runtimeSampleReadiness();
      if (defaultSamples.failed > 0) {
        logLine("warn", `default sample maps failed during bootstrap: ${defaultSamples.failures.join(" | ")}`);
      }
    }
  } catch (error) {
    logLine("warn", `runtime bootstrap failed: ${error instanceof Error ? error.message : String(error)}`);
  }

  let bootErrors = 0;
  try {
    await ensureProjectLoaded();
  } catch (error) {
    bootErrors += 1;
    logLine("error", `project bootstrap failed: ${error instanceof Error ? error.message : String(error)}`);
  }

  try {
    await loadPieceCatalog();
  } catch (error) {
    bootErrors += 1;
    setCatalog(FALLBACK_CATALOG);
    logLine(
      "warn",
      `catalog unavailable; using fallback pieces (${error instanceof Error ? error.message : String(error)})`,
    );
  }

  try {
    await refreshProjectAndGraph();
  } catch (error) {
    bootErrors += 1;
    logLine("error", `graph snapshot failed: ${error instanceof Error ? error.message : String(error)}`);
  }

  try {
    renderAll();
    if (bootErrors === 0) {
      logLine("info", "graph-canonical editor ready");
    } else {
      logLine("warn", `boot completed with ${bootErrors} issue(s); UI still available`);
    }
  } catch (error) {
    logLine("error", `render failed: ${error instanceof Error ? error.message : String(error)}`);
  } finally {
    setBusy(false);
  }
}

void boot();
