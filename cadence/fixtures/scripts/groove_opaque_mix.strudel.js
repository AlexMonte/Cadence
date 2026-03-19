const maybe = window.__dyn?.voiceFactory?.(Date.now())
if (Math.random() > 0.5) {
  globalThis.__last = maybe
}
const beat = n("0 1").s("cp").struct("<x _>")
stack(beat, maybe)
