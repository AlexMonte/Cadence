//! One decoded-file worker shared by import and relink. Only the dispatcher
//! publishes its results; project and per-asset tokens reject stale work.
use super::*;
use crate::{
    adapter::persistence::{self, PreparedSampleImport},
    domain::project::samples::{SampleId, SampleImportOptions},
};
use std::{
    path::PathBuf,
    sync::{Mutex, mpsc},
};

#[derive(Resource, Default)]
pub(super) struct ActiveSampleOptionsEdit {
    sample: Option<SampleId>,
    recorded: bool,
}

#[derive(Clone)]
enum Target {
    Import {
        sound: Option<tessera::prelude::NodeId>,
    },
    Relink {
        sample: SampleId,
        revision: u64,
        path: PathBuf,
    },
}
struct Completion {
    generation: u64,
    target: Target,
    result: Result<PreparedSampleImport, String>,
}

#[derive(Resource)]
pub(super) struct SampleImports {
    pub(super) generation: u64,
    busy: bool,
    #[cfg(not(target_arch = "wasm32"))]
    sender: mpsc::Sender<Completion>,
    receiver: Mutex<mpsc::Receiver<Completion>>,
}
impl Default for SampleImports {
    fn default() -> Self {
        let (sender, receiver) = mpsc::channel();
        #[cfg(target_arch = "wasm32")]
        let _ = sender;
        Self {
            generation: 0,
            busy: false,
            #[cfg(not(target_arch = "wasm32"))]
            sender,
            receiver: Mutex::new(receiver),
        }
    }
}

impl SampleImports {
    pub(super) fn replace_project(&mut self) {
        self.generation = self.generation.wrapping_add(1);
        // An obsolete worker may finish in the background; it must neither
        // block the new project nor clear the new project's busy flag.
        self.busy = false;
    }
}

pub(super) fn drain(ctx: &mut CommandContext) {
    let completed: Vec<_> = ctx
        .imports
        .receiver
        .lock()
        .expect("sample completion lock")
        .try_iter()
        .collect();
    for completed in completed {
        if completed.generation != ctx.imports.generation {
            continue;
        }
        ctx.imports.busy = false;
        if let Target::Relink {
            sample, revision, ..
        } = &completed.target
        {
            if !ctx.project.samples.manifest().samples.contains_key(sample)
                || ctx.project.samples.asset_revision(*sample) != *revision
            {
                continue;
            }
        }
        let result = completed
            .result
            .and_then(|prepared| match completed.target {
                Target::Import { sound } => {
                    let id = persistence::adopt_sample_import(&mut ctx.project, prepared)
                        .map_err(|error| error.to_string())?;
                    refresh_sound(ctx);
                    if let Some(sound) = sound {
                        set_sound(
                            ctx,
                            &sound,
                            crate::domain::instrument::InstrumentDefinition::new(
                                crate::domain::instrument::InstrumentSource::Sample(id),
                            ),
                            true,
                        );
                    }
                    Ok(())
                }
                Target::Relink { sample, path, .. } => {
                    let PreparedSampleImport::Recording(prepared) = prepared else {
                        return Err("Relink requires one WAV recording".into());
                    };
                    let before =
                        persistence::relink_prepared_sample(&mut ctx.project, sample, prepared)
                            .map_err(|error| error.to_string())?;
                    let after = persistence::snapshot_sample(&ctx.project, sample)
                        .map_err(|error| error.to_string())?;
                    refresh_sound(ctx);
                    ctx.record_mutation(HistoryEntry {
                        forward: EditorCommand::RelinkSample { sample, path },
                        inverse: super::super::EditorInverse::RestoreSampleAsset {
                            sample,
                            before: Some(before),
                            after: Some(after),
                        },
                        invalidation: Invalidation::document_replaced(),
                    });
                    Ok(())
                }
            });
        if let Err(error) = result {
            report(ctx, format!("Could not load sound: {error}"));
        }
    }
}

pub(super) fn handle(command: &EditorCommand, ctx: &mut CommandContext) -> bool {
    match command {
        EditorCommand::SetSampleBank { sample, definition } => {
            set_bank(ctx, *sample, definition.clone(), true)
        }
        EditorCommand::BeginSampleOptionsEdit { sample } => {
            *ctx.sample_options_edit = ActiveSampleOptionsEdit {
                sample: Some(*sample),
                recorded: false,
            }
        }
        EditorCommand::EndSampleOptionsEdit => {
            *ctx.sample_options_edit = ActiveSampleOptionsEdit::default()
        }
        EditorCommand::SetSampleOptions { sample, options } => {
            set_options(ctx, *sample, options.clone(), true)
        }
        EditorCommand::ImportSample { path, sound } => start(
            ctx,
            path.clone(),
            Target::Import {
                sound: sound.clone(),
            },
        ),
        EditorCommand::RelinkSample { sample, path } => {
            if !ctx.project.samples.manifest().samples.contains_key(sample) {
                report(ctx, "That sample is not in this project".into());
            } else {
                start(
                    ctx,
                    path.clone(),
                    Target::Relink {
                        sample: *sample,
                        revision: ctx.project.samples.asset_revision(*sample),
                        path: path.clone(),
                    },
                );
            }
        }
        _ => return false,
    }
    true
}

