use super::*;

pub(super) fn handle(command: &EditorCommand, ctx: &mut CommandContext) -> bool {
    let EditorCommand::SoundLibrary(command) = command else {
        return false;
    };
    run(ctx, command, true);
    true
}
fn run(
    ctx: &mut CommandContext,
    command: &super::super::sound_library::SoundLibraryCommand,
    record: bool,
) {
    match super::super::sound_library::apply(&mut ctx.project, command) {
        Ok(Some(library)) if record => ctx.record_mutation(HistoryEntry {
            forward: EditorCommand::SoundLibrary(command.clone()),
            inverse: super::super::EditorInverse::RestoreSoundLibrary { library },
            invalidation: Invalidation::document_replaced(),
        }),
        Ok(_) => {}
        Err(error) => push_execution_error(
            &mut ctx.diagnostics,
            crate::domain::DomainError::Message(error),
        ),
    }
}
pub(super) fn restore(
    entry: &HistoryEntry,
    direction: &HistoryDirection,
    ctx: &mut CommandContext,
) -> bool {
    let super::super::EditorInverse::RestoreSoundLibrary { library } = &entry.inverse else {
        return false;
    };
    match direction {
        HistoryDirection::Undo => {
            ctx.project.sound_library = library.clone();
            ctx.project.mark_dirty();
        }
        HistoryDirection::Redo => {
            if let EditorCommand::SoundLibrary(command) = &entry.forward {
                run(ctx, command, false);
            }
        }
    }
    true
}
