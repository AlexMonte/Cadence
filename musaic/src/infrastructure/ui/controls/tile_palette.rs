//! A native, scrollable tile library. Tile sizes do not shrink with catalog size.
use super::library_search::{TileLibraryLabel, TileLibrarySearch};
use crate::{
    application::{
        editor::{
            TileDrawerItem, TileLibraryContextKind, panels::context_panel::TileLibraryCategory,
        },
        pipeline::ui_projection::EditorUiProjection,
    },
    domain::document::{AtomValue, TileSpawnKind},
    infrastructure::{
        app::AppState,
        ui::{
            DrawerTileSource, on_drawer_tile_press, on_drawer_tile_release, theme::MusaicUiTheme,
        },
    },
};
use bevy::{input::mouse::MouseScrollUnit, picking::prelude::*, prelude::*};

#[derive(Resource, Debug, Clone, Copy)]
pub struct UiTilePaletteDisplay {
    pub context: TileLibraryContextKind,
}
impl Default for UiTilePaletteDisplay {
    fn default() -> Self {
        Self {
            context: TileLibraryContextKind::RootBoard,
        }
    }
}

#[derive(Component, Default)]
pub(crate) struct TileLibraryBody {
    options: Vec<TileDrawerItem>,
    query: Option<String>,
    collection: Option<super::library_search::LibraryCollection>,
    preferences: crate::application::editor::preferences::library::LibraryPreferences,
}
#[derive(Component)]
pub(crate) struct LibraryContextLabel;
#[derive(Component)]
struct LibraryScrollThumb {
    viewport: Entity,
}

pub struct UiTilePalettePlugin;
impl Plugin for UiTilePalettePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<TileLibrarySearch>()
            .init_resource::<UiTilePaletteDisplay>()
            .add_systems(
                Update,
                (
                    super::library_search::sync,
                    super::library_collections::sync,
                )
                    .run_if(in_state(AppState::Editor)),
            )
            .add_systems(
                PostUpdate,
                sync_scrollbars
                    .after(bevy::ui::UiSystems::Layout)
                    .run_if(in_state(AppState::Editor)),
            );
        // Content sync is ordered after the editor projection in MusaicUiPlugin.
    }
}

pub(crate) fn spawn_tile_library(parent: &mut ChildSpawnerCommands<'_>) {
    let theme = MusaicUiTheme::default();
    parent.spawn((
        LibraryContextLabel,
        Text::new("Pattern board"),
        TextFont {
            font_size: 11.0,
            ..default()
        },
        TextColor(theme.chrome.text_dim),
        Pickable::IGNORE,
    ));
    parent
        .spawn(Node {
            width: percent(100),
            flex_basis: px(0),
            min_height: px(180),
            flex_grow: 1.0,
            flex_shrink: 1.0,
            column_gap: px(4),
            ..default()
        })
        .with_children(|row| {
            let viewport = row
                .spawn((
                    TileLibraryBody::default(),
                    Node {
                        flex_grow: 1.0,
                        flex_basis: px(0),
                        min_width: px(0),
                        height: percent(100),
                        overflow: Overflow::scroll_y(),
                        flex_direction: FlexDirection::Column,
                        row_gap: px(16),
                        padding: UiRect::right(px(4)),
                        ..default()
                    },
                    ScrollPosition::default(),
                ))
                .observe(scroll_library)
                .id();
            row.spawn((
                Node {
                    width: px(5),
                    height: percent(100),
                    flex_shrink: 0.0,
                    position_type: PositionType::Relative,
                    ..default()
                },
                BackgroundColor(theme.chrome.panel_inset),
                Pickable::IGNORE,
            ))
            .with_children(|rail| {
                rail.spawn((
                    LibraryScrollThumb { viewport },
                    Node {
                        position_type: PositionType::Absolute,
                        top: px(0),
                        width: percent(100),
                        height: percent(100),
                        border_radius: BorderRadius::all(px(3)),
                        ..default()
                    },
                    BackgroundColor(theme.chrome.border),
                    Pickable::IGNORE,
                ));
            });
        });
}