fn set_bank(
    ctx: &mut CommandContext,
    sample: SampleId,
    definition: Option<crate::domain::project::samples::SampleBankDefinition>,
    record: bool,
) {
    let previous = match persistence::set_sample_bank(&mut ctx.project, sample, definition.clone())
    {
        Ok(Some(previous)) => previous,
        Ok(None) => return,
        Err(error) => {
            report(ctx, error.to_string());
            return;
        }
    };
    refresh_sound(ctx);
    if record {
        ctx.record_mutation(HistoryEntry {
            forward: EditorCommand::SetSampleBank { sample, definition },
            inverse: super::super::EditorInverse::RestoreSampleBank {
                sample,
                definition: previous,
            },
            invalidation: Invalidation::document_replaced(),
        });
    }
}

#[cfg(test)]
#[path = "sample_worker_tests.rs"]
mod tests;

fn start(ctx: &mut CommandContext, path: PathBuf, target: Target) {
    if ctx.imports.busy {
        report(ctx, "A sound is already being loaded".into());
        return;
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        let generation = ctx.imports.generation;
        let sender = ctx.imports.sender.clone();
        match std::thread::Builder::new()
            .name("musaic-sample-decode".into())
            .spawn(move || {
                let result = match &target {
                    Target::Import { .. } => persistence::prepare_sample_import(path),
                    Target::Relink { .. } => {
                        persistence::prepare_wav_sample(path, Default::default()).map(Into::into)
                    }
                }
                .map_err(|error| error.to_string());
                let _ = sender.send(Completion {
                    generation,
                    target,
                    result,
                });
            }) {
            Ok(_) => ctx.imports.busy = true,
            Err(error) => report(ctx, format!("Could not start sound loading: {error}")),
        }
    }
    #[cfg(target_arch = "wasm32")]
    {
        let _ = (path, target);
        report(ctx, "WAV loading currently requires the desktop app".into());
    }
}

fn set_options(
    ctx: &mut CommandContext,
    sample: SampleId,
    options: SampleImportOptions,
    record: bool,
) {
    let previous = match persistence::set_sample_options(&mut ctx.project, sample, options.clone())
    {
        Ok(Some(previous)) => previous,
        Ok(None) => return,
        Err(error) => {
            report(ctx, error.to_string());
            return;
        }
    };
    refresh_sound(ctx);
    if !record {
        return;
    }
    let entry = HistoryEntry {
        forward: EditorCommand::SetSampleOptions { sample, options },
        inverse: super::super::EditorInverse::RestoreSampleOptions {
            sample,
            options: previous,
        },
        invalidation: Invalidation::document_replaced(),
    };
    let gesture = ctx.sample_options_edit.sample == Some(sample);
    if gesture && ctx.sample_options_edit.recorded {
        ctx.history.continue_sample_options_edit(entry);
    } else {
        ctx.record_mutation(entry);
    }
    if gesture {
        ctx.sample_options_edit.recorded = true;
    }
}

/// Reload metadata-backed voices even while authoring has an unfinished edit.
fn refresh_sound(ctx: &mut CommandContext) {
    ctx.preview_epoch.0 = ctx.preview_epoch.0.wrapping_add(1);
    ctx.runtime.mark_sound_changed();
    if ctx.runtime.pending_ir.is_none() {
        ctx.runtime.mark_full_rebuild();
    }
}
fn report(ctx: &mut CommandContext, message: String) {
    push_execution_error(
        &mut ctx.diagnostics,
        crate::domain::DomainError::Message(message),
    );
}

pub(super) fn restore(
    entry: &HistoryEntry,
    direction: &HistoryDirection,
    ctx: &mut CommandContext,
) -> bool {
    match &entry.inverse {
        super::super::EditorInverse::RestoreSampleBank { sample, definition } => {
            let definition = match direction {
                HistoryDirection::Undo => definition.clone(),
                HistoryDirection::Redo => match &entry.forward {
                    EditorCommand::SetSampleBank { definition, .. } => definition.clone(),
                    _ => return true,
                },
            };
            set_bank(ctx, *sample, definition, false);
        }
        super::super::EditorInverse::RestoreSampleOptions { sample, options } => {
            let options = match direction {
                HistoryDirection::Undo => options.clone(),
                HistoryDirection::Redo => match &entry.forward {
                    EditorCommand::SetSampleOptions { options, .. } => options.clone(),
                    _ => return true,
                },
            };
            set_options(ctx, *sample, options, false);
        }
        super::super::EditorInverse::RestoreSampleAsset {
            sample,
            before,
            after,
        } => {
            let snapshot = match direction {
                HistoryDirection::Undo => before,
                HistoryDirection::Redo => after,
            };
            let result = snapshot
                .as_ref()
                .ok_or_else(|| "This undo entry has no decoded sound checkpoint".into())
                .and_then(|snapshot| {
                    persistence::restore_sample_snapshot(&mut ctx.project, *sample, snapshot)
                        .map_err(|error| error.to_string())
                });
            match result {
                Ok(()) => refresh_sound(ctx),
                Err(error) => report(ctx, error),
            }
        }
        _ => return false,
    }
    true
}
