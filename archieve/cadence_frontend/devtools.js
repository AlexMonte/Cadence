const statusEl = document.getElementById("status");
const runtimeView = document.getElementById("runtime-view");
const diagnosticsView = document.getElementById("diagnostics-view");

let timer = null;

function invokeHandle() {
  return window.__TAURI__?.core?.invoke ?? window.__TAURI_INTERNALS__?.invoke;
}

async function invokeTauri(command, args) {
  const invoke = invokeHandle();
  if (typeof invoke !== "function") {
    throw new Error("Tauri invoke API unavailable");
  }
  if (args === undefined) {
    return invoke(command);
  }
  return invoke(command, { args });
}

async function tick() {
  try {
    const [runtime, diagnostics] = await Promise.all([
      invokeTauri("runtime_status"),
      invokeTauri("diagnostics_snapshot"),
    ]);
    runtimeView.textContent = JSON.stringify(runtime, null, 2);
    diagnosticsView.textContent = JSON.stringify(diagnostics.entries.slice(-150), null, 2);
    statusEl.textContent = `updated ${new Date().toLocaleTimeString()}`;
  } catch (err) {
    statusEl.textContent = `error: ${err instanceof Error ? err.message : String(err)}`;
  }
}

async function bootstrap() {
  try {
    await invokeTauri("ui_set_devtools_visible", { visible: true });
  } catch {
    // no-op
  }

  await tick();
  timer = setInterval(() => {
    void tick();
  }, 900);
}

window.addEventListener("beforeunload", () => {
  if (timer) {
    clearInterval(timer);
  }
  void invokeTauri("ui_set_devtools_visible", { visible: false });
});

void bootstrap();
