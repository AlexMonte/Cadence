//! Library-local discovery controls; musical edits remain owned by the editor.
use super::library_search::{LibraryCollection, TileLibrarySearch};
use crate::{
    application::editor::{
        interaction::keyboard_navigation::KeyboardNavigation,
        preferences::{EditorPreferences, library::LibraryTileKey},
    },
    infrastructure::ui::{
        theme::MusaicUiTheme,
        widgets::{ButtonAccessibilityLabel, ButtonLabel, musaic_chrome_button},
    },
};
use bevy::{input_focus::tab_navigation::TabIndex, prelude::*};
use bevy_ui_widgets::Activate;

#[derive(Component)]
pub(super) struct CollectionButton(LibraryCollection);
#[derive(Component)]
pub(super) struct ClearCollection;
#[derive(Component)]
pub(crate) struct FavoriteButton {
    pub key: LibraryTileKey,
    pub label: String,
}

pub(crate) fn spawn(parent: &mut ChildSpawnerCommands<'_>) {
    let theme = MusaicUiTheme::default();
    parent
        .spawn(Node {
            width: percent(100),
            column_gap: px(4),
            ..default()
        })
        .with_children(|row| {
            for (label, collection) in [
                ("All tiles", LibraryCollection::All),
                ("Recent", LibraryCollection::Recent),
                ("Favorites", LibraryCollection::Favorites),
            ] {
                row.spawn(musaic_chrome_button(
                    &theme,
                    label,
                    CollectionButton(collection),
                ))
                .insert(Node {
                    height: px(32),
                    flex_basis: px(0),
                    flex_grow: 1.0,
                    justify_content: JustifyContent::Center,
                    align_items: AlignItems::Center,
                    border: UiRect::all(px(1)),
                    border_radius: BorderRadius::all(px(theme.radii.md)),
                    ..default()
                })
                .observe(choose_collection);
            }
        });
    parent
        .spawn(musaic_chrome_button(
            &theme,
            "Clear collection",
            ClearCollection,
        ))
        .insert(Node {
            display: Display::None,
            height: px(26),
            align_items: AlignItems::Center,
            justify_content: JustifyContent::Center,
            border_radius: BorderRadius::all(px(theme.radii.md)),
            ..default()
        })
        .observe(clear_collection);
}
fn clear_collection(
    _: On<Activate>,
    mut preferences: ResMut<EditorPreferences>,
    mut search: ResMut<TileLibrarySearch>,
) {
    match search.collection {
        LibraryCollection::All => return,
        LibraryCollection::Recent => preferences.library.clear_recent(),
        LibraryCollection::Favorites => preferences.library.clear_favorites(),
    }
    search.collection_error.clear();
}
fn choose_collection(
    event: On<Activate>,
    buttons: Query<&CollectionButton>,
    mut search: ResMut<TileLibrarySearch>,
) {
    if let Ok(button) = buttons.get(event.entity) {
        search.collection = button.0;
        search.hovered.clear();
        search.collection_error.clear();
    }
}
pub(crate) fn toggle(
    preferences: &mut EditorPreferences,
    navigation: &mut KeyboardNavigation,
    search: &mut TileLibrarySearch,
    key: LibraryTileKey,
    label: &str,
) {
    search.collection_error.clear();
    navigation.status = match preferences.library.toggle(key) {
        Ok(true) => format!("Added {label} to favorites"),
        Ok(false) => format!("Removed {label} from favorites"),
        Err(error) => {
            search.collection_error = error.into();
            error.into()
        }
    };
}
fn stop_press(mut event: On<Pointer<Press>>) {
    event.propagate(false);
}
fn favorite(
    event: On<Activate>,
    buttons: Query<&FavoriteButton>,
    mut preferences: ResMut<EditorPreferences>,
    mut navigation: ResMut<KeyboardNavigation>,
    mut search: ResMut<TileLibrarySearch>,
) {
    if let Ok(button) = buttons.get(event.entity) {
        toggle(
            &mut preferences,
            &mut navigation,
            &mut search,
            button.key.clone(),
            &button.label,
        );
    }
}
pub(super) fn spawn_favorite(
    parent: &mut ChildSpawnerCommands<'_>,
    key: LibraryTileKey,
    label: &str,
    theme: &MusaicUiTheme,
) {
    parent
        .spawn(musaic_chrome_button(
            theme,
            "+",
            (
                FavoriteButton {
                    key,
                    label: label.into(),
                },
                TabIndex(-1),
                ButtonAccessibilityLabel(format!("Add {label} to favorites")),
            ),
        ))
        .insert((
            Node {
                position_type: PositionType::Absolute,
                top: px(0),
                right: px(0),
                width: px(14),
                height: px(14),
                min_width: px(0),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                border_radius: BorderRadius::all(px(theme.radii.sm)),
                ..default()
            },
            TextFont {
                font_size: 12.0,
                ..default()
            },
        ))
        .observe(stop_press)
        .observe(favorite);
}
pub(super) fn sync(
    preferences: Res<EditorPreferences>,
    search: Res<TileLibrarySearch>,
    mut favorites: Query<(
        Ref<FavoriteButton>,
        &mut ButtonLabel,
        &mut ButtonAccessibilityLabel,
    )>,
    mut collections: Query<(&CollectionButton, &mut BackgroundColor)>,
    theme: Res<MusaicUiTheme>,
    mut clear: Query<
        (&mut ButtonLabel, &mut Node),
        (With<ClearCollection>, Without<FavoriteButton>),
    >,
) {
    {
        for (button, mut label, mut accessible) in &mut favorites {
            if !preferences.is_changed() && !button.is_added() {
                continue;
            }
            let saved = preferences.library.is_favorite(&button.key);
            label.0 = if saved { "−" } else { "+" }.into();
            accessible.0 = format!(
                "{} {} {} favorites",
                if saved { "Remove" } else { "Add" },
                button.label,
                if saved { "from" } else { "to" }
            );
        }
    }
    for (button, mut background) in &mut collections {
        let color = if button.0 == search.collection {
            theme.semantic.atom_note.with_alpha(0.12)
        } else {
            theme.chrome.button_bg
        };
        if background.0 != color {
            background.0 = color;
        }
    }
    for (mut label, mut node) in &mut clear {
        let (display, text) = match search.collection {
            LibraryCollection::All => (Display::None, "Clear collection"),
            LibraryCollection::Recent => (Display::Flex, "Clear recent"),
            LibraryCollection::Favorites => (Display::Flex, "Clear favorites"),
        };
        if node.display != display {
            node.display = display;
        }
        if label.0 != text {
            label.0 = text.into();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn full_favorites_report_a_local_error_and_clearing_recovers() {
        use crate::domain::document::{AtomValue, TileSpawnKind};
        let key = |n| {
            LibraryTileKey::Builtin(Box::new(TileSpawnKind::Atom {
                atom: AtomValue::Number(n),
            }))
        };
        let mut preferences = EditorPreferences::default();
        for n in 0..64 {
            preferences.library.toggle(key(n)).unwrap();
        }
        let mut navigation = KeyboardNavigation::default();
        let mut search = TileLibrarySearch::default();
        toggle(
            &mut preferences,
            &mut navigation,
            &mut search,
            key(100),
            "100",
        );
        assert!(search.collection_error.contains("Favorites is full"));
        assert_eq!(preferences.library.favorites().len(), 64);
        preferences.library.choose(key(4));
        preferences.library.clear_favorites();
        toggle(
            &mut preferences,
            &mut navigation,
            &mut search,
            key(100),
            "100",
        );
        assert!(search.collection_error.is_empty());
        assert_eq!(preferences.library.favorites(), &[key(100)]);
        assert_eq!(preferences.library.recent(), &[key(4)]);
        preferences.library.clear_recent();
        assert_eq!(preferences.library.favorites(), &[key(100)]);
        assert!(preferences.library.recent().is_empty());
    }
}
