export function applyMiniTilePreview(tileElements, tileStates) {
  if (!(tileElements instanceof Map)) {
    return;
  }

  for (const [key, refs] of tileElements.entries()) {
    const tile = refs?.tile;
    const preview = refs?.preview;
    if (!(tile instanceof HTMLElement) || !(preview instanceof HTMLElement)) {
      continue;
    }

    const frame = tileStates instanceof Map ? tileStates.get(key) : null;
    const active = !!frame?.active;
    const text = String(frame?.preview ?? "");

    tile.classList.toggle("is-mini-active", active);
    tile.dataset.miniPreview = text;
    preview.textContent = text;
    preview.classList.toggle("is-visible", active && text.length > 0);
  }
}
