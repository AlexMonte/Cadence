//! Compact, top-facing tile marks, rebuilt with the tile's visual identity.
//!
//! Glyphs and nested previews share one vertex-colored mesh per tile and one
//! retained font atlas. Selection and authored previews reconcile together.

use bevy::{
    asset::RenderAssetUsages, mesh::Indices, prelude::*, render::render_resource::PrimitiveTopology,
};

use super::theme::MusaicUiTheme;
use crate::application::pipeline::scene_sync::{TileSurfaceContent, VisibleNodeKind};
use crate::domain::document::{ContainerKind, PortSlotState};

pub(crate) fn content_for_spawn(
    tile: &crate::domain::document::TileSpawnKind,
) -> crate::application::pipeline::scene_sync::TileSurfaceContent {
    use crate::application::pipeline::scene_sync::TileSurfaceContent;
    use crate::domain::document::TileSpawnKind;
    match tile {
        TileSpawnKind::Atom { atom } => TileSurfaceContent::Scalar {
            display: crate::application::pipeline::scene_sync::surface_content::format_atom_display(
                atom,
            ),
        },
        TileSpawnKind::Container { kind } => TileSurfaceContent::Container {
            kind: *kind,
            children: Vec::new(),
            child_count: 0,
        },
        TileSpawnKind::TrickInstance { prototype } if prototype.0 == 32 => {
            TileSurfaceContent::Wire {
                ports: crate::domain::document::PortEndpointConfig {
                    west: PortSlotState::Input,
                    east: PortSlotState::Output,
                    ..Default::default()
                },
            }
        }
        TileSpawnKind::TrickInstance { prototype } => TileSurfaceContent::Transform {
            label: crate::domain::transform::transform_kind_from_prototype(*prototype)
                .map(|kind| format!("{kind:?}"))
                .unwrap_or_else(|| "?".into()),
            aux: None,
        },
        TileSpawnKind::Sound { .. } => TileSurfaceContent::Transform {
            label: "Sound".into(),
            aux: None,
        },
        TileSpawnKind::FlowControl { control } => TileSurfaceContent::Transform {
            label: crate::domain::flow::label(control.kind).into(),
            aux: None,
        },
        _ => TileSurfaceContent::Empty,
    }
}

pub(crate) fn compact_tile_mesh(
    kind: VisibleNodeKind,
    content: &TileSurfaceContent,
    face: Vec2,
    selected: bool,
    focused: bool,
    theme: &MusaicUiTheme,
) -> Mesh {
    let mut marks = Marks::default();
    let ink = content_ink(kind, content, theme);
    marks.rect(Vec2::ZERO, face, 0.035, Color::WHITE);
    marks.outline(Vec2::ZERO, face, 0.012, 0.040, theme.chrome.border);
    draw_content(&mut marks, kind, content, Vec2::ZERO, face, ink, theme, 0);
    if selected || focused {
        let color = theme.semantic.focus_accent;
        let length = face.min_element().min(0.65) * 0.26;
        let stroke = 0.026;
        for x in [-1.0, 1.0] {
            for z in [-1.0, 1.0] {
                marks.rect(
                    Vec2::new(x * (face.x - length) * 0.5, z * (face.y - stroke) * 0.5),
                    Vec2::new(length, stroke),
                    0.180,
                    color,
                );
                marks.rect(
                    Vec2::new(x * (face.x - stroke) * 0.5, z * (face.y - length) * 0.5),
                    Vec2::new(stroke, length),
                    0.180,
                    color,
                );
            }
        }
    }
    marks.mesh()
}

