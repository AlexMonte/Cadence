use crate::domain::{Diagnostic, NormalizedProgram, PatternIr, PatternStream};

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "bevy", derive(bevy_reflect::Reflect))]
pub struct CompileReport {
    pub normalized: NormalizedProgram,
    #[cfg_attr(feature = "bevy", reflect(ignore))]
    pub ir: PatternIr,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "bevy", derive(bevy_reflect::Reflect))]
pub struct PreviewReport {
    pub stream: PatternStream,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "bevy", derive(bevy_reflect::Reflect))]
pub struct ValidationReport {
    pub valid: bool,
    pub diagnostics: Vec<Diagnostic>,
    pub normalized: Option<NormalizedProgram>,
}

impl ValidationReport {
    pub fn valid(normalized: NormalizedProgram) -> Self {
        Self {
            valid: true,
            diagnostics: Vec::new(),
            normalized: Some(normalized),
        }
    }

    pub fn invalid(diagnostics: Vec<Diagnostic>) -> Self {
        Self {
            valid: false,
            diagnostics,
            normalized: None,
        }
    }

    pub fn is_valid(&self) -> bool {
        self.valid
    }

    pub fn is_invalid(&self) -> bool {
        !self.valid
    }
}
