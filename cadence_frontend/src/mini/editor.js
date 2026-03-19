import { Compartment, EditorState, StateEffect, StateField } from "@codemirror/state";
import { Decoration, EditorView } from "@codemirror/view";

const setHighlightsEffect = StateEffect.define();

const highlightMark = Decoration.mark({ class: "cm-cadence-active-token" });

const highlightField = StateField.define({
  create() {
    return Decoration.none;
  },
  update(decorations, transaction) {
    for (const effect of transaction.effects) {
      if (effect.is(setHighlightsEffect)) {
        return effect.value;
      }
    }
    return decorations.map(transaction.changes);
  },
  provide: (field) => EditorView.decorations.from(field),
});

function buildHighlightDecorations(doc, ranges) {
  if (!Array.isArray(ranges) || ranges.length === 0) {
    return Decoration.none;
  }

  const marks = [];
  for (const range of ranges) {
    const from = Number(range?.from ?? 0);
    const to = Number(range?.to ?? 0);
    if (!Number.isFinite(from) || !Number.isFinite(to)) {
      continue;
    }
    const start = Math.max(0, Math.min(doc.length, Math.floor(from)));
    const end = Math.max(start, Math.min(doc.length, Math.floor(to)));
    if (end > start) {
      marks.push(highlightMark.range(start, end));
    }
  }
  return marks.length > 0 ? Decoration.set(marks, true) : Decoration.none;
}

function buildState({ doc, readOnly, editable, onChange, theme, compartments }) {
  return EditorState.create({
    doc,
    extensions: [
      highlightField,
      compartments.editable.of(EditorView.editable.of(editable)),
      compartments.readOnly.of(EditorState.readOnly.of(readOnly)),
      EditorView.lineWrapping,
      theme,
      EditorView.updateListener.of((update) => {
        if (update.docChanged) {
          onChange(update.state.doc.toString());
        }
      }),
    ],
  });
}

export function createMiniEditor({ host, onChange }) {
  const compartments = {
    editable: new Compartment(),
    readOnly: new Compartment(),
  };
  const theme = EditorView.theme({
    "&": {
      height: "100%",
      minHeight: "220px",
      backgroundColor: "#0d1018",
      color: "#f4f6fb",
      fontFamily: "var(--mono)",
      fontSize: "12px",
      border: "2px solid #212734",
    },
    ".cm-scroller": {
      overflow: "auto",
      lineHeight: "1.6",
    },
    ".cm-content": {
      padding: "12px 14px 32px",
      caretColor: "#f7f8fb",
    },
    ".cm-focused": {
      outline: "none",
    },
    ".cm-cursor, .cm-dropCursor": {
      borderLeftColor: "#f7f8fb",
    },
    ".cm-selectionBackground": {
      backgroundColor: "rgba(120, 192, 255, 0.28)",
    },
    ".cm-cadence-active-token": {
      backgroundColor: "rgba(255, 212, 84, 0.35)",
      boxShadow: "inset 0 -2px 0 rgba(255, 212, 84, 0.9)",
      borderRadius: "2px",
    },
  });

  let suppressChanges = false;
  let currentDoc = "";
  let view = new EditorView({
    state: buildState({
      doc: "",
      readOnly: false,
      editable: true,
      theme,
      compartments,
      onChange(nextDoc) {
        currentDoc = nextDoc;
        if (!suppressChanges && typeof onChange === "function") {
          onChange(nextDoc);
        }
      },
    }),
    parent: host,
  });

  function setSession({ doc = "", readOnly = false } = {}) {
    const text = String(doc ?? "");
    currentDoc = text;
    suppressChanges = true;
    view.setState(
      buildState({
        doc: text,
        readOnly,
        editable: !readOnly,
        theme,
        compartments,
        onChange(nextDoc) {
          currentDoc = nextDoc;
          if (!suppressChanges && typeof onChange === "function") {
            onChange(nextDoc);
          }
        },
      }),
    );
    suppressChanges = false;
  }

  return {
    setSession,
    setHighlights(ranges) {
      view.dispatch({
        effects: setHighlightsEffect.of(
          buildHighlightDecorations(view.state.doc, ranges),
        ),
      });
    },
    focus() {
      view.focus();
    },
    getDoc() {
      return currentDoc;
    },
    destroy() {
      view.destroy();
      host.replaceChildren();
      view = null;
    },
  };
}