/// Keep readable principal labels; remove nested previews and modifier badges.
pub(crate) fn simplified_tile_mesh(
    kind: VisibleNodeKind,
    content: &TileSurfaceContent,
    face: Vec2,
    selected: bool,
    focused: bool,
    theme: &MusaicUiTheme,
    show_label: bool,
) -> Mesh {
    let mut marks = Marks::default();
    let ink = content_ink(kind, content, theme);
    let container = matches!(content, TileSurfaceContent::Container { .. });
    let paper = if container || is_note(content) {
        NOTE_PAPER
    } else {
        Color::WHITE
    };
    marks.rect(Vec2::ZERO, face, 0.035, paper);
    marks.outline(
        Vec2::ZERO,
        face,
        0.018,
        0.040,
        if container {
            theme.semantic.atom_scalar
        } else {
            theme.chrome.border
        },
    );
    if container {
        marks.rect(
            Vec2::new((-face.x + 0.82) * 0.5, 0.0),
            Vec2::new(0.82, face.y),
            0.045,
            CONTAINER_PAPER,
        );
        marks.rect(
            Vec2::new(0.0, face.y * 0.47),
            Vec2::new(face.x, face.y * 0.06),
            0.05,
            theme.semantic.atom_scalar,
        );
    }
    if matches!(content, TileSurfaceContent::Wire { .. }) {
        draw_content(&mut marks, kind, content, Vec2::ZERO, face, ink, theme, 0);
    } else if show_label {
        let label = summary_label(kind, content);
        marks.text(
            &label,
            Vec2::ZERO,
            face.x * 0.80,
            face.min_element() * 0.52,
            0.06,
            ink,
        );
    } else {
        // Shape and paper preserve broad roles without pretending tiny text is readable.
        let unit = face.min_element();
        if container {
            for i in -1..=1 {
                marks.rect(
                    Vec2::new(i as f32 * unit * 0.55, 0.0),
                    Vec2::new(unit * 0.30, unit * 0.45),
                    0.06,
                    NOTE_EDGE,
                );
            }
        } else if matches!(content, TileSurfaceContent::Scalar {display} | TileSurfaceContent::Compound {display,..} if display=="~" || display=="rest")
        {
            marks.rect(Vec2::ZERO, Vec2::new(unit * 0.46, unit * 0.08), 0.06, ink);
        } else if is_note(content) {
            marks.rect(
                Vec2::new(-unit * 0.06, unit * 0.14),
                Vec2::new(unit * 0.30, unit * 0.18),
                0.06,
                ink,
            );
            marks.rect(
                Vec2::new(unit * 0.10, -unit * 0.08),
                Vec2::new(unit * 0.08, unit * 0.50),
                0.06,
                ink,
            );
        } else if kind == VisibleNodeKind::Output {
            marks.triangle(
                Vec2::new(unit * 0.25, 0.0),
                Vec2::new(-unit * 0.20, -unit * 0.20),
                Vec2::new(-unit * 0.20, unit * 0.20),
                0.06,
                ink,
            );
        } else if kind == VisibleNodeKind::Atom {
            marks.outline(Vec2::ZERO, Vec2::splat(unit * 0.36), unit * 0.07, 0.06, ink);
        } else {
            for x in [-0.12, 0.12] {
                marks.rect(
                    Vec2::new(unit * x, 0.0),
                    Vec2::new(unit * 0.08, unit * 0.44),
                    0.06,
                    ink,
                );
            }
        }
    }
    if selected || focused {
        let unit = face.min_element();
        let length = unit.min(0.65) * 0.26;
        let stroke = 0.04;
        for x in [-1.0, 1.0] {
            for z in [-1.0, 1.0] {
                marks.rect(
                    Vec2::new(x * (face.x - length) * 0.5, z * (face.y - stroke) * 0.5),
                    Vec2::new(length, stroke),
                    0.18,
                    theme.semantic.focus_accent,
                );
                marks.rect(
                    Vec2::new(x * (face.x - stroke) * 0.5, z * (face.y - length) * 0.5),
                    Vec2::new(stroke, length),
                    0.18,
                    theme.semantic.focus_accent,
                );
            }
        }
    }
    marks.mesh()
}
fn summary_label(kind: VisibleNodeKind, content: &TileSurfaceContent) -> String {
    let glyph = content_glyph(kind, content);
    if let TileSurfaceContent::Container { child_count, .. } = content {
        format!("{glyph} · {child_count}")
    } else {
        glyph
    }
}
/// Match the atlas fitting policy instead of assuming every label has three letters.
fn fitted_text_height(text: &str, width: f32, height: f32) -> f32 {
    let mut advance = 0.0;
    let mut top = 0.0_f32;
    let mut bottom = 0.0_f32;
    for c in text.chars() {
        if let Some((_, m)) =
            super::tile_glyphs::glyph(c).or_else(|| super::tile_glyphs::glyph('?'))
        {
            advance += m[4];
            top = top.min(m[1]);
            bottom = bottom.max(m[1] + m[3]);
        }
    }
    let glyph_height = (bottom - top).max(1.0);
    glyph_height * (width / advance.max(1.0)).min(height / glyph_height)
}
pub(crate) fn detail_thresholds(
    kind: VisibleNodeKind,
    content: &TileSurfaceContent,
    face: Vec2,
) -> (f32, f32) {
    let label = summary_label(kind, content);
    let label_height = fitted_text_height(&label, face.x * 0.8, face.min_element() * 0.52);
    let full = if let TileSurfaceContent::Compound { display, .. } = content {
        display.clone()
    } else {
        content_glyph(kind, content)
    };
    let full_height = fitted_text_height(&full, face.x * 0.64, face.min_element() * 0.33);
    (
        (12.0 / full_height.max(0.001)).max(64.0),
        (12.0 / label_height.max(0.001)).max(28.0),
    )
}
/// Same container terminal hit rectangle, with no small text or count badge.
pub(crate) fn simplified_container_port_mesh(state: PortSlotState, theme: &MusaicUiTheme) -> Mesh {
    let mut marks = Marks::default();
    marks.rect(Vec2::ZERO, Vec2::new(0.35, 0.9), 0.0, NOTE_PAPER);
    if state != PortSlotState::None {
        let direction = if state == PortSlotState::Output {
            1.0
        } else {
            -1.0
        };
        marks.triangle(
            Vec2::new(0.14 * direction, 0.0),
            Vec2::new(-0.08 * direction, -0.12),
            Vec2::new(-0.08 * direction, 0.12),
            0.003,
            theme.semantic.atom_note,
        );
    }
    marks.mesh()
}

