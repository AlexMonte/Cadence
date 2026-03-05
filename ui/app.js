import {
  ensureRuntimeReady,
  evalProgram,
  primeAudioFromGesture,
  runtimeBootState,
  stopProgram,
} from "./src/bridge/strudel.js";

const controls = {
  panelTabPalette: document.getElementById("panel-tab-palette"),
  panelTabTile: document.getElementById("panel-tab-tile"),
  panelTabCompiled: document.getElementById("panel-tab-compiled"),
  panelPanePalette: document.getElementById("panel-pane-palette"),
  panelPaneTile: document.getElementById("panel-pane-tile"),
  panelPaneCompiled: document.getElementById("panel-pane-compiled"),
  canvas: document.getElementById("canvas"),
  canvasGrid: document.getElementById("canvas-grid"),
  canvasLayer: document.getElementById("canvas-layer"),
  edgeLayer: document.getElementById("edge-layer"),
  addNodeButton: document.getElementById("add-node"),
  widgetPaletteSearch: document.getElementById("widget-palette-search"),
  widgetPaletteTabs: document.getElementById("widget-palette-tabs"),
  widgetPaletteItems: document.getElementById("widget-palette-items"),
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
  nodeNameInput: document.getElementById("node-name-input"),
  nodeNameApply: document.getElementById("node-name-apply"),
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
  statusHost: document.getElementById("status-host"),
  statusPlayback: document.getElementById("status-playback"),
  statusSelection: document.getElementById("status-selection"),
  statusProject: document.getElementById("status-project"),
};

const CELL_W = 170;
const CELL_H = 110;
const NODE_W = 140;
const NODE_H = 72;
const GRID_COLS = 9;
const GRID_ROWS = 9;
const DRAG_MIME = "application/x-grooveatlas-grid";
const PALETTE_CATEGORY_ORDER = ["generator", "transform", "constant", "output", "control", "trick"];
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
    id: "strudel.sound",
    label: "s",
    category: "generator",
    params: [{ id: "value", side: "south", schema: { kind: "text", can_inline: true } }],
    output_type: "pattern",
    output_side: "east",
    description: "Create a sample pattern via s().",
  },
  {
    id: "strudel.fast",
    label: "fast",
    category: "transform",
    params: [
      { id: "pattern", side: "west", schema: { kind: "pattern", can_inline: false } },
      { id: "factor", side: "south", schema: { kind: "number", can_inline: true } },
    ],
    output_type: "pattern",
    output_side: "east",
    description: "Speed up a pattern by a factor.",
  },
  {
    id: "strudel.gain",
    label: "gain",
    category: "transform",
    params: [
      { id: "pattern", side: "west", schema: { kind: "pattern", can_inline: false } },
      { id: "amount", side: "south", schema: { kind: "number", can_inline: true } },
    ],
    output_type: "pattern",
    output_side: "east",
    description: "Set output gain.",
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
    id: "strudel.output",
    label: "play",
    category: "output",
    params: [{ id: "pattern", side: "west", schema: { kind: "pattern", can_inline: false } }],
    output_type: null,
    output_side: null,
    description: "Terminal output node.",
  },
];

const state = {
  project: null,
  graph: null,
  semantic: null,
  compilePreview: null,
  lastGoodCode: "",
  catalog: [],
  catalogById: new Map(),
  selectedPos: null,
  selectedEdgeId: null,
  armedPieceId: null,
  paletteCategory: "all",
  nodeDragFrom: null,
  edgeDragFrom: null,
  edgeAnimReady: false,
  lastEdgeKeys: new Set(),
  newEdgeUntil: new Map(),
  activePanel: "palette",
  playback: "stopped",
  busy: false,
  miniConsoleVisible: false,
  logLines: [],
  requestSeq: 0,
};

function setBusy(nextBusy) {
  state.busy = nextBusy;
  if (controls.statusHost) {
    controls.statusHost.textContent = `HOST: ${nextBusy ? "BUSY" : "READY"} | strudel:${runtimeBootState()}`;
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
  };
}

