mod authored_program_ext;
pub mod board;
mod compiler;
mod placement;
mod ports;
mod reports;
mod sequence_stack;
mod spatial;
mod stack;

pub use authored_program_ext::AuthoredTesseraProgramExt;
pub use board::{
    Alternate, Board, BoardError, BuiltContainer, Flow, FlowCursor, Layer, Sequence, TileHandle,
    TileRef, TileSlot,
};
pub use compiler::TesseraCompiler;
pub use ports::*;
pub use reports::{CompileReport, PreviewReport, ValidationReport};
pub use sequence_stack::SequenceStack;
pub use spatial::{footprint, placement, placement_with_footprint, slot, unit_footprint};
pub use stack::{
    StackBuilder, accidental, modifier, nested, note, notes, octave, op, rest, scalar, stack,
};

pub mod mini_notation;
pub use mini_notation::{MiniPattern, MiniPatternKind, parse_mini_notation, parse_pattern_number};