/// Normalized face bounds shared by rendering, connection endpoints and activity.
pub(crate) fn face_size(_kind: VisibleNodeKind) -> Vec2 {
    Vec2::ONE
}

/// A small arrow at the receiving tile, with local +X following the connection.
pub(crate) fn connection_arrow_mesh() -> Mesh {
    let mut marks = Marks::default();
    marks.triangle(
        Vec2::new(0.0, 0.0),
        Vec2::new(-0.15, -0.065),
        Vec2::new(-0.15, 0.065),
        0.02,
        Color::WHITE,
    );
    marks.mesh()
}

fn layers(content: &TileSurfaceContent) -> usize {
    match content {
        TileSurfaceContent::Compound { layers, .. } => *layers,
        _ => 1,
    }
}

fn content_ink(
    kind: VisibleNodeKind,
    content: &TileSurfaceContent,
    theme: &MusaicUiTheme,
) -> Color {
    if is_note(content) {
        return theme.semantic.atom_note;
    }
    match kind {
        VisibleNodeKind::Atom | VisibleNodeKind::Container => theme.semantic.atom_scalar,
        _ => theme.chrome.text_main,
    }
}

fn is_note(content: &TileSurfaceContent) -> bool {
    let display = match content {
        TileSurfaceContent::Scalar { display } | TileSurfaceContent::Compound { display, .. } => {
            display
        }
        _ => return false,
    };
    matches!(display.as_str(), "~" | "rest" | "bd" | "sd" | "hh" | "oh")
        || display
            .chars()
            .next()
            .is_some_and(|c| ('A'..='G').contains(&c))
            && display
                .chars()
                .skip(1)
                .all(|c| c.is_ascii_digit() || matches!(c, '#' | '♯' | '♭' | 'b' | '♮' | '-'))
}

