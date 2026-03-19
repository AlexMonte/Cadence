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
  "github:tidalcycles/Dirt-Samples/master/",
  "https://raw.githubusercontent.com/felixroos/dough-samples/main/vcsl.json",
  "https://raw.githubusercontent.com/felixroos/dough-samples/main/tidal-drum-machines.json",
];

const SAMPLE_FETCH_CACHE_NAME = "cadence-sample-fetch-v1";
const SAMPLE_FILE_EXTENSIONS =
  /\.(aac|aif|aiff|flac|json|m4a|mp3|ogg|wav|wave)$/i;
const sampleFetchCache = {
  name: SAMPLE_FETCH_CACHE_NAME,
  installed: false,
  available: false,
  ready: false,
  hits: 0,
  misses: 0,
  writes: 0,
  lastError: null,
};

let gestureAudioContext = null;
const runtime = {
  api: null,
  evaluate: null,
};
const initSampleLoads = new Map();
let sampleFetchCachePromise = null;

function noteSampleCacheError(error) {
  sampleFetchCache.lastError =
    error instanceof Error ? error.message : String(error ?? "unknown");
}

function isSampleCacheHost(hostname) {
  return (
    hostname === "api.github.com" ||
    hostname === "cdn.jsdelivr.net" ||
    hostname === "github.com" ||
    hostname === "raw.github.com" ||
    hostname === "raw.githubusercontent.com" ||
    hostname === "unpkg.com" ||
    hostname.endsWith(".githubusercontent.com")
  );
}

function normalizeSampleRequest(input, init) {
  try {
    return input instanceof Request && init === undefined
      ? input
      : new Request(input, init);
  } catch {
    return null;
  }
}

function sampleCacheKey(request) {
  try {
    return new Request(request.url, { method: "GET" });
  } catch {
    return request;
  }
}

function shouldCacheSampleRequest(request) {
  if (!request || String(request.method ?? "GET").toUpperCase() !== "GET") {
    return false;
  }
  if (request.cache === "no-store") {
    return false;
  }
  try {
    const url = new URL(request.url, window.location.href);
    if (url.protocol !== "http:" && url.protocol !== "https:") {
      return false;
    }
    return (
      SAMPLE_FILE_EXTENSIONS.test(url.pathname) ||
      isSampleCacheHost(url.hostname.toLowerCase())
    );
  } catch {
    return false;
  }
}

async function openSampleFetchCache() {
  if (!sampleFetchCache.available) {
    return null;
  }
  if (!sampleFetchCachePromise) {
    sampleFetchCachePromise = window.caches
      .open(SAMPLE_FETCH_CACHE_NAME)
      .catch((error) => {
        sampleFetchCachePromise = null;
        sampleFetchCache.ready = false;
        noteSampleCacheError(error);
        return null;
      });
  }
  const cache = await sampleFetchCachePromise;
  sampleFetchCache.ready = !!cache;
  return cache;
}

