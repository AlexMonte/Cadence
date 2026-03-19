import { compileMiniProbe, getRuntimeAudioTime } from "../bridge/strudel.js";

const FRAME_INTERVAL_MS = 1000 / 15;
const QUERY_EPSILON = 0.001;

function clampOffset(value, max) {
  if (!Number.isFinite(value)) {
    return 0;
  }
  return Math.max(0, Math.min(max, Math.floor(value)));
}

function normalizeLocationRange(location, sourceLength) {
  const start = clampOffset(location?.start?.offset ?? location?.start, sourceLength);
  const end = clampOffset(location?.end?.offset ?? location?.end, sourceLength);
  return end > start ? { from: start, to: end } : null;
}

function extractRangesFromHap(hap, sourceLength) {
  const locations = hap?.context?.locations;
  if (!Array.isArray(locations)) {
    return [];
  }
  return locations
    .map((location) => normalizeLocationRange(location, sourceLength))
    .filter(Boolean);
}

function compactPreviewText(source, ranges) {
  for (const range of ranges) {
    const text = source.slice(range.from, range.to).replace(/\s+/g, " ").trim();
    if (text) {
      return text.length > 14 ? `${text.slice(0, 13)}…` : text;
    }
  }
  return "";
}

function normalizeQueryResult(result) {
  if (Array.isArray(result)) {
    return result;
  }
  if (result && typeof result[Symbol.iterator] === "function") {
    return [...result];
  }
  return [];
}

export function createMiniProbeRegistry({ onFrame }) {
  let destroyed = false;
  let generation = 0;
  let rafId = 0;
  let lastFrameAt = 0;
  let playing = false;
  let selectedSourceKey = null;
  let sourceMap = new Map();

  function stopLoop() {
    if (rafId) {
      cancelAnimationFrame(rafId);
      rafId = 0;
    }
  }

  function emitFrame(frame) {
    if (typeof onFrame === "function") {
      onFrame(frame);
    }
  }

  function shouldRun() {
    return playing && sourceMap.size > 0 && !destroyed;
  }

  function schedule() {
    if (!shouldRun() || rafId) {
      return;
    }
    rafId = requestAnimationFrame(tick);
  }

  async function compileEntry(entry, token) {
    if (!entry.text.trim()) {
      entry.probe = null;
      entry.error = null;
      return;
    }
    try {
      const probe = await compileMiniProbe(entry.text, entry.semantics);
      if (destroyed || token !== generation) {
        return;
      }
      entry.probe = probe;
      entry.error = null;
      schedule();
    } catch (error) {
      if (destroyed || token !== generation) {
        return;
      }
      entry.probe = null;
      entry.error = error instanceof Error ? error.message : String(error ?? "unknown");
    }
  }

  function tick(time) {
    rafId = 0;
    if (!shouldRun()) {
      emitFrame({ tileStates: new Map(), selectedRanges: [] });
      return;
    }
    if (time - lastFrameAt < FRAME_INTERVAL_MS) {
      schedule();
      return;
    }
    lastFrameAt = time;

    const now = getRuntimeAudioTime();
    if (!Number.isFinite(now)) {
      schedule();
      return;
    }

    const tileStates = new Map();
    let selectedRanges = [];

    for (const [key, entry] of sourceMap.entries()) {
      if (!entry.probe || typeof entry.probe.queryArc !== "function") {
        tileStates.set(key, { active: false, preview: "", ranges: [] });
        continue;
      }

      let ranges = [];
      try {
        const haps = normalizeQueryResult(entry.probe.queryArc(now, now + QUERY_EPSILON));
        for (const hap of haps) {
          ranges.push(...extractRangesFromHap(hap, entry.text.length));
        }
      } catch (error) {
        entry.error = error instanceof Error ? error.message : String(error ?? "unknown");
        ranges = [];
      }

      const preview = compactPreviewText(entry.text, ranges);
      tileStates.set(key, {
        active: ranges.length > 0,
        preview,
        ranges,
      });

      if (key === selectedSourceKey) {
        selectedRanges = ranges;
      }
    }

    emitFrame({ tileStates, selectedRanges });
    schedule();
  }

  return {
    setSources(sources) {
      generation += 1;
      const token = generation;
      const nextMap = new Map();
      for (const source of Array.isArray(sources) ? sources : []) {
        const key = String(source?.key ?? "");
        if (!key) {
          continue;
        }
        const entry = {
          ...source,
          key,
          text: String(source?.text ?? ""),
          semantics: String(source?.semantics ?? "mini"),
          mode: String(source?.mode ?? "mini"),
          probe: null,
          error: null,
        };
        nextMap.set(key, entry);
        if (playing) {
          void compileEntry(entry, token);
        }
      }
      sourceMap = nextMap;
      emitFrame({ tileStates: new Map(), selectedRanges: [] });
      if (!shouldRun()) {
        stopLoop();
      } else {
        schedule();
      }
    },
    setSelectedSourceKey(nextKey) {
      selectedSourceKey = nextKey ? String(nextKey) : null;
    },
    setPlaying(nextPlaying) {
      playing = !!nextPlaying;
      if (!playing) {
        stopLoop();
        emitFrame({ tileStates: new Map(), selectedRanges: [] });
        return;
      }
      const token = generation;
      for (const entry of sourceMap.values()) {
        if (!entry.probe) {
          void compileEntry(entry, token);
        }
      }
      schedule();
    },
    destroy() {
      destroyed = true;
      sourceMap = new Map();
      stopLoop();
    },
  };
}