pub(crate) fn content_glyph(kind: VisibleNodeKind, content: &TileSurfaceContent) -> String {
    match content {
        TileSurfaceContent::Wire { .. } => "→".into(),
        TileSurfaceContent::Scalar { display } | TileSurfaceContent::Compound { display, .. } => {
            if display == "rest" {
                return "~".into();
            }
            display.split_whitespace().next().unwrap_or("?").to_owned()
        }
        TileSurfaceContent::Transform { label, .. } => match label.as_str() {
            "Instrument" => "Inst",
            "Kit" => "kit",
            "Trick" => "Trk",
            "Fast" => "×",
            "Slow" => "/",
            "Rev" | "Reverse" => "↔",
            "Gain" => "g",
            "Attack" => "a",
            "Transpose" => "↑",
            "Degrade" => "%",
            "Gate" => "Gate",
            "Legato" => "L",
            "Sustain" => "S",
            "PlaybackRate" => "Rate",
            "PlaybackStart" => "In",
            "PlaybackEnd" => "Out",
            "Fit" => "Fit",
            "Loop" => "Loop",
            "Delay" => "Dly",
            "Reverb" => "Rvb",
            "Compressor" => "Cmp",

            "Velocity" => "Vel",
            "ClipLength" => "Cl",
            "PostGain" => "PG",
            "PitchBend" => "PB",
            "Expression" => "Ex",

            "Decay" => "D",
            "Release" => "R",
            "Pan" => "Pan",
            "HighPassCutoff" => "HP",
            "HighPassResonance" => "HQ",
            "LowPassCutoff" => "LP",
            "LowPassResonance" => "Q",
            "SampleVariant" => "V",
            "Layer" => "Lay",
            "Merge" => "Mrg",
            "Mix" => "Mix",
            "Split" => "Spl",
            "Mask" => "Msk",
            "Switch" => "Swt",
            "Route" => "Rte",
            "Choice" => "Chc",
            _ => return label.chars().take(3).collect(),
        }
        .into(),
        TileSurfaceContent::Container { kind, .. } => match kind {
            ContainerKind::Sequence => "[]",
            ContainerKind::Arrangement => "Song",
            ContainerKind::Subdivision => "[:]",
            ContainerKind::Alternating => "<>",
            ContainerKind::Parallel => "=",
        }
        .into(),
        TileSurfaceContent::Empty => match kind {
            VisibleNodeKind::Container => "[]",
            VisibleNodeKind::Output => "↗",
            _ => "·",
        }
        .into(),
    }
}

const NOTE_PAPER: Color = Color::srgb(0.890, 0.933, 0.902);
const NOTE_EDGE: Color = Color::srgb(0.47, 0.67, 0.56);
const CONTAINER_PAPER: Color = Color::srgb(0.949, 0.925, 0.847);

