//! Shared helpers for compile previews, metadata, and diagnostic aggregation.

use tessera::diagnostics::{Diagnostic, DiagnosticSeverity};

pub(crate) fn has_error_diagnostics(diagnostics: &[Diagnostic]) -> bool {
    diagnostics
        .iter()
        .any(|diag| matches!(diag.severity, DiagnosticSeverity::Error))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tessera::diagnostics::DiagnosticKind;

    #[test]
    fn warning_and_info_diagnostics_do_not_block_playback() {
        let diagnostics = vec![
            Diagnostic {
                kind: DiagnosticKind::NoOutputNode,
                site: None,
                edge_id: None,
                severity: DiagnosticSeverity::Warning,
            },
            Diagnostic {
                kind: DiagnosticKind::NoOutputNode,
                site: None,
                edge_id: None,
                severity: DiagnosticSeverity::Info,
            },
        ];

        assert!(!has_error_diagnostics(&diagnostics));
    }
}