pub(crate) fn sync_ui_tile_palette_scene(
    mut commands: Commands,
    projection: Res<EditorUiProjection>,
    search: Res<TileLibrarySearch>,
    mut display: ResMut<UiTilePaletteDisplay>,
    mut bodies: Query<(
        Entity,
        &mut TileLibraryBody,
        &mut ScrollPosition,
        Option<&Children>,
    )>,
    mut labels: Query<&mut Text, With<LibraryContextLabel>>,
    mut sources: Query<(&DrawerTileSource, &mut BorderColor)>,
    preferences: Res<crate::application::editor::preferences::EditorPreferences>,
    mut focus: ResMut<bevy::input_focus::InputFocus>,
    source_labels: Query<(&DrawerTileSource, &TileLibraryLabel)>,
    favorite_buttons: Query<&super::library_collections::FavoriteButton>,
    search_fields: Query<Entity, With<super::library_search::SearchField>>,
) {
    let theme = MusaicUiTheme::default();
    display.context = projection.palette.context;
    let context = match display.context {
        TileLibraryContextKind::RootBoard => "Pattern board · drag a tile, or click then place",
        TileLibraryContextKind::ContainerBody => {
            "Inside pattern · drag a tile, or click then place"
        }
    };
    for mut text in &mut labels {
        if text.0 != context {
            text.0 = context.into();
        }
    }
    for (source, mut border) in &mut sources {
        let color = if projection.palette.armed.as_ref() == Some(&source.tile) {
            theme.semantic.focus_accent
        } else {
            Color::NONE
        };
        let next = BorderColor::all(color);
        if *border != next {
            *border = next;
        }
    }
    for (entity, mut body, mut scroll, children) in &mut bodies {
        if body.query.as_ref() == Some(&search.query)
            && body.options == projection.palette.options
            && body.collection == Some(search.collection)
            && (search.collection == super::library_search::LibraryCollection::All
                || body.preferences == preferences.library)
        {
            continue;
        }
        let sections = library_sections(&projection.palette.options, &search, &preferences.library);
        body.query = Some(search.query.clone());
        body.options.clone_from(&projection.palette.options);
        body.collection = Some(search.collection);
        body.preferences.clone_from(&preferences.library);
        let restore_key = focus.0.and_then(|focused| {
            source_labels
                .get(focused)
                .ok()
                .map(|(source, label)| {
                    crate::application::editor::preferences::library::LibraryTileKey::new(
                        &source.tile,
                        &label.0,
                    )
                })
                .or_else(|| {
                    favorite_buttons
                        .get(focused)
                        .ok()
                        .map(|button| button.key.clone())
                })
        });
        let mut first = None;
        let mut restored = None;
        scroll.y = 0.0;
        if let Some(children) = children {
            for child in children.iter() {
                commands.entity(child).despawn();
            }
        }
        commands.entity(entity).with_children(|parent| {
            let mut order = 0;
            let mut first_row = 0;
            for (_, heading, items) in sections {
                if items.is_empty() {
                    continue;
                }
                let rows = items.len().div_ceil(5);
                parent
                    .spawn(Node {
                        width: percent(100),
                        flex_direction: FlexDirection::Column,
                        flex_shrink: 0.0,
                        row_gap: px(5),
                        ..default()
                    })
                    .with_children(|section| {
                        section.spawn((
                            Text::new(heading),
                            TextFont {
                                font_size: 13.0,
                                ..default()
                            },
                            TextColor(theme.chrome.text_main),
                            Pickable::IGNORE,
                        ));
                        section
                            .spawn(Node {
                                width: percent(100),
                                display: Display::Grid,
                                grid_template_columns: RepeatedGridTrack::flex(5, 1.0),
                                column_gap: px(4),
                                row_gap: px(4),
                                ..default()
                            })
                            .with_children(|grid| {
                                for (index, item) in items.into_iter().enumerate() {
                                    let spawned = spawn_entry(
                                        grid,
                                        item,
                                        projection.palette.armed.as_ref() == Some(&item.spawn),
                                        &theme,
                                        super::library_search::LibraryPosition {
                                            body: entity,
                                            order,
                                            row: first_row + index / 5,
                                            column: index % 5,
                                        },
                                    );
                                    first.get_or_insert(spawned);
                                    if restore_key.as_ref().is_some_and(|key| key.matches(item)) { restored = Some(spawned); }
                                    order += 1;
                                }
                            });
                    });
                first_row += rows;
            }
            if order == 0 {
                parent.spawn((Text::new(match search.collection {
                    super::library_search::LibraryCollection::Favorites => "No favorite tiles here. Save a tile in All tiles, or press F while it is focused.",
                    super::library_search::LibraryCollection::Recent => "No recent choices here. Choose a tile from All tiles to start.",
                    _ => "No matching tiles.",
                }), TextFont { font_size: 13.0, ..default() }, TextColor(theme.chrome.text_dim)));
            }
        });
        if restore_key.is_some() {
            if let Some(next) = restored.or(first) {
                if let Some(first) = first {
                    commands
                        .entity(first)
                        .insert(bevy::input_focus::tab_navigation::TabIndex(-1));
                }
                commands
                    .entity(next)
                    .insert(bevy::input_focus::tab_navigation::TabIndex(0));
                focus.set(next);
            } else {
                if let Ok(field) = search_fields.single() {
                    focus.set(field);
                } else {
                    focus.clear();
                }
            }
        }
    }
}