fn draw_content(
    marks: &mut Marks,
    kind: VisibleNodeKind,
    content: &TileSurfaceContent,
    center: Vec2,
    dims: Vec2,
    ink: Color,
    theme: &MusaicUiTheme,
    depth: usize,
) {
    let glyph = content_glyph(kind, content);
    let unit = dims.min_element();
    let y = 0.055 + depth as f32 * 0.03;
    if let TileSurfaceContent::Wire { ports } = content {
        marks.rect(center, dims, y, Color::WHITE);
        let color = theme.semantic.atom_note;
        for (direction, state) in [
            (Vec2::NEG_Y, ports.north),
            (Vec2::X, ports.east),
            (Vec2::Y, ports.south),
            (Vec2::NEG_X, ports.west),
        ] {
            if state == PortSlotState::None {
                continue;
            }
            let end = direction * dims * 0.5;
            marks.rect(
                center + end * 0.5,
                end.abs() + Vec2::splat(0.028),
                y + 0.01,
                color,
            );
            if state == PortSlotState::Output {
                let tip = center + direction * dims * 0.39;
                let back = tip - direction * 0.13;
                let normal = Vec2::new(-direction.y, direction.x) * 0.065;
                marks.triangle(tip, back - normal, back + normal, y + 0.02, color);
            }
        }
        return;
    }
    if let TileSurfaceContent::Container {
        children,
        child_count,
        ..
    } = content
    {
        let brass = theme.semantic.atom_scalar;
        if depth > 0 {
            // Nested previews use a clear heading and small child faces, rather
            // than repeating a full horizontal rail inside a tiny thumbnail.
            marks.rect(center, dims, y, CONTAINER_PAPER);
            stacked_paper(marks, center, dims, y, CONTAINER_PAPER, brass);
            marks.text(
                &glyph,
                center + Vec2::new(0.0, -dims.y * 0.25),
                dims.x * 0.6,
                unit * 0.25,
                y + 0.01,
                brass,
            );
            let count = children.len().min(2);
            for (index, child) in children.iter().take(2).enumerate() {
                let width = (dims.x - unit * 0.18) / count as f32;
                let at = center
                    + Vec2::new(
                        -dims.x * 0.5 + unit * 0.07 + (index as f32 + 0.5) * width,
                        dims.y * 0.06,
                    );
                let bounds = Vec2::new(width - unit * 0.035, dims.y * 0.33);
                marks.rect(at, bounds, y + 0.005, NOTE_PAPER);
                marks.outline(at, bounds, 0.008, y + 0.006, NOTE_EDGE);
                let child_glyph = content_glyph(child.kind, &child.content);
                marks.text(
                    &child_glyph,
                    at,
                    bounds.x * 0.85,
                    bounds.y * 0.55,
                    y + 0.012,
                    theme.semantic.atom_note,
                );
            }
            stack_badge(
                marks,
                center,
                dims,
                unit,
                *child_count,
                y + 0.016,
                CONTAINER_PAPER,
                brass,
            );
            return;
        }
        let rail = 0.82;
        let terminal = 0.38;
        marks.rect(center, dims, y, NOTE_PAPER);
        marks.rect(
            center + Vec2::new((-dims.x + rail) * 0.5, 0.0),
            Vec2::new(rail, dims.y),
            y + 0.001,
            CONTAINER_PAPER,
        );
        marks.outline(
            center,
            dims - Vec2::splat(unit * 0.015),
            unit * 0.012,
            y + 0.002,
            brass,
        );
        marks.rect(
            center + Vec2::new(0.0, dims.y * 0.5 - unit * 0.025),
            Vec2::new(dims.x, unit * 0.05),
            y + 0.004,
            brass,
        );
        let heading = center + Vec2::new((-dims.x + rail) * 0.5, 0.0);
        marks.text(&glyph, heading, rail * 0.65, unit * 0.36, y + 0.01, brass);
        let connector = center + Vec2::new((dims.x - terminal) * 0.5, 0.0);
        marks.rect(
            connector + Vec2::new(-terminal * 0.5, 0.0),
            Vec2::new(unit * 0.008, dims.y * 0.96),
            y + 0.003,
            NOTE_EDGE,
        );
        marks.text(
            "~",
            connector - Vec2::new(terminal * 0.14, 0.0),
            terminal * 0.42,
            unit * 0.25,
            y + 0.01,
            theme.semantic.atom_note,
        );
        marks.triangle(
            connector + Vec2::new(terminal * 0.36, 0.0),
            connector + Vec2::new(terminal * 0.05, -unit * 0.07),
            connector + Vec2::new(terminal * 0.05, unit * 0.07),
            y + 0.012,
            theme.semantic.atom_note,
        );
        if !children.is_empty() {
            let area = Vec2::new(dims.x - rail - terminal, dims.y - unit * 0.06);
            let origin = center + Vec2::new(-dims.x * 0.5 + rail, -dims.y * 0.5);
            // A bounded miniature strip keeps its contents readable. The count
            // reports all authored tiles; the ellipsis exposes clipped contents.
            let limit = 3;
            let count = children.len().min(limit);
            let weight =
                |child: &crate::application::pipeline::scene_sync::types::TilePreviewCell| {
                    if child.kind == VisibleNodeKind::Container {
                        2.0
                    } else {
                        1.0
                    }
                };
            let total: f32 = children.iter().take(count).map(weight).sum();
            let mut offset = 0.0;
            for (index, child) in children.iter().take(limit).enumerate() {
                let child_width = area.x * weight(child) / total;
                let at = origin + Vec2::new(offset + child_width * 0.5, area.y * 0.5);
                offset += child_width;
                let bounds = Vec2::new(child_width, area.y);
                draw_content(
                    marks,
                    child.kind,
                    &child.content,
                    at,
                    bounds,
                    content_ink(child.kind, &child.content, theme),
                    theme,
                    depth + 1,
                );
                if index > 0 {
                    marks.rect(
                        at - Vec2::new(child_width * 0.5, 0.0),
                        Vec2::new(unit * 0.008, area.y),
                        y + 0.015,
                        NOTE_EDGE,
                    );
                }
            }
            if children.len() > limit
                || children.iter().map(|c| layers(&c.content)).sum::<usize>() < *child_count
            {
                marks.text(
                    "…",
                    center + Vec2::new(dims.x * 0.5 - terminal - unit * 0.12, unit * 0.35),
                    unit * 0.2,
                    unit * 0.1,
                    y + 0.1,
                    brass,
                );
            }
        }
        // The terminal has a quiet footer below its connection target. Keep the
        // container's count here, separate from both its heading and child counts.
        stack_badge(
            marks,
            connector,
            Vec2::new(terminal, dims.y),
            unit,
            *child_count,
            y + 0.105,
            NOTE_PAPER,
            brass,
        );
        return;
    }
    let stacked_note = is_note(content) && layers(content) > 1;
    let paper = if is_note(content) {
        NOTE_PAPER
    } else {
        Color::WHITE
    };
    marks.rect(center, dims, y, paper);
    if stacked_note {
        stacked_paper(marks, center, dims, y, paper, NOTE_EDGE);
    }
    let label = if let TileSurfaceContent::Compound { display, .. } = content {
        display.as_str()
    } else {
        &glyph
    };
    marks.text(
        label,
        center - Vec2::new(0.0, if stacked_note { unit * 0.08 } else { 0.0 }),
        // Keep word labels clear of input sockets and incoming route arrows.
        dims.x
            * if matches!(content, TileSurfaceContent::Transform { .. }) {
                0.64
            } else {
                0.8
            },
        unit * if stacked_note { 0.33 } else { 0.39 },
        y + 0.012,
        ink,
    );
    if let TileSurfaceContent::Compound { layers, parts, .. } = content {
        if *layers > 1 {
            stack_badge(marks, center, dims, unit, *layers, y + 0.016, paper, ink);
            let mut icons = Vec::new();
            for part in parts.iter().skip(1) {
                if part.parse::<f64>().is_ok()
                    || matches!(part.as_str(), "#" | "b" | "♯" | "♭" | "♮")
                {
                    continue;
                }
                let icon: String = part
                    .split_whitespace()
                    .next()
                    .unwrap_or("")
                    .chars()
                    .take(2)
                    .collect();
                if !icons.contains(&icon) {
                    icons.push(icon);
                }
                if icons.len() == 2 {
                    break;
                }
            }
            if !icons.is_empty() {
                marks.text(
                    &icons.join(" "),
                    center + Vec2::new(-dims.x * 0.5 + unit * 0.27, unit * 0.35),
                    unit * 0.38,
                    unit * 0.14,
                    y + 0.015,
                    theme.semantic.atom_scalar,
                );
            }
        }
    }
}

