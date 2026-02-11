const PROTOCOL_VERSION = 1;
const PLAY_SUFFIX = "\nmain.play()";

let engineReady = false;
let playing = false;
let currentRev = 0;
let currentTempo = 120;
let currentCode = "";

function emit(payload) {
  if (!window.ipc || typeof window.ipc.postMessage !== "function") {
    return;
  }
  window.ipc.postMessage(JSON.stringify(payload));
}

function emitReady() {
  emit({
    type: "ready",
    protocol_version: PROTOCOL_VERSION,
  });
}

function emitError(rev, message) {
  emit({
    type: "error",
    protocol_version: PROTOCOL_VERSION,
    rev,
    message: String(message),
  });
}

function emitStatus() {
  emit({
    type: "status",
    protocol_version: PROTOCOL_VERSION,
    playing,
    current_rev: currentRev,
  });
}

function emitValidation(requestId, ok, message = null, line = null, column = null) {
  emit({
    type: "validation",
    protocol_version: PROTOCOL_VERSION,
    request_id: Number(requestId || 0),
    ok: Boolean(ok),
    message: message == null ? null : String(message),
    line: line == null ? null : Number(line),
    column: column == null ? null : Number(column),
  });
}

async function ensureEngine() {
  if (engineReady) {
    return;
  }

  if (typeof initStrudel !== "function") {
    throw new Error("initStrudel() is unavailable. @strudel/web did not load.");
  }

  await initStrudel();

  if (typeof evaluate !== "function") {
    throw new Error("evaluate() is unavailable after initStrudel().");
  }

  if (typeof hush !== "function") {
    throw new Error("hush() is unavailable after initStrudel().");
  }

  if (typeof setcpm === "function") {
    await setcpm(currentTempo);
  }

  engineReady = true;
}

function parseValidationLocation(err) {
  if (!err || typeof err !== "object") {
    return { line: null, column: null };
  }

  const line =
    Number.isFinite(err.lineNumber) ? Number(err.lineNumber) : Number.isFinite(err.line) ? Number(err.line) : null;
  const column =
    Number.isFinite(err.column) ? Number(err.column) : Number.isFinite(err.columnNumber) ? Number(err.columnNumber) : null;

  return { line, column };
}

async function handleValidate(intent) {
  const requestId = Number(intent.request_id || 0);
  const code = String(intent.code || "");
  if (!code.trim()) {
    emitValidation(requestId, false, "draft is empty", 1, 1);
    return;
  }

  try {
    if (typeof esprima !== "undefined" && typeof esprima.parseScript === "function") {
      esprima.parseScript(code, { loc: true, tolerant: false });
    } else {
      // Fallback to JS parser in the runtime JS engine if Esprima is unavailable.
      new Function(code);
    }
    emitValidation(requestId, true, null, null, null);
  } catch (err) {
    const { line, column } = parseValidationLocation(err);
    const message = err && err.message ? err.message : String(err);
    emitValidation(requestId, false, message, line, column);
  }
}

async function handleEval(intent) {
  const rev = Number(intent.rev || 0);
  if (Number.isFinite(rev) && rev >= 0) {
    currentRev = Math.max(currentRev, rev);
  }

  currentCode = String(intent.code || "");
  if (!currentCode.trim()) {
    emitStatus();
    return;
  }

  if (playing) {
    await evaluate(`${currentCode}${PLAY_SUFFIX}`);
  } else {
    await evaluate(currentCode);
  }

  emitStatus();
}

async function handlePlay() {
  playing = true;

  if (currentCode.trim()) {
    await hush();
    await evaluate(`${currentCode}${PLAY_SUFFIX}`);
  }

  emitStatus();
}

async function handleStop() {
  playing = false;
  await hush();
  emitStatus();
}

async function handleSetTempo(intent) {
  const cpm = Number(intent.cpm);
  if (!Number.isFinite(cpm) || cpm <= 0) {
    emitError(currentRev, `Invalid tempo value '${String(intent.cpm)}'`);
    return;
  }

  currentTempo = cpm;
  if (typeof setcpm === "function") {
    await setcpm(currentTempo);
  }

  emitStatus();
}

async function handleIntent(intent) {
  if (!intent || typeof intent !== "object") {
    emitError(currentRev, "Runtime intent payload must be an object.");
    return;
  }

  if (intent.protocol_version !== PROTOCOL_VERSION) {
    emitError(
      Number(intent.rev || currentRev || 0),
      `Unsupported protocol version '${String(intent.protocol_version)}'`
    );
    return;
  }

  await ensureEngine();

  switch (intent.type) {
    case "validate":
      await handleValidate(intent);
      break;
    case "eval":
      await handleEval(intent);
      break;
    case "play":
      await handlePlay();
      break;
    case "stop":
      await handleStop();
      break;
    case "set_tempo":
      await handleSetTempo(intent);
      break;
    default:
      emitError(currentRev, `Unknown command type '${String(intent.type)}'`);
      break;
  }
}

window.__grooveatlas_handle_intent = (intent) => {
  Promise.resolve(handleIntent(intent)).catch((err) => {
    emitError(Number(intent?.rev || currentRev || 0), `Runtime command failed: ${String(err)}`);
  });
};

Promise.resolve()
  .then(ensureEngine)
  .then(() => {
    emitReady();
    emitStatus();
  })
  .catch((err) => {
    emitError(currentRev, `Failed to initialize Strudel runtime: ${String(err)}`);
  });
