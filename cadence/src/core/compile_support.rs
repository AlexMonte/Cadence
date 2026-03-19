//! Shared helpers for compile previews, metadata, and diagnostic aggregation.

use std::collections::BTreeSet;

use tessera::diagnostics::{Diagnostic, DiagnosticSeverity};

pub(crate) fn merge_diagnostics<I>(groups: I) -> Vec<Diagnostic>
where
    I: IntoIterator<Item = Vec<Diagnostic>>,
{
    let mut seen = BTreeSet::new();
    let mut merged = Vec::new();

    for group in groups {
        for diagnostic in group {
            if seen.insert(diagnostic_key(&diagnostic)) {
                merged.push(diagnostic);
            }
        }
    }

    merged
}

pub(crate) fn has_error_diagnostics(diagnostics: &[Diagnostic]) -> bool {
    diagnostics
        .iter()
        .any(|diag| matches!(diag.severity, DiagnosticSeverity::Error))
}

fn diagnostic_key(diagnostic: &Diagnostic) -> String {
    let kind = serde_json::to_string(&diagnostic.kind)
        .unwrap_or_else(|_| format!("{:?}", diagnostic.kind));
    format!(
        "{:?}|{}|{:?}|{:?}",
        diagnostic.severity, kind, diagnostic.site, diagnostic.edge_id
    )
}