/// Inset sheets leave the cell itself flush with its neighbors. The exposed
/// lower edges and corner count use the same treatment on notes and containers.
fn stacked_paper(marks: &mut Marks, center: Vec2, dims: Vec2, y: f32, paper: Color, edge: Color) {
    let unit = dims.min_element();
    let front = center - Vec2::new(unit * 0.035, unit * 0.055);
    let bounds = dims - Vec2::splat(unit * 0.20);
    for offset in [0.10, 0.05] {
        marks.outline(
            front + Vec2::splat(unit * offset),
            bounds,
            unit * 0.008,
            y + 0.002,
            edge,
        );
    }
    marks.rect(front, bounds, y + 0.004, paper);
    marks.outline(front, bounds, unit * 0.013, y + 0.005, edge);
}

fn stack_badge(
    marks: &mut Marks,
    center: Vec2,
    dims: Vec2,
    unit: f32,
    count: usize,
    y: f32,
    paper: Color,
    ink: Color,
) {
    let label = format!("≡{count}");
    let width = (unit * (0.19 * label.chars().count() as f32)).min(dims.x - unit * 0.04);
    let at = center
        + Vec2::new(
            (dims.x - width) * 0.5 - unit * 0.02,
            dims.y * 0.5 - unit * 0.15,
        );
    marks.rect(at, Vec2::new(width, unit * 0.24), y, paper);
    marks.text(
        &label,
        at,
        width - unit * 0.025,
        unit * 0.20,
        y + 0.001,
        ink,
    );
}

