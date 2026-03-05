const STRUDEL_WEB_URL = "https://unpkg.com/@strudel/web@1.2.6";

const boot = {
  ready: false,
  loading: null,
  error: null,
  sampleLibraries: {
    attempted: 0,
    loaded: 0,
    failed: 0,
    failures: [],
  },
};

const DEFAULT_SAMPLE_LIBRARIES = [
  "github:tidalcycles/dirt-samples",
  "github:felixroos/vcsl",
];

let gestureAudioContext = null;
const runtime = {
  api: null,
  evaluate: null,
};

async function callIfFunction(name) {
  const fn = window[name];
  if (typeof fn !== "function") {
    return false;
  }
  await fn();
  return true;
}

async function forceStopRuntime() {
  const api = runtime.api;
  if (api) {
    for (const key of ["hush", "silence", "allNotesOff", "stop"]) {
      const fn = api[key];
      if (typeof fn === "function") {
        try {
          await fn.call(api);
        } catch {
          // keep trying remaining stop paths
        }
      }
    }
  }

  const attempts = [];
  attempts.push(callIfFunction("hush"));
  attempts.push(callIfFunction("silence"));
  attempts.push(callIfFunction("allNotesOff"));
  attempts.push(callIfFunction("stop"));
  await Promise.allSettled(attempts);

  if (typeof runtime.evaluate === "function") {
    // fallback in case direct hush-like bindings are not exposed
    await Promise.allSettled([
      runtime.evaluate("hush()"),
      runtime.evaluate("silence()"),
    ]);
  }
}

function collectKnownAudioContexts() {
  const out = [];
  if (gestureAudioContext) {
    out.push(gestureAudioContext);
  }

  const tone = window.Tone;
  const toneContext =
    tone?.getContext?.()?.rawContext ??
    tone?.getContext?.()?.context ??
    tone?.context?.rawContext ??
    tone?.context?.context ??
    tone?.context;
  if (toneContext) {
    out.push(toneContext);
  }

  for (const key of ["audioContext", "context", "__audioContext", "__strudelAudioContext"]) {
    const candidate = window[key];
    if (candidate && typeof candidate.resume === "function") {
      out.push(candidate);
    }
  }

  return out;
}

async function resumeKnownAudioContexts() {
  const contexts = collectKnownAudioContexts();
  const unique = [];
  for (const context of contexts) {
    if (!context || unique.includes(context)) {
      continue;
    }
    unique.push(context);
  }

  for (const context of unique) {
    try {
      if (context.state === "suspended" && typeof context.resume === "function") {
        await context.resume();
      }
    } catch {
      // best effort; do not block playback
    }
  }
}

export async function primeAudioFromGesture() {
  try {
    const AudioContextCtor = window.AudioContext || window.webkitAudioContext;
    if (!gestureAudioContext && typeof AudioContextCtor === "function") {
      gestureAudioContext = new AudioContextCtor();
    }
    await resumeKnownAudioContexts();
  } catch {
    // best effort; startup continues
  }
}

function loadScript(src) {
  return new Promise((resolve, reject) => {
    const existing = document.querySelector(`script[data-grooveatlas-src=\"${src}\"]`);
    if (existing) {
      existing.addEventListener("load", () => resolve());
      existing.addEventListener("error", () => reject(new Error(`failed to load ${src}`)));
      if (existing.dataset.loaded === "1") {
        resolve();
      }
      return;
    }

    const script = document.createElement("script");
    script.src = src;
    script.async = true;
    script.dataset.grooveatlasSrc = src;
    script.onload = () => {
      script.dataset.loaded = "1";
      resolve();
    };
    script.onerror = () => reject(new Error(`failed to load ${src}`));
    document.head.appendChild(script);
  });
}

function installCompatibilityShims() {
  if (typeof window.setDefaultVoicings !== "function") {
    window.setDefaultVoicings = () => undefined;
  }
  if (typeof window.setCps !== "function" && typeof window.setcps === "function") {
    window.setCps = (...args) => window.setcps(...args);
  }
  if (typeof window.setcps !== "function" && typeof window.setCps === "function") {
    window.setcps = (...args) => window.setCps(...args);
  }
  if (typeof window.setCpm !== "function" && typeof window.setcpm === "function") {
    window.setCpm = (...args) => window.setcpm(...args);
  }
  if (typeof window.setcpm !== "function" && typeof window.setCpm === "function") {
    window.setcpm = (...args) => window.setCpm(...args);
  }
}

function normalizeLegacyScript(code) {
  return String(code ?? "");
}