pub(super) fn library_sections<'a>(
    options: &'a [TileDrawerItem],
    search: &TileLibrarySearch,
    preferences: &crate::application::editor::preferences::library::LibraryPreferences,
) -> Vec<(
    Option<TileLibraryCategory>,
    &'static str,
    Vec<&'a TileDrawerItem>,
)> {
    use super::library_search::LibraryCollection;
    if search.collection != LibraryCollection::All {
        let (heading, keys) = match search.collection {
            LibraryCollection::Recent => ("Recent choices", preferences.recent()),
            _ => ("Favorites", preferences.favorites()),
        };
        let items = keys
            .iter()
            .filter_map(|key| {
                options
                    .iter()
                    .find(|item| key.matches(item) && search.matches_item(item))
            })
            .collect();
        return vec![(None, heading, items)];
    }
    TileLibraryCategory::ALL
        .into_iter()
        .map(|category| {
            let mut items: Vec<_> = options
                .iter()
                .filter(|item| item.category() == category && search.matches_item(item))
                .collect();
            if category == TileLibraryCategory::Numbers {
                items.sort_by_key(|item| match item.spawn {
                    TileSpawnKind::Atom {
                        atom: AtomValue::Number(value),
                    } => (0, value),
                    _ => (1, 0),
                });
            }
            (Some(category), category.label(), items)
        })
        .collect()
}