function normalizeSemantic(semantic) {
  return {
    errors: Array.isArray(semantic?.errors) ? semantic.errors : [],
    eval_order: Array.isArray(semantic?.eval_order) ? semantic.eval_order : [],
    terminal: semantic?.terminal ?? null,
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

function setCatalog(defs) {
  state.catalog = Array.isArray(defs) ? defs : [];
  state.catalog.sort((left, right) => String(left.label).localeCompare(String(right.label)));
  state.catalogById = new Map(state.catalog.map((item) => [item.id, item]));
}

function compilePreviewFromApplyResult(result) {
  const semantic = normalizeSemantic(result?.semantic);
  const diagnostics = semantic.errors;
  const code = typeof result?.preview_code === "string" && result.preview_code.length > 0
    ? result.preview_code
    : null;
  return {
    can_compile: diagnostics.length === 0 && code !== null,
    code,
    expr: null,
    diagnostics,
    eval_order: semantic.eval_order,
    terminal: semantic.terminal,
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

function categoryLabel(category) {
  if (category === "all") {
    return "All";
  }
  return category
    .split("_")
    .filter(Boolean)
    .map((part) => part.charAt(0).toUpperCase() + part.slice(1))
    .join(" ");
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
  switch (schema.kind) {
    case "number":
      return sourceType === "number";
    case "text":
      return sourceType === "text";
    case "pattern":
      return sourceType === "pattern";
    case "rhythm":
      return sourceType === "rhythm";
    default:
      return false;
  }
}

function pickTargetParam(fromPos, toPos) {
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
  const paramId = pickTargetParam(fromPos, toPos);
  if (!paramId) {
    logLine("warn", `${reason}: no compatible target param at ${nodeKey(toPos)}`);
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
  return pickTargetParam(fromPos, toPos) ? "valid" : "invalid";
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
  return nodeByPos(toPos) ? "invalid" : "valid";
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
  for (const diag of currentDiagnostics()) {
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

  const slots = { north: 0, south: 0, east: 0, west: 0 };
  const pushIndicator = (sideRaw, markerType, title) => {
    const side = String(sideRaw || "").toLowerCase();
    if (!(side in slots)) {
      return;
    }
    const marker = document.createElement("span");
    marker.className = `port-indicator side-${side} ${markerType}`;
    marker.style.setProperty("--slot-index", String(slots[side]));
    marker.title = title;
    slots[side] += 1;
    nodeEl.append(marker);
  };

  for (const param of def.params ?? []) {
    const schemaKind = String(param?.schema?.kind ?? "unknown");
    pushIndicator(nodeInputSide(nodeEntry, param), "input", `${param.id} (${schemaKind})`);
  }
  const outSide = nodeOutputSide(nodeEntry, def);
  if (def.output_type && outSide) {
    pushIndicator(outSide, "output", `out (${def.output_type})`);
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

function setActivePanel(panel) {
  state.activePanel = panel;
  controls.panelPanePalette?.classList.toggle("is-hidden", panel !== "palette");
  controls.panelPaneTile?.classList.toggle("is-hidden", panel !== "tile");
  controls.panelPaneCompiled?.classList.toggle("is-hidden", panel !== "compiled");
  controls.panelTabPalette?.classList.toggle("is-active", panel === "palette");
  controls.panelTabTile?.classList.toggle("is-active", panel === "tile");
  controls.panelTabCompiled?.classList.toggle("is-active", panel === "compiled");
  controls.panelTabPalette?.setAttribute("aria-selected", panel === "palette" ? "true" : "false");
  controls.panelTabTile?.setAttribute("aria-selected", panel === "tile" ? "true" : "false");
  controls.panelTabCompiled?.setAttribute("aria-selected", panel === "compiled" ? "true" : "false");
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
    setActivePanel("tile");
    return;
  }
  selectNode(target);
  setActivePanel("tile");
  logLine("warn", `${index + 1}: ${diagToText(diag)}`);
}

function currentDiagnostics() {
  if (Array.isArray(state.compilePreview?.diagnostics) && state.compilePreview.diagnostics.length > 0) {
    return state.compilePreview.diagnostics;
  }
  return Array.isArray(state.semantic?.errors) ? state.semantic.errors : [];
}

function updateLastGoodCode() {
  if (state.compilePreview?.can_compile && typeof state.compilePreview.code === "string") {
    state.lastGoodCode = state.compilePreview.code;
  }
}

function parseDragPayload(event) {
  const raw = event.dataTransfer?.getData(DRAG_MIME) || event.dataTransfer?.getData("text/plain") || "";
  if (!raw) {
    return null;
  }
  try {
    const parsed = JSON.parse(raw);
    return parsed && typeof parsed === "object" ? parsed : null;
  } catch {
    return null;
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
    x: 40 + pos.col * CELL_W,
    y: 40 + pos.row * CELL_H,
  };
}

function selectNode(pos) {
  state.selectedPos = { col: Number(pos.col), row: Number(pos.row) };
  state.selectedEdgeId = null;
  renderAll();
}

function isWithinGrid(pos) {
  return pos.col >= 0 && pos.col < GRID_COLS && pos.row >= 0 && pos.row < GRID_ROWS;
}

function firstFreePosition() {
  const occupied = new Set(sortedNodes().map((entry) => nodeKey(entry.position)));
  for (let row = 0; row < GRID_ROWS; row += 1) {
    for (let col = 0; col < GRID_COLS; col += 1) {
      const key = `${col}:${row}`;
      if (!occupied.has(key)) {
        return { col, row };
      }
    }
  }
  return { col: 0, row: 0 };
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

async function refreshProjectAndGraph() {
  state.project = await invokeTauri("project_snapshot");
  state.graph = normalizeGraph(await invokeTauri("graph_snapshot"));
  updateEdgeAnimationState();
  state.compilePreview = await invokeTauri("graph_compile_preview");
  state.semantic = {
    errors: Array.isArray(state.compilePreview?.diagnostics) ? state.compilePreview.diagnostics : [],
    eval_order: Array.isArray(state.compilePreview?.eval_order) ? state.compilePreview.eval_order : [],
    terminal: state.compilePreview?.terminal ?? null,
  };
  updateLastGoodCode();
  normalizeSelectionState();

  if (!state.selectedPos) {
    const first = sortedNodes()[0];
    state.selectedPos = first ? first.position : null;
  } else if (!nodeByPos(state.selectedPos)) {
    const first = sortedNodes()[0];
    state.selectedPos = first ? first.position : null;
  }
}

async function loadPieceCatalog() {
  const defs = await invokeTauri("graph_piece_catalog");
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
    });
    state.graph = normalizeGraph(result?.graph);
    state.semantic = normalizeSemantic(result?.semantic);
    updateEdgeAnimationState();
    state.compilePreview = compilePreviewFromApplyResult(result);
    updateLastGoodCode();
    normalizeSelectionState();
    state.nodeDragFrom = null;
    state.edgeDragFrom = null;
    state.project = await invokeTauri("project_snapshot");
    logLine("info", `${label}: applied`);
    renderAll();
    return true;
  } catch (error) {
    const diagnostics = diagnosticsFromInvokeError(error);
    if (diagnostics.length > 0) {
      state.nodeDragFrom = null;
      state.edgeDragFrom = null;
      state.semantic = {
        errors: diagnostics,
        eval_order: Array.isArray(state.semantic?.eval_order) ? state.semantic.eval_order : [],
        terminal: state.semantic?.terminal ?? null,
      };
      state.compilePreview = {
        can_compile: false,
        code: null,
        expr: null,
        diagnostics,
        eval_order: state.semantic.eval_order,
        terminal: state.semantic.terminal,
      };
      logLine("error", `${label}: ${diagnostics.map(diagToText).join("; ")}`);
      renderAll();
      return false;
    }
    state.nodeDragFrom = null;
    state.edgeDragFrom = null;
    logLine("error", `${label}: ${error instanceof Error ? error.message : String(error)}`);
    return false;
  } finally {
    setBusy(false);
  }
}

async function placePieceAt(pieceId, position) {
  const def = pieceDef(pieceId);
  if (!def) {
    return;
  }
  if (!isWithinGrid(position)) {
    logLine("warn", `node_place: out of bounds (${position.col}, ${position.row})`);
    return;
  }
  if (nodeByPos(position)) {
    logLine("warn", `node_place: cell occupied (${position.col}, ${position.row})`);
    return;
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
    setActivePanel("tile");
    renderAll();
  }
}

async function placePiece(pieceId) {
  const position = suggestedPlacement(state.selectedPos);
  return placePieceAt(pieceId, position);
}

function renderPalette() {
  if (!controls.widgetPaletteItems) {
    return;
  }
  if (controls.widgetPaletteTabs) {
    controls.widgetPaletteTabs.replaceChildren();
  }
  controls.widgetPaletteItems.replaceChildren();

  const query = String(controls.widgetPaletteSearch?.value ?? "").trim().toLowerCase();

  const byCategoryAll = new Map();
  for (const def of state.catalog) {
    const category = normalizedCategory(def.category);
    if (!byCategoryAll.has(category)) {
      byCategoryAll.set(category, []);
    }
    byCategoryAll.get(category).push(def);
  }

  const orderedCategories = [
    ...PALETTE_CATEGORY_ORDER.filter((category) => byCategoryAll.has(category)),
    ...[...byCategoryAll.keys()]
      .filter((category) => !PALETTE_CATEGORY_ORDER.includes(category))
      .sort(),
  ];

  if (
    state.paletteCategory !== "all"
    && !orderedCategories.includes(state.paletteCategory)
  ) {
    state.paletteCategory = "all";
  }

  const addTab = (category, count) => {
    if (!controls.widgetPaletteTabs) {
      return;
    }
    const button = document.createElement("button");
    button.type = "button";
    button.className = "widget-palette-tab";
    if (state.paletteCategory === category) {
      button.classList.add("is-active");
    }
    button.textContent = `${categoryLabel(category)} (${count})`;
    button.addEventListener("click", () => {
      if (state.paletteCategory === category) {
        return;
      }
      state.paletteCategory = category;
      renderPalette();
    });
    controls.widgetPaletteTabs.append(button);
  };

  addTab("all", state.catalog.length);
  for (const category of orderedCategories) {
    addTab(category, (byCategoryAll.get(category) ?? []).length);
  }

  const filtered = state.catalog.filter((item) => {
    const matchesCategory =
      state.paletteCategory === "all"
      || normalizedCategory(item.category) === state.paletteCategory;
    if (!matchesCategory) {
      return false;
    }
    if (!query) {
      return true;
    }
    const description = String(item.description ?? "");
    return (
      String(item.id).toLowerCase().includes(query)
      || String(item.label).toLowerCase().includes(query)
      || normalizedCategory(item.category).includes(query)
      || description.toLowerCase().includes(query)
    );
  });

  if (filtered.length === 0) {
    const empty = document.createElement("p");
    empty.className = "widget-palette-empty";
    empty.textContent = query
      ? "No pieces match the search and category filter."
      : "No pieces available in this category.";
    controls.widgetPaletteItems.append(empty);
    return;
  }

  const byCategory = new Map();
  for (const def of filtered) {
    const category = normalizedCategory(def.category);
    if (!byCategory.has(category)) {
      byCategory.set(category, []);
    }
    byCategory.get(category).push(def);
  }
  const orderedFilteredCategories = [
    ...PALETTE_CATEGORY_ORDER.filter((category) => byCategory.has(category)),
    ...[...byCategory.keys()]
      .filter((category) => !PALETTE_CATEGORY_ORDER.includes(category))
      .sort(),
  ];

  for (const category of orderedFilteredCategories) {
    const group = document.createElement("section");
    group.className = "widget-palette-group";

    const heading = document.createElement("h3");
    heading.className = "widget-palette-group-title";
    heading.textContent = categoryLabel(category);
    if (state.paletteCategory === "all") {
      group.append(heading);
    }

    const list = document.createElement("div");
    list.className = "widget-palette-group-items";

    const defs = byCategory.get(category) ?? [];
    defs.sort((left, right) => String(left.label).localeCompare(String(right.label)));
    for (const def of defs) {
      const button = document.createElement("button");
      button.type = "button";
      button.className = "widget-palette-item";
      if (state.armedPieceId === def.id) {
        button.classList.add("is-active");
      }
      button.draggable = true;

      const symbol = document.createElement("span");
      symbol.className = "symbol";
      symbol.textContent = String(def.label ?? def.id);

      const kind = document.createElement("span");
      kind.className = "kind";
      kind.textContent = `${def.id} • ${(def.params ?? []).length} params`;

      const glyph = document.createElement("span");
      glyph.className = "palette-tile-glyph";
      glyph.setAttribute("aria-hidden", "true");
      glyph.textContent = String(def.label ?? def.id).slice(0, 1).toUpperCase();

      button.append(glyph, symbol, kind);
      if (def.description) {
        button.title = String(def.description);
      }

      button.addEventListener("click", () => {
        state.armedPieceId = state.armedPieceId === def.id ? null : def.id;
        if (state.armedPieceId) {
          logLine("info", `armed piece: ${def.id}`);
        } else {
          logLine("info", "armed piece cleared");
        }
        renderPalette();
        renderCanvas();
      });
      button.addEventListener("dblclick", () => {
        void placePiece(def.id);
        setActivePanel("tile");
      });
      button.addEventListener("dragstart", (event) => {
        const payload = JSON.stringify({ kind: "piece", piece_id: def.id });
        event.dataTransfer?.setData(DRAG_MIME, payload);
        event.dataTransfer?.setData("text/plain", payload);
        if (event.dataTransfer) {
          event.dataTransfer.effectAllowed = "copy";
        }
      });
      list.append(button);
    }

    group.append(list);
    controls.widgetPaletteItems.append(group);
  }
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
  }

  const nodes = sortedNodes();
  const posToCenter = new Map();
  const occupied = new Set(nodes.map((entry) => nodeKey(entry.position)));
  const categoryByPos = new Map(
    nodes.map((entry) => [nodeKey(entry.position), normalizedCategory(pieceDef(entry.node.piece_id)?.category)]),
  );

  let maxX = 0;
  let maxY = 0;

  for (let row = 0; row < GRID_ROWS; row += 1) {
    for (let col = 0; col < GRID_COLS; col += 1) {
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
      if (!isOccupied && state.armedPieceId) {
        cell.classList.add("is-armed-target");
      }
      if (state.nodeDragFrom) {
        const moveStatus = nodeMoveDropStatus(state.nodeDragFrom, position);
        if (moveStatus === "valid") {
          cell.classList.add("is-move-valid");
        } else if (moveStatus === "invalid" && !posEquals(state.nodeDragFrom, position)) {
          cell.classList.add("is-move-invalid");
        }
      }
      if (state.edgeDragFrom && isOccupied) {
        const dropStatus = edgeDropStatus(state.edgeDragFrom, position);
        if (dropStatus === "valid") {
          cell.classList.add("is-drop-valid");
        } else if (dropStatus === "invalid") {
          cell.classList.add("is-drop-invalid");
        }
      }
      cell.style.left = `${pt.x - 10}px`;
      cell.style.top = `${pt.y - 10}px`;
      cell.style.width = `${CELL_W - 20}px`;
      cell.style.height = `${CELL_H - 20}px`;
      cell.title = `(${col}, ${row})`;
      const cellLabel = document.createElement("span");
      cellLabel.className = "grid-cell-label";
      cellLabel.textContent = `${col},${row}`;
      cell.append(cellLabel);
      cell.addEventListener("click", () => {
        const existing = nodeByPos(position);
        if (existing) {
          selectNode(position);
          setActivePanel("tile");
          return;
        }
        if (state.armedPieceId) {
          void placePieceAt(state.armedPieceId, position);
          return;
        }
        state.selectedPos = position;
        state.selectedEdgeId = null;
        renderAll();
      });
      cell.addEventListener("dragover", (event) => {
        const payload = parseDragPayload(event);
        if (!payload) {
          return;
        }
        event.preventDefault();
        if (event.dataTransfer) {
          if (payload.kind === "piece") {
            event.dataTransfer.dropEffect = "copy";
          } else if (payload.kind === "node_move_from") {
            event.dataTransfer.dropEffect = isOccupied ? "none" : "move";
          } else {
            event.dataTransfer.dropEffect = "link";
          }
        }
      });
      cell.addEventListener("drop", (event) => {
        event.preventDefault();
        const payload = parseDragPayload(event);
        if (!payload) {
          return;
        }
        if (payload.kind === "piece" && typeof payload.piece_id === "string" && !isOccupied) {
          void placePieceAt(payload.piece_id, position);
          return;
        }
        if (
          payload.kind === "node_move_from"
          && payload.from
          && Number.isFinite(payload.from.col)
          && Number.isFinite(payload.from.row)
          && !isOccupied
        ) {
          const from = { col: Number(payload.from.col), row: Number(payload.from.row) };
          state.nodeDragFrom = null;
          if (!posEquals(from, position)) {
            void applyOps("node_move_drag", [
              {
                op: "node_move",
                from,
                to: position,
              },
            ]);
          }
          return;
        }
        if (
          payload.kind === "edge_from"
          && payload.from
          && Number.isFinite(payload.from.col)
          && Number.isFinite(payload.from.row)
          && isOccupied
        ) {
          state.edgeDragFrom = null;
          void connectNodes(
            { col: Number(payload.from.col), row: Number(payload.from.row) },
            position,
            "edge_connect_drag",
          );
        }
      });
      controls.canvasGrid?.append(cell);
    }
  }

  for (const entry of nodes) {
    const def = pieceDef(entry.node.piece_id);
    const pt = gridToPx(entry.position);
    maxX = Math.max(maxX, pt.x + NODE_W + 40);
    maxY = Math.max(maxY, pt.y + NODE_H + 40);

    const node = document.createElement("button");
    node.type = "button";
    node.className = "voice-node";
    node.draggable = true;
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
    node.style.minHeight = `${NODE_H}px`;

    const tileHeader = document.createElement("div");
    tileHeader.className = "grid-tile-head";

    const tileTitle = document.createElement("strong");
    tileTitle.className = "grid-tile-title";
    tileTitle.textContent = String(def?.label ?? entry.node.piece_id);

    const tileCategory = document.createElement("span");
    tileCategory.className = "grid-tile-category";
    tileCategory.textContent = categoryLabel(normalizedCategory(def?.category));

    tileHeader.append(tileTitle, tileCategory);

    const tileId = document.createElement("div");
    tileId.className = "grid-tile-id";
    tileId.textContent = entry.node.piece_id;

    const tilePos = document.createElement("div");
    tilePos.className = "grid-tile-pos";
    tilePos.textContent = `(${entry.position.col}, ${entry.position.row})`;

    const inlineCount = Object.keys(entry.node.inline_params ?? {}).length;
    const edgeCount = edgesForNode(entry.position).length;
    const tileMeta = document.createElement("div");
    tileMeta.className = "grid-tile-meta";
    tileMeta.textContent = `${inlineCount} inline • ${edgeCount} edges`;

    node.append(tileHeader, tileId, tilePos, tileMeta);

    node.addEventListener("click", () => {
      selectNode(entry.position);
      setActivePanel("tile");
    });
    node.addEventListener("dragstart", (event) => {
      state.nodeDragFrom = { col: entry.position.col, row: entry.position.row };
      state.edgeDragFrom = null;
      renderCanvas();
      const payload = JSON.stringify({ kind: "node_move_from", from: entry.position });
      event.dataTransfer?.setData(DRAG_MIME, payload);
      event.dataTransfer?.setData("text/plain", payload);
      if (event.dataTransfer) {
        event.dataTransfer.effectAllowed = "move";
      }
    });
    node.addEventListener("dragend", () => {
      state.nodeDragFrom = null;
      renderCanvas();
    });
    node.addEventListener("dragover", (event) => {
      const payload = parseDragPayload(event);
      if (!payload || (payload.kind !== "edge_from" && payload.kind !== "node_move_from")) {
        return;
      }
      event.preventDefault();
      if (event.dataTransfer) {
        event.dataTransfer.dropEffect = payload.kind === "node_move_from" ? "none" : "link";
      }
    });
    node.addEventListener("drop", (event) => {
      event.preventDefault();
      const payload = parseDragPayload(event);
      if (
        !payload
        || payload.kind !== "edge_from"
        || !payload.from
        || !Number.isFinite(payload.from.col)
        || !Number.isFinite(payload.from.row)
      ) {
        return;
      }
      state.edgeDragFrom = null;
      void connectNodes(
        { col: Number(payload.from.col), row: Number(payload.from.row) },
        entry.position,
        "edge_connect_drag",
      );
    });

    appendPortIndicators(node, def, entry);
    const outSide = nodeOutputSide(entry, def);
    if (def?.output_type && outSide) {
      const outHandle = document.createElement("button");
      outHandle.type = "button";
      outHandle.className = `node-output-handle side-${outSide}`;
      outHandle.title = "Drag to connect output";
      outHandle.draggable = true;
      outHandle.addEventListener("click", (event) => {
        event.stopPropagation();
        state.selectedPos = entry.position;
        state.selectedEdgeId = null;
        setActivePanel("tile");
        renderAll();
      });
      outHandle.addEventListener("dragstart", (event) => {
        event.stopPropagation();
        state.nodeDragFrom = null;
        state.edgeDragFrom = { col: entry.position.col, row: entry.position.row };
        renderCanvas();
        const payload = JSON.stringify({ kind: "edge_from", from: entry.position });
        event.dataTransfer?.setData(DRAG_MIME, payload);
        event.dataTransfer?.setData("text/plain", payload);
        if (event.dataTransfer) {
          event.dataTransfer.effectAllowed = "link";
        }
      });
      outHandle.addEventListener("dragend", () => {
        state.edgeDragFrom = null;
        renderCanvas();
      });
      node.append(outHandle);
    }

    controls.canvasLayer.append(node);

    posToCenter.set(nodeKey(entry.position), {
      x: pt.x + NODE_W / 2,
      y: pt.y + NODE_H / 2,
    });
  }

  const gridPt = gridToPx({ col: GRID_COLS - 1, row: GRID_ROWS - 1 });
  const fixedWidth = gridPt.x + NODE_W + 40;
  const fixedHeight = gridPt.y + NODE_H + 40;
  controls.canvas.style.minWidth = `${Math.max(800, fixedWidth, maxX)}px`;
  controls.canvas.style.minHeight = `${Math.max(420, fixedHeight, maxY)}px`;

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
    line.setAttribute("pointer-events", "stroke");
    line.addEventListener("click", (event) => {
      event.stopPropagation();
      state.selectedEdgeId = key;
      state.selectedPos = null;
      renderAll();
    });
    controls.edgeLayer.append(line);

    if (invalidEdges.has(key)) {
      const marker = document.createElementNS("http://www.w3.org/2000/svg", "text");
      marker.classList.add("graph-edge-error");
      marker.setAttribute("x", String((from.x + to.x) / 2));
      marker.setAttribute("y", String((from.y + to.y) / 2 - 4));
      marker.textContent = "x";
      controls.edgeLayer.append(marker);
    }
  }
}

function buildParamEditor(param, nodeEntry) {
  const schema = param?.schema ?? {};
  const schemaKind = String(schema.kind ?? "unknown");
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
  paramId.textContent = `${param.id} • ${displayLabel(schemaKind)} • ${param.required ? "required" : "optional"}`;
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
      } else if (!schemaAcceptsType(schema, fromType)) {
        connectionText.textContent =
          `Type mismatch: got ${displayLabel(fromType)}, need ${displayLabel(schemaKind)}.`;
      } else {
        connectionText.textContent = `Ready to connect from (${expected.col}, ${expected.row}).`;
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

    const input = document.createElement("input");
    input.className = "tile-inline-input";
    input.type = schemaKind === "number" ? "number" : "text";
    input.placeholder = schemaKind === "number" ? "0" : "value";
    if (schemaKind === "number") {
      input.step = "0.1";
      if (Number.isFinite(schema.min)) {
        input.min = String(schema.min);
      }
      if (Number.isFinite(schema.max)) {
        input.max = String(schema.max);
      }
    }

    const inlineMap = nodeEntry.node.inline_params ?? {};
    if (Object.prototype.hasOwnProperty.call(inlineMap, param.id)) {
      input.value = String(inlineMap[param.id]);
    } else if (Object.prototype.hasOwnProperty.call(schema, "default")) {
      input.value = String(schema.default);
    }

    const buttons = document.createElement("div");
    buttons.className = "tile-inline-actions";

    const save = document.createElement("button");
    save.type = "button";
    save.className = "tile-action-btn";
    save.textContent = "Set";
    save.addEventListener("click", () => {
      let value;
      if (schemaKind === "number") {
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
          value,
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
      controls.nodeCodeEditorHost.append(info);
    } else if (!entry || !def) {
      const placeholder = document.createElement("p");
      placeholder.className = "tile-empty-message";
      placeholder.textContent = "Select a tile to edit value and side directions.";
      controls.nodeCodeEditorHost.append(placeholder);
    } else {
      const summary = document.createElement("p");
      summary.className = "tile-empty-message";
      summary.textContent = `${entry.node.piece_id} @ (${entry.position.col}, ${entry.position.row})`;
      controls.nodeCodeEditorHost.append(summary);

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
        controls.nodeCodeEditorHost.append(outputCard);
      }

      const paramTitle = document.createElement("h3");
      paramTitle.className = "tile-section-title";
      paramTitle.textContent = "Parameters";
      controls.nodeCodeEditorHost.append(paramTitle);

      for (const param of def.params ?? []) {
        controls.nodeCodeEditorHost.append(buildParamEditor(param, entry));
      }
      if ((def.params ?? []).length === 0) {
        const noParams = document.createElement("p");
        noParams.className = "tile-empty-message";
        noParams.textContent = "This tile has no parameters.";
        controls.nodeCodeEditorHost.append(noParams);
      }
    }
  }

  if (controls.nodeNameInput) {
    controls.nodeNameInput.value = entry
      ? `${entry.position.col},${entry.position.row}`
      : "";
  }
  if (controls.nodeNameApply) {
    controls.nodeNameApply.disabled = !entry;
  }
  if (controls.deleteNodeButton) {
    controls.deleteNodeButton.disabled = !entry && !selectedEdge;
    controls.deleteNodeButton.textContent = selectedEdge ? "Delete Edge" : "Delete Node";
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

  if (controls.projectMeta) {
    controls.projectMeta.textContent = `Nodes ${nodeCount} | Edges ${edgeCount}`;
  }
  if (controls.projectChip) {
    const dirty = state.project?.dirty ? "*" : "";
    controls.projectChip.textContent = `Project: ${state.project?.name ?? "None"}${dirty}`;
  }
  if (controls.modeChip) {
    controls.modeChip.textContent = "Mode: GRAPH (Canonical)";
  }

  if (controls.statusPlayback) {
    controls.statusPlayback.textContent = `PLAYBACK: ${state.playback.toUpperCase()} | CPM ${Number(
      controls.cpmInput?.value ?? 120
    ).toFixed(0)}`;
  }
  if (controls.statusSelection) {
    if (state.selectedEdgeId) {
      controls.statusSelection.textContent = `SELECTED: EDGE ${state.selectedEdgeId}`;
    } else {
      controls.statusSelection.textContent = `SELECTED: ${state.selectedPos ? nodeKey(state.selectedPos) : "NONE"}`;
    }
  }
  if (controls.statusProject) {
    controls.statusProject.textContent = `PROJECT: ${state.project?.name ?? "NONE"}`;
  }
}

function renderCompileView() {
  if (!controls.textScriptInput) {
    return;
  }
  controls.textScriptInput.readOnly = true;

  if (state.compilePreview?.can_compile && state.compilePreview.code) {
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

async function refreshEverything() {
  setBusy(true);
  try {
    await refreshProjectAndGraph();
    renderAll();
  } catch (error) {
    logLine("error", error instanceof Error ? error.message : String(error));
  } finally {
    setBusy(false);
  }
}

function renderAll() {
  renderPalette();
  renderCanvas();
  renderInspector();
  renderCompileView();
  renderProjectMeta();
}

async function handlePlay() {
  setBusy(true);
  try {
    await primeAudioFromGesture();
    const commit = await invokeTauri("runtime_commit", {
      cpm: Number(controls.cpmInput?.value ?? 120),
      force: false,
      playing: true,
    });
    if (!commit.success || !commit.code) {
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
    await evalProgram(commit.code);
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
      await refreshEverything();
      return;
    }
    case "file.open": {
      if (!(await maybeSaveBeforeDangerousAction())) {
        return;
      }
      const path = await invokeTauri("project_pick_open_path");
      if (path) {
        await invokeTauri("project_open_path", { path });
        await refreshEverything();
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
  controls.panelTabPalette?.addEventListener("click", () => {
    setActivePanel("palette");
    renderAll();
  });
  controls.panelTabTile?.addEventListener("click", () => {
    setActivePanel("tile");
    renderAll();
  });
  controls.panelTabCompiled?.addEventListener("click", () => {
    setActivePanel("compiled");
    renderAll();
  });

  controls.addNodeButton?.addEventListener("click", () => {
    setActivePanel("palette");
    if (!state.armedPieceId) {
      logLine("info", "pick a piece in Palette, then click an empty grid cell.");
      return;
    }
    void placePiece(state.armedPieceId);
  });

  controls.widgetPaletteSearch?.addEventListener("input", () => renderPalette());

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

  controls.nodeNameApply?.addEventListener("click", () => {
    const entry = selectedNodeEntry();
    if (!entry || !controls.nodeNameInput) {
      return;
    }
    const parsed = parseNodeKey(controls.nodeNameInput.value.replace(",", ":"));
    if (!Number.isFinite(parsed.col) || !Number.isFinite(parsed.row)) {
      logLine("warn", "invalid target cell, use col,row");
      return;
    }
    const target = { col: Math.trunc(parsed.col), row: Math.trunc(parsed.row) };
    if (!isWithinGrid(target)) {
      logLine("warn", `node_move: out of bounds (${target.col}, ${target.row})`);
      return;
    }
    void applyOps("node_move", [
      {
        op: "node_move",
        from: entry.position,
        to: target,
      },
    ]);
  });

  controls.textRefreshButton?.addEventListener("click", () => {
    void refreshEverything();
  });

  controls.playButton?.addEventListener("click", () => {
    void handlePlay();
  });

  controls.stopButton?.addEventListener("click", () => {
    void handleStop();
  });

  controls.miniConsoleToggle?.addEventListener("click", () => {
    state.miniConsoleVisible = !state.miniConsoleVisible;
    controls.miniConsole?.classList.toggle("is-hidden", !state.miniConsoleVisible);
  });

  window.addEventListener("keydown", (event) => {
    if ((event.key !== "Delete" && event.key !== "Backspace") || isTypingTarget(event.target)) {
      return;
    }
    event.preventDefault();
    void deleteSelection();
  });

  window.__GA_MENU_ACTION = (action) => {
    void handleMenuAction(action);
  };
}

async function boot() {
  bindEvents();
  setActivePanel("palette");
  setBusy(true);
  try {
    await ensureRuntimeReady();
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
