//! Exact sustain boundaries use the same metadata command and undo as tuning.
use super::*;
use crate::domain::project::samples::SampleLoopRegion;
use tessera::prelude::Rational;

#[derive(Component)]
struct ToggleLoop(NodeId);
#[derive(Component)]
struct LoopBoundary {
    output: NodeId,
    start: bool,
}

pub(super) fn spawn(
    parent: &mut ChildSpawnerCommands<'_>,
    output: &NodeId,
    sample: &SampleOptionsPaint,
) {
    parent
        .spawn(musaic_button(
            Node {
                min_height: px(26),
                padding: UiRect::all(px(5)),
                ..default()
            },
            ToggleLoop(output.clone()),
            if sample.options.sustain_loop.is_some() {
                "Sustain loop: on"
            } else {
                "Sustain loop: off"
            },
        ))
        .observe(toggle);
    if let Some(region) = sample.options.sustain_loop {
        label(
            parent,
            "Attack plays once, then the loop repeats through the note and release. Frames count from zero; end is exclusive.",
        );
        for (start, value) in [(true, region.start_frame), (false, region.end_frame)] {
            parent
                .spawn(Node {
                    width: percent(100),
                    flex_direction: FlexDirection::Column,
                    row_gap: px(3),
                    ..default()
                })
                .with_children(|row| {
                    label(
                        row,
                        if start {
                            "Loop start frame"
                        } else {
                            "Loop end frame"
                        },
                    );
                    let field = crate::infrastructure::ui::widgets::exact_number::spawn(
                        row,
                        Rational::from_integer(value as i64),
                        LoopBoundary {
                            output: output.clone(),
                            start,
                        },
                    );
                    row.commands()
                        .entity(field)
                        .insert(Node {
                            width: percent(100),
                            min_height: px(28),
                            flex_shrink: 0.,
                            padding: UiRect::axes(px(7), px(4)),
                            border: UiRect::bottom(px(1)),
                            ..default()
                        })
                        .observe(change_boundary);
                });
        }
        label(parent, &format!("Recording: {} frames", sample.frame_count));
    }
}

fn toggle(
    event: On<Activate>,
    buttons: Query<&ToggleLoop>,
    projection: Res<EditorUiProjection>,
    mut bus: MessageWriter<EditorCommandBus>,
) {
    let Ok(button) = buttons.get(event.entity) else {
        return;
    };
    let Some(sample) = projected(&projection, &button.0) else {
        return;
    };
    let mut options = sample.options.clone();
    options.sustain_loop = if options.sustain_loop.is_some() {
        None
    } else {
        Some(SampleLoopRegion {
            start_frame: 0,
            end_frame: sample.frame_count,
        })
    };
    bus.write(EditorCommandBus(EditorCommand::SetSampleOptions {
        sample: sample.sample,
        options,
    }));
}

fn boundary_options(
    sample: &SampleOptionsPaint,
    start: bool,
    value: Rational,
) -> Result<SampleImportOptions, String> {
    if value.denominator != 1 || value.numerator < 0 {
        return Err("Enter a nonnegative whole frame number".into());
    }
    let mut options = sample.options.clone();
    let region = options
        .sustain_loop
        .as_mut()
        .ok_or("Enable the sustain loop first")?;
    if start {
        region.start_frame = value.numerator as u64;
    } else {
        region.end_frame = value.numerator as u64;
    }
    options.validate_frame_count(sample.frame_count)?;
    Ok(options)
}

fn change_boundary(
    event: On<ValueChange<Rational>>,
    editors: Query<&LoopBoundary>,
    projection: Res<EditorUiProjection>,
    mut texts: Query<&mut Text>,
    mut focus: ResMut<bevy::input_focus::InputFocus>,
    mut bus: MessageWriter<EditorCommandBus>,
) {
    let Ok(editor) = editors.get(event.source) else {
        return;
    };
    let Some(sample) = projected(&projection, &editor.output) else {
        return;
    };
    match boundary_options(sample, editor.start, event.value) {
        Ok(options) => {
            bus.write(EditorCommandBus(EditorCommand::SetSampleOptions {
                sample: sample.sample,
                options,
            }));
        }
        Err(message) => {
            if let Ok(mut text) = texts.get_mut(event.source) {
                crate::infrastructure::ui::widgets::exact_number::reject(
                    &mut text,
                    &mut focus,
                    event.source,
                    &message,
                );
            }
        }
    }
}