function installSampleFetchCache() {
  if (sampleFetchCache.installed || typeof window?.fetch !== "function") {
    return;
  }

  sampleFetchCache.installed = true;
  sampleFetchCache.available = typeof window.caches?.open === "function";
  sampleFetchCache.ready = sampleFetchCache.available;

  const nativeFetch = window.fetch.bind(window);
  window.fetch = async (input, init) => {
    const request = normalizeSampleRequest(input, init);
    if (!sampleFetchCache.available || !shouldCacheSampleRequest(request)) {
      return nativeFetch(input, init);
    }

    const cache = await openSampleFetchCache();
    if (!cache) {
      return nativeFetch(input, init);
    }

    const key = sampleCacheKey(request);
    try {
      const cached = await cache.match(key);
      if (cached) {
        sampleFetchCache.hits += 1;
        return cached.clone();
      }
    } catch (error) {
      noteSampleCacheError(error);
    }

    sampleFetchCache.misses += 1;
    const response = await nativeFetch(input, init);
    if (response.ok || response.type === "opaque") {
      try {
        await cache.put(key, response.clone());
        sampleFetchCache.writes += 1;
        sampleFetchCache.lastError = null;
      } catch (error) {
        noteSampleCacheError(error);
      }
    }
    return response;
  };
}

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

  for (const key of [
    "audioContext",
    "context",
    "__audioContext",
    "__strudelAudioContext",
  ]) {
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
      if (
        context.state === "suspended" &&
        typeof context.resume === "function"
      ) {
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

const ALLOWED_SCRIPT_ORIGINS = [
  "https://unpkg.com",
  "https://cdn.jsdelivr.net",
  "https://esm.sh",
];

function isAllowedScriptSource(src) {
  try {
    const url = new URL(src, window.location.href);
    return ALLOWED_SCRIPT_ORIGINS.some((origin) => url.href.startsWith(origin));
  } catch {
    return false;
  }
}

function loadScript(src) {
  return new Promise((resolve, reject) => {
    if (!isAllowedScriptSource(src)) {
      reject(new Error(`blocked script load from untrusted source: ${src}`));
      return;
    }

    const safeSrc = CSS.escape(src);
    const existing = document.querySelector(
      `script[data-cadence-src="${safeSrc}"]`,
    );
    if (existing) {
      existing.addEventListener("load", () => resolve());
      existing.addEventListener("error", () =>
        reject(new Error(`failed to load ${src}`)),
      );
      if (existing.dataset.loaded === "1") {
        resolve();
      }
      return;
    }

    const script = document.createElement("script");
    script.src = src;
    script.async = true;
    script.dataset.cadenceSrc = src;
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
  if (
    typeof window.setCps !== "function" &&
    typeof window.setcps === "function"
  ) {
    window.setCps = (...args) => window.setcps(...args);
  }
  if (
    typeof window.setcps !== "function" &&
    typeof window.setCps === "function"
  ) {
    window.setcps = (...args) => window.setCps(...args);
  }
  if (
    typeof window.setCpm !== "function" &&
    typeof window.setcpm === "function"
  ) {
    window.setCpm = (...args) => window.setcpm(...args);
  }
  if (
    typeof window.setcpm !== "function" &&
    typeof window.setCpm === "function"
  ) {
    window.setcpm = (...args) => window.setCpm(...args);
  }
}

function normalizeLegacyScript(code) {
  return String(code ?? "");
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

function resolveSamplesFunction() {
  return (
    (runtime.api &&
      typeof runtime.api.samples === "function" &&
      runtime.api.samples.bind(runtime.api)) ||
    (typeof window.samples === "function" ? window.samples.bind(window) : null)
  );
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
  // Use Function() instead of eval() to avoid local scope leakage.
  return new Function(source)();
}

function resolveAudioClockSource() {
  const contexts = collectKnownAudioContexts();
  return contexts.find(
    (context) =>
      context &&
      typeof context.currentTime === "number" &&
      Number.isFinite(context.currentTime),
  );
}

export function getRuntimeAudioTime() {
  const context = resolveAudioClockSource();
  return context ? context.currentTime : null;
}

export async function compileMiniProbe(source, semantics = "mini") {
  const text = String(source ?? "").trim();
  if (!text) {
    return null;
  }

  await ensureRuntimeReady();
  await resumeKnownAudioContexts();

  const mode =
    semantics === "rhythm" || semantics === "mini" ? semantics : "mini";
  const body = `(() => {
    const source = ${JSON.stringify(text)};
    const semantics = ${JSON.stringify(mode)};
    const miniFn =
      typeof mini === "function"
        ? mini
        : typeof window.mini === "function"
          ? window.mini
          : null;
    if (typeof miniFn !== "function") {
      throw new Error("mini() API unavailable");
    }
    if (semantics !== "mini" && semantics !== "rhythm") {
      throw new Error("unsupported mini probe semantics");
    }
    const probe = miniFn(source);
    if (!probe || typeof probe.queryArc !== "function") {
      throw new Error("compiled mini probe missing queryArc()");
    }
    return probe;
  })()`;

  const evaluator = runtime.evaluate;
  if (typeof evaluator === "function") {
    const result = evaluator(body);
    return result && typeof result.then === "function" ? await result : result;
  }

  return new Function(`return ${body}`)();
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

  const samplesFn = resolveSamplesFunction();
  if (samplesFn) {
    const loads = DEFAULT_SAMPLE_LIBRARIES.map((library) => samplesFn(library));
    const results = await Promise.allSettled(loads);
    const failures = [];
    let loaded = 0;
    results.forEach((result, index) => {
      if (result.status === "fulfilled") {
        loaded += 1;
      } else {
        const reason =
          result.reason instanceof Error
            ? result.reason.message
            : String(result.reason ?? "unknown");
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
  installSampleFetchCache();

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

export function runtimeSampleCacheStatus() {
  return { ...sampleFetchCache };
}

export function runtimeInitSampleStatus() {
  return [...initSampleLoads.values()].map((entry) => ({
    id: entry.id,
    source: entry.source,
    aliases: { ...(entry.aliases ?? {}) },
    status: entry.status,
    error: entry.error ?? null,
  }));
}

function setInitSampleStatus(id, next) {
  initSampleLoads.set(id, {
    id,
    source: next.source,
    aliases: { ...(next.aliases ?? {}) },
    status: next.status,
    error: next.error ?? null,
  });
}

export async function runCadenceProgram(program) {
  await ensureRuntimeReady();
  await resumeKnownAudioContexts();

  const cpsExpr =
    typeof program?.cpsExpr === "string" ? program.cpsExpr.trim() : "";
  const sampleLoads = Array.isArray(program?.sampleLoads)
    ? program.sampleLoads
    : [];
  const declarationCode = Array.isArray(program?.declarationCode)
    ? program.declarationCode
    : [];
  const runtimeCode =
    typeof program?.runtimeCode === "string" ? program.runtimeCode.trim() : "";

  if (cpsExpr) {
    await executeScript(`setCps(${cpsExpr})`);
  }

  const samplesFn = resolveSamplesFunction();
  for (const load of sampleLoads) {
    if (!load || typeof load !== "object") {
      continue;
    }
    const id = String(load.id ?? "").trim();
    const source = String(load.source ?? "").trim();
    const aliases =
      load.aliases && typeof load.aliases === "object" ? load.aliases : {};
    if (!id || !source) {
      continue;
    }
    setInitSampleStatus(id, {
      source,
      aliases,
      status: "loading",
      error: null,
    });
    if (typeof samplesFn !== "function") {
      setInitSampleStatus(id, {
        source,
        aliases,
        status: "error",
        error: "samples() API unavailable",
      });
      throw new Error(
        `sample_load_failed: samples() API unavailable for ${id}`,
      );
    }
    try {
      const result = samplesFn(aliases, source);
      if (result && typeof result.then === "function") {
        await result;
      }
      setInitSampleStatus(id, {
        source,
        aliases,
        status: "ready",
        error: null,
      });
    } catch (error) {
      const detail =
        error instanceof Error ? error.message : String(error ?? "unknown");
      setInitSampleStatus(id, {
        source,
        aliases,
        status: "error",
        error: detail,
      });
      throw new Error(`sample_load_failed[${id}]: ${detail}`);
    }
  }

  const runtimeSections = [];
  for (const declaration of declarationCode) {
    const source = normalizeLegacyScript(declaration).trim();
    if (source) {
      runtimeSections.push(source);
    }
  }
  if (runtimeCode) {
    runtimeSections.push(runtimeCode);
  }

  if (runtimeSections.length > 0) {
    await executeScript(runtimeSections.join("\n"));
  }
}

export async function stopProgram() {
  await forceStopRuntime();
}

installSampleFetchCache();