/// The container's terminal cell is a real port target, not decorative chrome.
pub(crate) fn compact_container_port_mesh(
    state: PortSlotState,
    content: &TileSurfaceContent,
    theme: &MusaicUiTheme,
) -> Mesh {
    let mut marks = Marks::default();
    marks.rect(Vec2::ZERO, Vec2::new(0.35, 0.9), 0.0, NOTE_PAPER);
    let ink = theme.semantic.atom_note;
    if state == PortSlotState::None {
        marks.text("·", Vec2::ZERO, 0.2, 0.25, 0.002, theme.chrome.text_dim);
    } else {
        marks.text("~", Vec2::new(-0.07, 0.0), 0.18, 0.26, 0.002, ink);
        let direction = if state == PortSlotState::Output {
            1.0
        } else {
            -1.0
        };
        marks.triangle(
            Vec2::new(0.14 * direction, 0.0),
            Vec2::new(0.02 * direction, -0.07),
            Vec2::new(0.02 * direction, 0.07),
            0.003,
            ink,
        );
    }
    // The port is a separate transparent mesh drawn over the tile face. Repeat
    // its footer here so the port's paper cannot cover the container's badge.
    if let TileSurfaceContent::Container { child_count, .. } = content {
        stack_badge(
            &mut marks,
            Vec2::ZERO,
            Vec2::new(0.38, 1.0),
            1.0,
            *child_count,
            0.010,
            NOTE_PAPER,
            theme.semantic.atom_scalar,
        );
    }
    marks.mesh()
}

/// Input is a hollow socket; output is a directional filled arrow. The mesh
/// is oriented toward the tile's north edge and rotated by the caller.
pub(crate) fn compact_port_mesh(state: PortSlotState, theme: &MusaicUiTheme) -> Mesh {
    let mut marks = Marks::default();
    let ink = if state == PortSlotState::Input {
        theme.semantic.flow_scalar
    } else {
        theme.semantic.container
    };
    match state {
        PortSlotState::None => marks.rect(Vec2::ZERO, Vec2::splat(0.045), 0.0, theme.chrome.border),
        PortSlotState::Input => {
            marks.outline(Vec2::ZERO, Vec2::new(0.14, 0.11), 0.025, 0.0, ink);
            marks.rect(Vec2::new(0.0, 0.085), Vec2::new(0.025, 0.08), 0.0, ink);
        }
        PortSlotState::Output => {
            // The arrow lives on the actual connection. This is only its socket.
            marks.rect(Vec2::ZERO, Vec2::splat(0.065), 0.0, ink);
        }
    }
    marks.mesh()
}