function compactSnippet(code, maxLength = 180) {
  const text = String(code ?? "").replace(/\s+/g, " ").trim();
  if (!text) {
    return "";
  }
  if (text.length <= maxLength) {
    return text;
  }
  return `${text.slice(0, maxLength)}…`;
}

function resolveEvaluateFunction(api) {
  if (api && typeof api.evaluate === "function") {
    return api.evaluate.bind(api);
  }
  if (
    typeof window.evaluate === "function" &&
    window.evaluate !== document.evaluate
  ) {
    return window.evaluate.bind(window);
  }
  return null;
}

async function executeScript(source) {
  const evaluator = runtime.evaluate;
  if (typeof evaluator === "function") {
    const result = evaluator(source);
    if (result && typeof result.then === "function") {
      await result;
    }
    return;
  }
  // Final fallback when a Strudel evaluate helper is not exposed.
  window.eval(source);
}

async function initRuntime() {
  if (typeof window.initStrudel !== "function") {
    throw new Error("Strudel API missing initStrudel()");
  }

  // Keep Strudel defaults intact (including default instrument maps such as VCSL).
  // User scripts can still add/override via samples(...).
  const api = await window.initStrudel();
  runtime.api = api ?? null;
  runtime.evaluate = resolveEvaluateFunction(api);
  installCompatibilityShims();

  const samplesFn =
    (runtime.api && typeof runtime.api.samples === "function" && runtime.api.samples.bind(runtime.api)) ||
    (typeof window.samples === "function" ? window.samples.bind(window) : null);
  if (samplesFn) {
    const loads = DEFAULT_SAMPLE_LIBRARIES.map((library) => samplesFn(library));
    const results = await Promise.allSettled(loads);
    const failures = [];
    let loaded = 0;
    results.forEach((result, index) => {
      if (result.status === "fulfilled") {
        loaded += 1;
      } else {
        const reason = result.reason instanceof Error ? result.reason.message : String(result.reason ?? "unknown");
        failures.push(`${DEFAULT_SAMPLE_LIBRARIES[index]}: ${reason}`);
      }
    });
    boot.sampleLibraries = {
      attempted: DEFAULT_SAMPLE_LIBRARIES.length,
      loaded,
      failed: failures.length,
      failures,
    };
    return;
  }
  boot.sampleLibraries = {
    attempted: 0,
    loaded: 0,
    failed: 1,
    failures: ["samples() API unavailable during Strudel init"],
  };
}

export async function ensureRuntimeReady() {
  if (boot.ready) {
    return;
  }

  if (boot.loading) {
    return boot.loading;
  }

  boot.error = null;
  boot.loading = (async () => {
    try {
      await loadScript(STRUDEL_WEB_URL);
      await initRuntime();
      boot.ready = true;
      boot.error = null;
    } catch (err) {
      boot.error = err instanceof Error ? err.message : String(err);
      boot.ready = false;
      boot.sampleLibraries = {
        attempted: 0,
        loaded: 0,
        failed: 1,
        failures: [boot.error],
      };
      throw err;
    } finally {
      boot.loading = null;
    }
  })();

  return boot.loading;
}

export function runtimeBootState() {
  if (boot.error) {
    return `error: ${boot.error}`;
  }
  if (boot.ready) {
    return "ready";
  }
  if (boot.loading) {
    return "booting";
  }
  return "idle";
}

export function runtimeSampleReadiness() {
  const libraries = boot.sampleLibraries ?? {
    attempted: 0,
    loaded: 0,
    failed: 0,
    failures: [],
  };
  return {
    ready: boot.ready && libraries.failed === 0,
    attempted: libraries.attempted,
    loaded: libraries.loaded,
    failed: libraries.failed,
    failures: Array.isArray(libraries.failures) ? [...libraries.failures] : [],
  };
}

export async function evalProgram(code) {
  await ensureRuntimeReady();
  await resumeKnownAudioContexts();
  const candidates = [normalizeLegacyScript(code)];
  let lastError = null;
  let lastSource = "";
  for (const candidate of candidates) {
    try {
      await executeScript(candidate);
      return;
    } catch (error) {
      lastError = error;
      lastSource = candidate;
    }
  }
  if (lastError) {
    const message = lastError instanceof Error ? lastError.message : String(lastError);
    const snippet = compactSnippet(lastSource);
    const context = snippet ? ` | snippet: ${snippet}` : "";
    throw new Error(`runtime_eval_failed: ${message}${context}`);
  }
}

export async function stopProgram() {
  await forceStopRuntime();
}
