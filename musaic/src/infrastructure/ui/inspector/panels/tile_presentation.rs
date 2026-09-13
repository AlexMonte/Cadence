//! Compact tile faces and owned groups from the approved paper/forest inspector.

use bevy::{prelude::*, ui::widget::Text as UiText};
use bevy_feathers::theme::{ThemeFontColor, ThemedText};
use tessera::prelude::{NodeId, ParameterKey};

use crate::application::{
    command::EditorCommand,
    editor::selection::SelectionMode,
    pipeline::{
        scene_sync::{
            TileSurfaceContent,
            surface_content::{OwnedCompoundView, OwnedTileGroup, OwnedTileGroupRole},
        },
        selected_tile::{SelectedTilePaint, container_glyph},
    },
};
use crate::domain::document::OperatorValue;
use crate::infrastructure::ui::{
    InspectorButtonAction, on_inspector_button_activated, theme::MusaicUiTheme,
    widgets::musaic_button,
};

const GREEN: Color = Color::srgb(0.157, 0.420, 0.310);
const GREEN_SOFT: Color = Color::srgb(0.886, 0.933, 0.898);
const GOLD: Color = Color::srgb(0.529, 0.388, 0.114);
const GOLD_SOFT: Color = Color::srgb(0.945, 0.922, 0.843);

#[derive(Clone, Copy)]
pub(super) enum GlyphPart {
    Selected,
    CompoundPitch,
    CompoundSummary,
    Owner,
    Operand,
}

#[derive(Component)]
pub(crate) struct InspectorTileGlyph {
    pub node: NodeId,
    pub(super) part: GlyphPart,
}

pub(crate) fn inspector_label(
    parent: &mut ChildSpawnerCommands<'_>,
    text: &str,
    size: f32,
    muted: bool,
) {
    let chrome = MusaicUiTheme::default().chrome;
    parent.spawn((
        UiText::new(text),
        TextFont {
            font_size: size,
            ..default()
        },
        TextColor(if muted {
            chrome.text_dim
        } else {
            chrome.text_main
        }),
        ThemeFontColor(if muted {
            bevy_feathers::tokens::TEXT_DIM
        } else {
            bevy_feathers::tokens::TEXT_MAIN
        }),
        ThemedText,
    ));
}

