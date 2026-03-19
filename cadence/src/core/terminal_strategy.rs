use tessera::{Backend, Expr, JsBackend};

/// Open strategy interface that controls how compiled terminal expressions
/// are rendered into a final executable program string.
///
/// The generic tile library produces one `Expr` per terminal node.
/// The host language runtime (Strudel, PSi, MIDI, etc.) decides what to
/// do with them by implementing this trait.
///
/// Built-in implementations:
/// - [`StackRenderer`]  — Strudel: joins terminals with `stack(a, b, ...)`
/// - [`DollarRenderer`] — Strudel: emits one `$: expr` line per terminal
/// - [`SingleOutputRenderer`] — PSi-style: only the first terminal is used
pub trait TerminalRenderer: Send + Sync {
    /// Render all terminal expressions into the final program string that
    /// the host runtime evaluates.
    fn render(&self, terminals: &[Expr]) -> String;
}

// ─── Built-in Strudel renderers ───────────────────────────────────────────────

/// Wraps multiple terminals inside a single Strudel `stack(a, b, c)` call.
/// With a single terminal, emits the expression directly (no wrapper).
pub struct StackRenderer;

impl TerminalRenderer for StackRenderer {
    fn render(&self, terminals: &[Expr]) -> String {
        match terminals {
            [] => String::new(),
            [single] => JsBackend.render(single),
            _ => JsBackend.render(&Expr::call_named("stack", terminals.to_vec())),
        }
    }
}

/// Emits one `$: expr` line per terminal — independent Strudel mini-notation
/// voices that run concurrently. This is the preferred strategy for multi-voice
/// compositions where each terminal represents a different instrument/track.
pub struct DollarRenderer;

impl TerminalRenderer for DollarRenderer {
    fn render(&self, terminals: &[Expr]) -> String {
        terminals
            .iter()
            .map(|expr| format!("$: {}", JsBackend.render(expr)))
            .collect::<Vec<_>>()
            .join("\n")
    }
}

// ─── Generic renderers ────────────────────────────────────────────────────────

/// Uses only the first terminal and discards the rest.
/// Suitable for single-output systems (PSi-style) where only one root
/// expression is meaningful.
// Intentionally kept for embedders; not used in the Strudel binary target.
#[allow(dead_code)]
pub struct SingleOutputRenderer;

impl TerminalRenderer for SingleOutputRenderer {
    fn render(&self, terminals: &[Expr]) -> String {
        terminals
            .first()
            .map(|t| JsBackend.render(t))
            .unwrap_or_default()
    }
}
