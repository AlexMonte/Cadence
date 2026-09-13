//! Personal discovery history. Never persist process-local entities or a song's
//! numeric reusable-pattern ID as a cross-project library identity.
use crate::{application::editor::TileDrawerItem, domain::document::TileSpawnKind};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum LibraryTileKey {
    Builtin(Box<TileSpawnKind>),
    NamedPattern(String),
}
impl LibraryTileKey {
    pub fn new(tile: &TileSpawnKind, label: &str) -> Self {
        match tile {
            TileSpawnKind::TrickInstance { prototype } if prototype.0 >= 1000 => {
                Self::NamedPattern(label.to_owned())
            }
            _ => Self::Builtin(Box::new(tile.clone())),
        }
    }
    pub fn matches(&self, item: &TileDrawerItem) -> bool {
        *self == Self::new(&item.spawn, &item.label)
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LibraryPreferences {
    favorites: Vec<LibraryTileKey>,
    recent: Vec<LibraryTileKey>,
}
impl LibraryPreferences {
    pub const RECENT_LIMIT: usize = 12;
    pub const FAVORITE_LIMIT: usize = 64;
    pub fn favorites(&self) -> &[LibraryTileKey] {
        &self.favorites
    }
    pub fn recent(&self) -> &[LibraryTileKey] {
        &self.recent
    }
    pub fn clear_favorites(&mut self) {
        self.favorites.clear();
    }
    pub fn clear_recent(&mut self) {
        self.recent.clear();
    }
    pub fn is_favorite(&self, key: &LibraryTileKey) -> bool {
        self.favorites.contains(key)
    }
    pub fn choose(&mut self, key: LibraryTileKey) {
        self.recent.retain(|old| old != &key);
        self.recent.insert(0, key);
        self.recent.truncate(Self::RECENT_LIMIT);
    }
    pub fn toggle(&mut self, key: LibraryTileKey) -> Result<bool, &'static str> {
        if self.is_favorite(&key) {
            self.favorites.retain(|old| old != &key);
            Ok(false)
        } else if self.favorites.len() >= Self::FAVORITE_LIMIT {
            Err("Favorites is full (64 tiles). Remove one before adding another.")
        } else {
            self.favorites.push(key);
            Ok(true)
        }
    }
    pub fn validate(&self) -> Result<(), String> {
        if self.recent.len() > Self::RECENT_LIMIT {
            return Err("Recent tiles exceed the current limit".into());
        }
        if self.favorites.len() > Self::FAVORITE_LIMIT {
            return Err("Favorite tiles exceed the current limit".into());
        }
        if has_duplicates(&self.recent) || has_duplicates(&self.favorites) {
            return Err("Tile collections contain duplicate entries".into());
        }
        Ok(())
    }
}

fn has_duplicates(values: &[LibraryTileKey]) -> bool {
    values
        .iter()
        .enumerate()
        .any(|(index, value)| values[index + 1..].contains(value))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::document::{AtomValue, GraphTilePrototypeId};
    fn key(n: i32) -> LibraryTileKey {
        LibraryTileKey::Builtin(Box::new(TileSpawnKind::Atom {
            atom: AtomValue::Number(n),
        }))
    }
    #[test]
    fn recent_choices_are_bounded_unique_and_favorites_survive_roundtrip() {
        let mut prefs = LibraryPreferences::default();
        for n in 0..20 {
            prefs.choose(key(n));
        }
        prefs.choose(key(10));
        assert_eq!(prefs.recent().len(), 12);
        assert_eq!(prefs.recent()[0], key(10));
        assert_eq!(prefs.recent().iter().filter(|k| **k == key(10)).count(), 1);
        assert_eq!(prefs.toggle(key(2)), Ok(true));
        assert_eq!(prefs.toggle(key(3)), Ok(true));
        assert_eq!(prefs.toggle(key(2)), Ok(false));
        let restored: LibraryPreferences =
            serde_json::from_slice(&serde_json::to_vec(&prefs).unwrap()).unwrap();
        assert_eq!(restored, prefs);
        assert_eq!(restored.favorites(), &[key(3)]);
    }
    #[test]
    fn named_patterns_follow_names_instead_of_reusing_foreign_numeric_ids() {
        let item = |id, name: &str| TileDrawerItem {
            label: name.into(),
            spawn: TileSpawnKind::TrickInstance {
                prototype: GraphTilePrototypeId(id),
            },
        };
        let original = item(1000, "Evening motif");
        let favorite = LibraryTileKey::new(&original.spawn, &original.label);
        assert!(favorite.matches(&item(1037, "Evening motif")));
        assert!(!favorite.matches(&item(1000, "Different motif")));
        assert!(!favorite.matches(&item(33, "Evening motif")));
    }
    #[test]
    fn invalid_collections_are_rejected() {
        let prefs = LibraryPreferences {
            favorites: (0..80).flat_map(|n| [key(n), key(n)]).collect(),
            recent: (0..20).map(key).collect(),
        };
        assert!(prefs.validate().is_err());
        let duplicates = LibraryPreferences {
            favorites: vec![key(1), key(1)],
            recent: Vec::new(),
        };
        assert!(duplicates.validate().is_err());
    }
}