pub(super) fn spawn_selected_header(
    parent: &mut ChildSpawnerCommands<'_>,
    node: &NodeId,
    title: &str,
    description: &str,
    selected: &SelectedTilePaint,
    compound: Option<&OwnedCompoundView>,
) {
    let pitch = compound
        .filter(|compound| {
            compound
                .groups
                .iter()
                .any(|group| group.owner == *node && group.role == OwnedTileGroupRole::Pitch)
        })
        .map(compound_pitch);
    let glyph = pitch.as_deref().unwrap_or(&selected.glyph);
    let chrome = MusaicUiTheme::default().chrome;
    parent
        .spawn((
            Node {
                width: px(44),
                height: px(44),
                flex_shrink: 0.0,
                border: UiRect::all(px(1)),
                margin: UiRect::bottom(px(7)),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            BackgroundColor(if selected.pitched {
                GREEN_SOFT
            } else {
                Color::WHITE
            }),
            BorderColor::all(chrome.border),
        ))
        .with_children(|face| {
            face.spawn((
                InspectorTileGlyph {
                    node: node.clone(),
                    part: if pitch.is_some() {
                        GlyphPart::CompoundPitch
                    } else {
                        GlyphPart::Selected
                    },
                },
                UiText::new(glyph),
                TextFont {
                    font_size: if glyph.chars().count() > 3 {
                        15.0
                    } else {
                        23.0
                    },
                    ..default()
                },
                TextColor(if selected.pitched { GREEN } else { GOLD }),
            ));
        });
    if pitch.is_some() {
        parent.spawn((
            InspectorTileGlyph {
                node: node.clone(),
                part: GlyphPart::CompoundSummary,
            },
            UiText::new(compound.map(compound_summary).unwrap_or_default()),
            TextFont {
                font_size: 18.0,
                ..default()
            },
            TextColor(chrome.text_main),
        ));
    } else {
        inspector_label(parent, title, 18.0, false);
    }
    inspector_label(parent, description, 12.0, true);
}

pub(super) fn spawn_owned_groups(
    parent: &mut ChildSpawnerCommands<'_>,
    compound: &OwnedCompoundView,
) {
    detail_heading(
        parent,
        &format!(
            "Stack · {} tiles",
            compound
                .groups
                .iter()
                .map(|group| group.members.len())
                .sum::<usize>()
        ),
        "Owned groups",
    );
    let pitch_parts: Vec<_> = compound
        .groups
        .iter()
        .filter(|group| is_pitch_part(group.role))
        .collect();
    let has_pitch = pitch_parts
        .iter()
        .any(|group| group.role == OwnedTileGroupRole::Pitch);
    let mut pitch_shown = false;
    for (index, group) in compound.groups.iter().enumerate() {
        let pitched = is_pitch_part(group.role);
        if pitched && has_pitch {
            if pitch_shown {
                continue;
            }
            pitch_shown = true;
        }
        let color = if pitched { GREEN } else { GOLD };
        parent
            .spawn((
                Node {
                    width: percent(100),
                    padding: UiRect::all(px(10)),
                    border: UiRect::left(px(2)),
                    flex_direction: FlexDirection::Column,
                    row_gap: px(6),
                    flex_shrink: 0.0,
                    ..default()
                },
                BackgroundColor(MusaicUiTheme::default().chrome.panel_inset),
                BorderColor::all(color),
            ))
            .with_children(|group_panel| {
                inspector_label(
                    group_panel,
                    if pitched && has_pitch {
                        "Pitch"
                    } else {
                        group_role_label(group.role)
                    },
                    12.0,
                    true,
                );
                group_panel
                    .spawn(Node {
                        width: percent(100),
                        column_gap: px(3),
                        align_items: AlignItems::Center,
                        ..default()
                    })
                    .with_children(|row| {
                        if pitched && has_pitch {
                            // A single visual pitch card retains each constituent's
                            // document ID, so its face still opens its own editor.
                            for role in [
                                OwnedTileGroupRole::Pitch,
                                OwnedTileGroupRole::Accidental,
                                OwnedTileGroupRole::Octave,
                            ] {
                                for part in pitch_parts.iter().filter(|part| part.role == role) {
                                    group_cell(row, part, GlyphPart::Owner, true);
                                }
                            }
                        } else {
                            group_cell(row, group, GlyphPart::Owner, pitched);
                            if shows_operand(group) {
                                group_cell(row, group, GlyphPart::Operand, pitched);
                            }
                        }
                        row.spawn(Node {
                            flex_grow: 1.0,
                            ..default()
                        });
                        if index > 0
                            && !(pitched && has_pitch)
                            && group.complete
                            && matches!(
                                group.role,
                                OwnedTileGroupRole::Octave
                                    | OwnedTileGroupRole::Accidental
                                    | OwnedTileGroupRole::Modifier(_)
                                    | OwnedTileGroupRole::SoundModifier(_)
                                    | OwnedTileGroupRole::RhythmModifier
                            )
                        {
                            for (step, glyph, available) in [
                                (
                                    -1,
                                    "↑",
                                    index > 1 && !is_pitch_part(compound.groups[index - 1].role),
                                ),
                                (1, "↓", index + 1 < compound.groups.len()),
                            ] {
                                if available {
                                    row.spawn(musaic_button(
                                        cell_node(34.0),
                                        InspectorButtonAction(EditorCommand::MoveModifierGroup {
                                            owner: group.owner.clone(),
                                            step,
                                        }),
                                        glyph,
                                    ))
                                    .insert((
                                        BackgroundColor(Color::WHITE),
                                        BorderColor::all(MusaicUiTheme::default().chrome.border),
                                        TextFont {
                                            font_size: 14.0,
                                            ..default()
                                        },
                                    ))
                                    .observe(on_inspector_button_activated);
                                }
                            }
                        }
                    });
            });
    }
    inspector_label(
        parent,
        "Each modifier keeps its own value when moved.",
        11.0,
        true,
    );
    parent
        .spawn((
            musaic_button(
                Node {
                    width: percent(100),
                    min_height: px(34.0),
                    justify_content: JustifyContent::Center,
                    align_items: AlignItems::Center,
                    border: UiRect::all(px(1.0)),
                    border_radius: BorderRadius::all(px(6.0)),
                    ..default()
                },
                InspectorButtonAction(EditorCommand::NavigateToSurface {
                    surface: compound.surface,
                }),
                "View containing pattern",
            ),
            BackgroundColor(Color::WHITE),
            BorderColor::all(MusaicUiTheme::default().chrome.border),
        ))
        .observe(on_inspector_button_activated);
}

pub(super) fn compound_summary(compound: &OwnedCompoundView) -> String {
    let mut summary = compound_pitch(compound);
    for group in &compound.groups {
        let label = match group.role {
            OwnedTileGroupRole::Modifier(OperatorValue::At) => "weight",
            OwnedTileGroupRole::Modifier(OperatorValue::Multiply) => "speed",
            _ => continue,
        };
        if group.owned_value.is_some() {
            summary.push_str(&format!(
                " · {label} {}",
                group_glyph(group, GlyphPart::Operand)
            ));
        }
    }
    summary
}

pub(super) fn compound_pitch(compound: &OwnedCompoundView) -> String {
    [
        OwnedTileGroupRole::Pitch,
        OwnedTileGroupRole::Accidental,
        OwnedTileGroupRole::Octave,
    ]
    .into_iter()
    .flat_map(|role| {
        compound
            .groups
            .iter()
            .filter(move |group| group.role == role)
    })
    .map(|group| group_glyph(group, GlyphPart::Owner))
    .collect()
}

fn is_pitch_part(role: OwnedTileGroupRole) -> bool {
    matches!(
        role,
        OwnedTileGroupRole::Pitch | OwnedTileGroupRole::Accidental | OwnedTileGroupRole::Octave
    )
}

fn cell_node(size: f32) -> Node {
    Node {
        width: px(size),
        height: px(size),
        min_width: px(size),
        flex_shrink: 0.0,
        border: UiRect::all(px(1)),
        justify_content: JustifyContent::Center,
        align_items: AlignItems::Center,
        ..default()
    }
}

fn group_cell(
    parent: &mut ChildSpawnerCommands<'_>,
    group: &OwnedTileGroup,
    part: GlyphPart,
    pitched: bool,
) {
    let target = if matches!(part, GlyphPart::Operand) {
        group
            .owned_value
            .as_ref()
            .map(|value| &value.node)
            .unwrap_or(&group.owner)
    } else {
        &group.owner
    };
    let text = group_glyph(group, part);
    parent
        .spawn(musaic_button(
            cell_node(34.0),
            (
                InspectorTileGlyph {
                    node: group.owner.clone(),
                    part,
                },
                InspectorButtonAction(EditorCommand::SelectNode {
                    node: target.clone(),
                    mode: SelectionMode::Replace,
                }),
            ),
            &text,
        ))
        .remove::<ThemeFontColor>()
        .insert((
            BackgroundColor(if pitched { GREEN_SOFT } else { Color::WHITE }),
            BorderColor::all(MusaicUiTheme::default().chrome.border),
            TextColor(if pitched { GREEN } else { GOLD }),
            TextFont {
                font_size: if text.chars().count() > 3 { 11.0 } else { 16.0 },
                ..default()
            },
        ))
        .observe(on_inspector_button_activated);
}

pub(super) fn group_glyph(group: &OwnedTileGroup, part: GlyphPart) -> String {
    if matches!(part, GlyphPart::Operand) {
        return group
            .owned_value
            .as_ref()
            .map(|value| value.display.clone())
            .unwrap_or_default();
    }
    match group.role {
        OwnedTileGroupRole::Modifier(operator) => match operator {
            OperatorValue::At => "@",
            OperatorValue::Multiply => "×",
            OperatorValue::Divide => "÷",
            OperatorValue::Power => "^",
            OperatorValue::Choice => "|",
            OperatorValue::Parallel => ",",
        }
        .into(),
        OwnedTileGroupRole::SoundModifier(key) => parameter_glyph(key).into(),
        _ => group.label.clone(),
    }
}

fn shows_operand(group: &OwnedTileGroup) -> bool {
    group.owned_value.as_ref().is_some_and(|value| {
        value.node != group.owner
            || group_glyph(group, GlyphPart::Owner) != group_glyph(group, GlyphPart::Operand)
    })
}

fn group_role_label(role: OwnedTileGroupRole) -> &'static str {
    match role {
        OwnedTileGroupRole::Pitch => "Pitch",
        OwnedTileGroupRole::Octave => "Octave",
        OwnedTileGroupRole::Accidental => "Accidental",
        OwnedTileGroupRole::Modifier(OperatorValue::At) => "Sequence weight",
        OwnedTileGroupRole::Modifier(OperatorValue::Multiply) => "Pattern speed",
        OwnedTileGroupRole::Modifier(OperatorValue::Divide) => "Pattern duration",
        OwnedTileGroupRole::Modifier(OperatorValue::Power) => "Repeats",
        OwnedTileGroupRole::Modifier(OperatorValue::Choice) => "Alternate choices",
        OwnedTileGroupRole::Modifier(OperatorValue::Parallel) => "Play together",
        OwnedTileGroupRole::SoundModifier(key) => key.spec().label,
        OwnedTileGroupRole::RhythmModifier => "Rhythm",
        OwnedTileGroupRole::Value => "Value",
    }
}

