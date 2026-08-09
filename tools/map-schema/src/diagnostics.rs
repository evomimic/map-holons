//! Diagnostic formatting for active map-schema source tooling.

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diagnostic {
    pub message: String,
}

pub fn format_diagnostics(diagnostics: &[Diagnostic]) -> String {
    diagnostics.iter().map(|diagnostic| diagnostic.message.as_str()).collect::<Vec<_>>().join("\n")
}