#[derive(Default)]
struct Marks {
    positions: Vec<[f32; 3]>,
    colors: Vec<[f32; 4]>,
    uvs: Vec<[f32; 2]>,
    indices: Vec<u32>,
}
impl Marks {
    fn rect(&mut self, center: Vec2, size: Vec2, y: f32, color: Color) {
        let a = center - size * 0.5;
        let b = center + size * 0.5;
        let base = self.positions.len() as u32;
        self.positions
            .extend([[a.x, y, a.y], [b.x, y, a.y], [b.x, y, b.y], [a.x, y, b.y]]);
        self.colors.extend([color.to_linear().to_f32_array(); 4]);
        self.uvs.extend([[2.0 / 2048.0, 2.0 / 1024.0]; 4]);
        self.indices
            .extend([base, base + 2, base + 1, base, base + 3, base + 2]);
    }
    fn triangle(&mut self, a: Vec2, b: Vec2, c: Vec2, y: f32, color: Color) {
        let base = self.positions.len() as u32;
        self.positions
            .extend([[a.x, y, a.y], [b.x, y, b.y], [c.x, y, c.y]]);
        self.colors.extend([color.to_linear().to_f32_array(); 3]);
        self.uvs.extend([[2.0 / 2048.0, 2.0 / 1024.0]; 3]);
        self.indices.extend([base, base + 1, base + 2]);
    }
    fn outline(&mut self, center: Vec2, size: Vec2, line: f32, y: f32, color: Color) {
        for side in [-1.0, 1.0] {
            self.rect(
                center + Vec2::new(side * size.x * 0.5, 0.0),
                Vec2::new(line, size.y),
                y,
                color,
            );
            self.rect(
                center + Vec2::new(0.0, side * size.y * 0.5),
                Vec2::new(size.x, line),
                y,
                color,
            );
        }
    }
    fn text(&mut self, text: &str, center: Vec2, width: f32, height: f32, y: f32, color: Color) {
        let glyphs: Vec<_> = text
            .chars()
            .filter_map(|c| super::tile_glyphs::glyph(c).or_else(|| super::tile_glyphs::glyph('?')))
            .collect();
        if glyphs.is_empty() {
            return;
        }
        let advance: f32 = glyphs.iter().map(|(_, m)| m[4]).sum();
        let top = glyphs.iter().map(|(_, m)| m[1]).fold(0.0_f32, f32::min);
        let bottom = glyphs
            .iter()
            .map(|(_, m)| m[1] + m[3])
            .fold(0.0_f32, f32::max);
        let scale = (width / advance.max(1.0)).min(height / (bottom - top).max(1.0));
        let mut x = center.x - advance * scale * 0.5;
        let baseline = center.y - (top + bottom) * scale * 0.5;
        for (uv, m) in glyphs {
            self.rect(
                Vec2::new(
                    x + (m[0] + m[2] * 0.5) * scale,
                    baseline + (m[1] + m[3] * 0.5) * scale,
                ),
                Vec2::new(m[2], m[3]) * scale,
                y,
                color,
            );
            let offset = self.uvs.len() - 4;
            self.uvs[offset..].copy_from_slice(&[
                [uv[0], uv[1]],
                [uv[2], uv[1]],
                [uv[2], uv[3]],
                [uv[0], uv[3]],
            ]);
            x += m[4] * scale;
        }
    }
    fn mesh(self) -> Mesh {
        let count = self.positions.len();
        let mut mesh = Mesh::new(
            PrimitiveTopology::TriangleList,
            RenderAssetUsages::default(),
        );
        mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, self.positions);
        mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, vec![[0.0, 1.0, 0.0]; count]);
        mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, self.uvs);
        mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, self.colors);
        mesh.insert_indices(Indices::U32(self.indices));
        mesh
    }
}

#[cfg(test)]
mod flow_glyph_tests {
    use super::*;
    #[test]
    fn every_flow_tile_has_a_distinct_compact_face() {
        let glyphs = crate::domain::flow::KINDS
            .into_iter()
            .map(|kind| {
                let content =
                    content_for_spawn(&crate::domain::document::TileSpawnKind::FlowControl {
                        control: tessera::prelude::FlowControlNode::new(kind),
                    });
                let glyph = content_glyph(VisibleNodeKind::TrickInstance, &content);
                assert_ne!(glyph, "?");
                glyph
            })
            .collect::<std::collections::BTreeSet<_>>();
        assert_eq!(glyphs.len(), 8);
    }
}