fn spawn_entry(
    parent: &mut ChildSpawnerCommands<'_>,
    item: &TileDrawerItem,
    armed: bool,
    theme: &MusaicUiTheme,
    position: super::library_search::LibraryPosition,
) -> Entity {
    let category = item.category();
    let green = matches!(category, TileLibraryCategory::Notes)
        || matches!(
            item.spawn,
            TileSpawnKind::Atom {
                atom: AtomValue::DrumHit(_)
            }
        );
    let ink = if green {
        theme.semantic.atom_note
    } else {
        theme.semantic.atom_scalar
    };
    let paper = if green {
        Color::srgb(0.886, 0.933, 0.898)
    } else if category == TileLibraryCategory::Patterns {
        Color::srgb(0.945, 0.922, 0.843)
    } else {
        Color::WHITE
    };
    let glyph = entry_glyph(item);
    let mut accessible = accesskit::Node::new(accesskit::Role::Button);
    accessible.set_label(item.label.clone());
    accessible.set_description("Enter or Space chooses this tile; then Enter places it on the board. F adds or removes it from favorites.");
    parent
        .spawn((
            DrawerTileSource {
                tile: item.spawn.clone(),
            },
            TileLibraryLabel(item.label.clone()),
            position,
            bevy::input_focus::tab_navigation::TabIndex(if position.order == 0 { 0 } else { -1 }),
            bevy::a11y::AccessibilityNode::from(accessible),
            super::super::widgets::ButtonAccessibilityLabel(item.label.clone()),
            Node {
                width: percent(100),
                min_width: px(0),
                min_height: px(46),
                padding: UiRect::all(px(3)),
                border: UiRect::all(px(1)),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                row_gap: px(2),
                ..default()
            },
            BorderColor::all(if armed {
                theme.semantic.focus_accent
            } else {
                Color::NONE
            }),
            Pickable::default(),
        ))
        .observe(super::library_search::hover)
        .observe(on_drawer_tile_press)
        .observe(on_drawer_tile_release)
        .observe(crate::infrastructure::ui::inspector::panels::drawer::on_drawer_tile_key)
        .with_children(|card| {
            card.spawn((
                Node {
                    width: px(36),
                    height: px(36),
                    flex_shrink: 0.0,
                    border: UiRect::all(px(1)),
                    align_items: AlignItems::Center,
                    justify_content: JustifyContent::Center,
                    ..default()
                },
                BackgroundColor(paper),
                BorderColor::all(theme.chrome.border),
                Pickable::IGNORE,
            ))
            .with_children(|face| {
                face.spawn((
                    Text::new(&glyph),
                    TextFont {
                        font_size: if glyph.chars().count() > 3 {
                            13.0
                        } else {
                            20.0
                        },
                        ..default()
                    },
                    TextColor(ink),
                    Pickable::IGNORE,
                ));
            });
            super::library_collections::spawn_favorite(
                card,
                crate::application::editor::preferences::library::LibraryTileKey::new(
                    &item.spawn,
                    &item.label,
                ),
                &item.label,
                theme,
            );
        })
        .id()
}
fn entry_glyph(item: &TileDrawerItem) -> String {
    use crate::application::pipeline::scene_sync::surface_content::format_atom_display;
    match &item.spawn {
        TileSpawnKind::Atom {
            atom: AtomValue::Accidental(crate::domain::document::Accidental::Natural),
        } => "♮".into(),
        TileSpawnKind::Atom {
            atom: AtomValue::Rest,
        } => "~".into(),
        TileSpawnKind::Atom { atom } => format_atom_display(atom)
            .split_whitespace()
            .next()
            .unwrap_or("·")
            .chars()
            .take(5)
            .collect(),
        TileSpawnKind::Container { kind } => {
            crate::application::pipeline::selected_tile::container_glyph(*kind).into()
        }
        TileSpawnKind::Output { .. } => "→".into(),
        TileSpawnKind::TrickInstance { .. } | TileSpawnKind::FlowControl { .. } => {
            super::super::tile_surface::content_glyph(
                super::super::tile_visual::visible_kind_for_spawn(&item.spawn),
                &super::super::tile_surface::content_for_spawn(&item.spawn),
            )
        }
        _ => item.label.clone(),
    }
}
fn scroll_library(
    mut event: On<Pointer<Scroll>>,
    mut lists: Query<(&ComputedNode, &mut ScrollPosition), With<TileLibraryBody>>,
) {
    let Ok((computed, mut position)) = lists.get_mut(event.entity) else {
        return;
    };
    let scale = match event.unit {
        MouseScrollUnit::Line => 28.0,
        MouseScrollUnit::Pixel => 1.0,
    };
    let maximum = ((computed.content_size().y - computed.size().y)
        * computed.inverse_scale_factor())
    .max(0.0);
    position.y = (position.y - event.y * scale).clamp(0.0, maximum);
    event.propagate(false);
}
fn sync_scrollbars(
    mut lists: Query<(&ComputedNode, &mut ScrollPosition), With<TileLibraryBody>>,
    mut thumbs: Query<(&LibraryScrollThumb, &mut Node)>,
) {
    for (thumb, mut node) in &mut thumbs {
        let Ok((computed, mut position)) = lists.get_mut(thumb.viewport) else {
            continue;
        };
        let height = computed.size().y * computed.inverse_scale_factor();
        let content = computed.content_size().y * computed.inverse_scale_factor();
        let maximum = (content - height).max(0.0);
        position.y = position.y.clamp(0.0, maximum);
        let size = if content > 0.0 {
            (height * height / content).clamp(16.0_f32.min(height), height)
        } else {
            height
        };
        node.display = if maximum > 0.0 {
            Display::Flex
        } else {
            Display::None
        };
        node.height = px(size);
        node.top = px(if maximum > 0.0 {
            (height - size) * position.y / maximum
        } else {
            0.0
        });
    }
}

#[cfg(test)]
#[path = "tile_palette_tests.rs"]
mod tests;
