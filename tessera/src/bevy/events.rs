use bevy_ecs::message::Message;
use bevy_reflect::Reflect;

use crate::domain::Diagnostic;
use crate::infrastructure::{CompileOptions, CompileReport, ValidationReport};

#[derive(Debug, Clone, Message, Reflect, Default)]
pub struct CompileRequested {
    pub force: bool,
}

impl CompileRequested {
    pub fn forced() -> Self {
        Self { force: true }
    }
}

#[derive(Debug, Clone, Message)]
pub enum CompileFinished {
    Ok(crate::domain::PatternIr),
    Err(Vec<Diagnostic>),
}

#[derive(Debug, Clone, Message)]
pub enum ValidateFinished {
    Ok(ValidationReport),
    Err(Vec<Diagnostic>),
}

#[derive(Debug, Clone, Reflect)]
pub struct TesseraCompileReport {
    pub normalized_outputs: usize,
    pub ir_outputs: usize,
}

impl From<CompileReport> for TesseraCompileReport {
    fn from(report: CompileReport) -> Self {
        Self {
            normalized_outputs: report.normalized.containers.len(),
            ir_outputs: report.ir.outputs.len(),
        }
    }
}

#[derive(Debug, Clone, Reflect, Default)]
pub struct TesseraCompilerSettingsReflect {
    pub validate_before_compile: bool,
}

impl From<CompileOptions> for TesseraCompilerSettingsReflect {
    fn from(value: CompileOptions) -> Self {
        Self {
            validate_before_compile: value.validate_before_compile,
        }
    }
}
