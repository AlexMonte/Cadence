//! Cadence-owned Terrance language pipeline.
//!
//! Parsing remains syntax-first, then moves through a typed semantic layer
//! before lowering into host IR. This keeps musical meaning in Cadence instead
//! of baking it into parser-side ad hoc rewrites.

mod lowering;
mod semantic;
mod surface;

use crate::{
    application::authoring::tile_pattern,
    domain::{common::GridPos, program::CadencePatternExpr},
};

pub(crate) use semantic::{PitchResolveMode, ScaleSpec};

pub(crate) use lowering::apply_scale_to_pattern_expr;
use lowering::{lower_pattern_values, transpose_pitch_pattern};
use semantic::{resolve_pattern_values, resolve_pitch_pattern};
use surface::SurfacePattern;

use super::tile_pattern::TileScriptError;

fn parse_surface(
    text: &str,
    parse: impl FnOnce(&str) -> Result<tile_pattern::TileScript, TileScriptError>,
) -> Result<SurfacePattern, TileScriptError> {
    Ok(SurfacePattern::from(parse(text)?))
}

pub(crate) fn lower_control_text(
    text: &str,
    site: Option<GridPos>,
) -> Result<CadencePatternExpr, TileScriptError> {
    lower_pattern_like_text(text, site, tile_pattern::parse_control_script)
}

fn lower_pattern_like_text(
    text: &str,
    site: Option<GridPos>,
    parse: impl FnOnce(&str) -> Result<tile_pattern::TileScript, TileScriptError>,
) -> Result<CadencePatternExpr, TileScriptError> {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return Ok(CadencePatternExpr::Silence);
    }

    let surface = parse_surface(trimmed, parse)?;
    let semantic = resolve_pattern_values(&surface, semantic::StreamKind::Control)?;
    Ok(lower_pattern_values(&semantic, site))
}

pub(crate) fn lower_add_text(
    text: &str,
    input: CadencePatternExpr,
) -> Result<CadencePatternExpr, TileScriptError> {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return Ok(input);
    }

    let surface = parse_surface(trimmed, tile_pattern::parse_note_script)?;
    let semantic = resolve_pitch_pattern(&surface, PitchResolveMode::Offset)?;
    Ok(transpose_pitch_pattern(input, &semantic))
}