fn parameter_glyph(key: ParameterKey) -> &'static str {
    use ParameterKey::*;
    match key {
        Fast => "×",
        Slow => "÷",
        Late => "Late",
        Gain => "g",
        Attack => "a",
        Transpose => "±",
        Gate => "G",
        Legato => "L",
        Sustain => "S",
        Delay => "Dly",
        Reverb => "Revb",
        Compressor => "Cmp",
        Velocity => "Vel",
        ClipLength => "Cl",
        PostGain => "PG",
        PitchBend => "PB",
        Expression => "Ex",

        Decay => "D",
        Release => "R",
        Pan => "Pan",
        HighPassCutoff => "HP",
        HighPassResonance => "HQ",
        LowPassCutoff => "LP",
        LowPassResonance => "Q",
        SampleBank => "B",
        SampleVariant => "v",
        PlaybackRate => "r",
        PlaybackStart => "[",
        PlaybackEnd => "]",
        Reverse => "←",
        Fit => "F",
        Loop => "↻",
        Slice => "Sl",
    }
}

pub(super) fn spawn_container_contents(
    parent: &mut ChildSpawnerCommands<'_>,
    node: &NodeId,
    content: &TileSurfaceContent,
) {
    let TileSurfaceContent::Container {
        children,
        child_count,
        ..
    } = content
    else {
        return;
    };
    detail_heading(parent, "Contents", &format!("{child_count} tiles"));
    parent
        .spawn((
            Node {
                padding: UiRect::all(px(10)),
                display: Display::Grid,
                width: percent(100),
                grid_template_columns: RepeatedGridTrack::flex(3, 1.0),
                column_gap: px(1),
                row_gap: px(1),
                align_self: AlignSelf::Start,
                border: UiRect::all(px(1)),
                ..default()
            },
            BackgroundColor(MusaicUiTheme::default().chrome.panel_inset),
            BorderColor::all(GOLD),
        ))
        .with_children(|grid| {
            for index in 0..children.len() {
                if let Some(child) = children.get(index) {
                    preview_cell(grid, &child.content);
                } else {
                    grid.spawn((
                        cell_node(44.0),
                        BackgroundColor(Color::srgb(0.925, 0.933, 0.906)),
                        BorderColor::all(MusaicUiTheme::default().chrome.border),
                    ));
                }
            }
        });
    if *child_count > children.len() {
        inspector_label(
            parent,
            "Grouped notes and modifiers share a tile face.",
            11.0,
            true,
        );
    }
    parent
        .spawn(musaic_button(
            Node {
                width: percent(100),
                height: px(34),
                margin: UiRect::top(px(6)),
                border: UiRect::all(px(1)),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            InspectorButtonAction(EditorCommand::EnterContainer {
                container: node.clone(),
            }),
            "Open container  →",
        ))
        .insert((
            BackgroundColor(Color::WHITE),
            BorderColor::all(MusaicUiTheme::default().chrome.border),
            TextFont {
                font_size: 12.0,
                ..default()
            },
        ))
        .observe(on_inspector_button_activated);
}

fn preview_cell(parent: &mut ChildSpawnerCommands<'_>, content: &TileSurfaceContent) {
    let nested = matches!(content, TileSurfaceContent::Container { .. });
    let count = match content {
        TileSurfaceContent::Container { child_count, .. } => Some(*child_count),
        TileSurfaceContent::Compound { layers, .. } if *layers > 1 => Some(*layers),
        _ => None,
    };
    let paper = if nested { GOLD_SOFT } else { GREEN_SOFT };
    let edge = if nested {
        GOLD
    } else {
        Color::srgb(0.47, 0.67, 0.56)
    };
    parent
        .spawn((
            Node {
                flex_direction: FlexDirection::Column,
                row_gap: px(1),
                padding: UiRect {
                    left: px(4),
                    right: px(6),
                    top: px(4),
                    bottom: px(15),
                },
                width: percent(100),
                height: px(60),
                min_width: px(0),
                border: UiRect::all(px(1)),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                ..default()
            },
            BackgroundColor(paper),
            BorderColor::all(if nested {
                GOLD
            } else {
                MusaicUiTheme::default().chrome.border
            }),
        ))
        .with_children(|cell| {
            if count.is_some() {
                for (left, top, right, bottom) in [(8, 8, 2, 3), (6, 6, 4, 6), (4, 4, 6, 9)] {
                    cell.spawn((
                        Node {
                            position_type: PositionType::Absolute,
                            left: px(left),
                            top: px(top),
                            right: px(right),
                            bottom: px(bottom),
                            border: UiRect::all(px(1)),
                            ..default()
                        },
                        BackgroundColor(paper),
                        BorderColor::all(edge),
                        Pickable::IGNORE,
                    ));
                }
            }
            match content {
                TileSurfaceContent::Container { kind, children, .. } => {
                    cell.spawn((
                        UiText::new(container_glyph(*kind)),
                        TextFont {
                            font_size: 14.0,
                            ..default()
                        },
                        TextColor(GOLD),
                    ));
                    cell.spawn(Node {
                        display: Display::Flex,
                        column_gap: px(2),
                        justify_content: JustifyContent::Center,
                        ..default()
                    })
                    .with_children(|mini| {
                        for child in children.iter().take(2) {
                            mini.spawn((
                                Node {
                                    min_width: px(20),
                                    height: px(19),
                                    padding: UiRect::horizontal(px(2)),
                                    border: UiRect::all(px(1)),
                                    justify_content: JustifyContent::Center,
                                    align_items: AlignItems::Center,
                                    ..default()
                                },
                                BackgroundColor(GREEN_SOFT),
                                BorderColor::all(MusaicUiTheme::default().chrome.border),
                            ))
                            .with_children(|face| {
                                face.spawn((
                                    UiText::new(preview_glyph(&child.content)),
                                    TextFont {
                                        font_size: 10.0,
                                        ..default()
                                    },
                                    TextColor(GREEN),
                                ));
                            });
                        }
                    });
                }
                TileSurfaceContent::Compound { display, .. } => {
                    cell.spawn((
                        UiText::new(display.replace('♯', "#").replace('♭', "b")),
                        TextFont {
                            font_size: if display.chars().count() > 4 {
                                11.0
                            } else {
                                15.0
                            },
                            ..default()
                        },
                        TextColor(GREEN),
                    ));
                }
                _ => {
                    cell.spawn((
                        UiText::new(preview_glyph(content)),
                        TextFont {
                            font_size: 16.0,
                            ..default()
                        },
                        TextColor(GREEN),
                    ));
                }
            }
            if let Some(count) = count {
                let ink = if nested { GOLD } else { GREEN };
                cell.spawn((
                    Node {
                        position_type: PositionType::Absolute,
                        right: px(2),
                        bottom: px(1),
                        padding: UiRect::horizontal(px(1)),
                        column_gap: px(1),
                        align_items: AlignItems::Center,
                        ..default()
                    },
                    BackgroundColor(paper),
                ))
                .with_children(|badge| {
                    // Drawing the stack mark keeps it independent of the UI
                    // font's Unicode coverage and as thin as the paper edges.
                    badge
                        .spawn(Node {
                            flex_direction: FlexDirection::Column,
                            row_gap: px(2),
                            ..default()
                        })
                        .with_children(|symbol| {
                            for _ in 0..3 {
                                symbol.spawn((
                                    Node {
                                        width: px(7),
                                        height: px(1),
                                        ..default()
                                    },
                                    BackgroundColor(ink),
                                ));
                            }
                        });
                    badge.spawn((
                        UiText::new(count.to_string()),
                        TextFont {
                            font_size: 12.0,
                            ..default()
                        },
                        TextColor(ink),
                    ));
                });
            }
        });
}

fn preview_glyph(content: &TileSurfaceContent) -> String {
    match content {
        TileSurfaceContent::Scalar { display } | TileSurfaceContent::Compound { display, .. } => {
            display.clone()
        }
        TileSurfaceContent::Container { kind, .. } => container_glyph(*kind).into(),
        TileSurfaceContent::Transform { label, .. } => label.chars().take(3).collect(),
        TileSurfaceContent::Wire { .. } => "→".into(),
        TileSurfaceContent::Empty => "·".into(),
    }
}

fn detail_heading(parent: &mut ChildSpawnerCommands<'_>, title: &str, meta: &str) {
    parent
        .spawn((
            Node {
                width: percent(100),
                justify_content: JustifyContent::SpaceBetween,
                margin: UiRect::top(px(12)),
                padding: UiRect::top(px(14)),
                border: UiRect::top(px(1)),
                ..default()
            },
            BorderColor::all(MusaicUiTheme::default().chrome.border),
        ))
        .with_children(|row| {
            inspector_label(row, title, 12.0, false);
            inspector_label(row, meta, 11.0, true);
        });
}
