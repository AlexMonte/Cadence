//! Stable action names and validated, focus-scoped shortcuts. These preferences
//! are independent of music and never generate input events or mutate a song.
use bevy::prelude::{ButtonInput, KeyCode};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Action {
    Left,
    Right,
    Up,
    Down,
    SelectLeft,
    SelectRight,
    SelectUp,
    SelectDown,
    NextExpression,
    PreviousExpression,
    FirstExpression,
    LastExpression,
    SelectMode,
    Inspect,
    Parent,
    Undo,
    Redo,
    Copy,
    Paste,
    Duplicate,
    Group,
    Delete,
    PlayPause,
    Stop,
    Restart,
    Help,
    InsertBefore,
    InsertAfter,
    Library,
    Minimap,
    Search,
    NextRegion,
    PreviousRegion,
    Save,
    New,
    Open,
}
impl Action {
    pub const ALL: &'static [Self] = &[
        Self::PlayPause,
        Self::Stop,
        Self::Restart,
        Self::Save,
        Self::New,
        Self::Open,
        Self::Search,
        Self::Help,
        Self::NextRegion,
        Self::PreviousRegion,
        Self::Left,
        Self::Right,
        Self::Up,
        Self::Down,
        Self::SelectLeft,
        Self::SelectRight,
        Self::SelectUp,
        Self::SelectDown,
        Self::NextExpression,
        Self::PreviousExpression,
        Self::FirstExpression,
        Self::LastExpression,
        Self::Inspect,
        Self::Parent,
        Self::SelectMode,
        Self::InsertBefore,
        Self::InsertAfter,
        Self::Copy,
        Self::Paste,
        Self::Duplicate,
        Self::Group,
        Self::Delete,
        Self::Undo,
        Self::Redo,
        Self::Library,
        Self::Minimap,
    ];
    pub fn label(self) -> &'static str {
        match self {
            Self::Left => "Move left",
            Self::Right => "Move right",
            Self::Up => "Move up",
            Self::Down => "Move down",
            Self::SelectLeft => "Extend selection left",
            Self::SelectRight => "Extend selection right",
            Self::SelectUp => "Extend selection up",
            Self::SelectDown => "Extend selection down",
            Self::NextExpression => "Next expression",
            Self::PreviousExpression => "Previous expression",
            Self::FirstExpression => "First expression",
            Self::LastExpression => "Last expression",
            Self::SelectMode => "Toggle range selection",
            Self::Inspect => "Inspect / open / place",
            Self::Parent => "Containing pattern",
            Self::Undo => "Undo",
            Self::Redo => "Redo",
            Self::Copy => "Copy",
            Self::Paste => "Paste",
            Self::Duplicate => "Duplicate",
            Self::Group => "Group sequence",
            Self::Delete => "Delete",
            Self::PlayPause => "Play / pause",
            Self::Stop => "Stop",
            Self::Restart => "Return to start",
            Self::Help => "Keyboard help",
            Self::InsertBefore => "Insert notes before",
            Self::InsertAfter => "Insert notes after",
            Self::Library => "Toggle tile library",
            Self::Minimap => "Toggle minimap",
            Self::Search => "Search commands",
            Self::NextRegion => "Next region",
            Self::PreviousRegion => "Previous region",
            Self::Save => "Save project",
            Self::New => "New project",
            Self::Open => "Open project",
        }
    }
    pub fn is_region(self) -> bool {
        matches!(self, Self::NextRegion | Self::PreviousRegion)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Chord {
    pub key: KeyCode,
    pub primary: bool,
    pub shift: bool,
    pub alt: bool,
}
impl Chord {
    const fn plain(key: KeyCode) -> Self {
        Self {
            key,
            primary: false,
            shift: false,
            alt: false,
        }
    }
    const fn primary(key: KeyCode) -> Self {
        Self {
            primary: true,
            ..Self::plain(key)
        }
    }
    const fn shifted(key: KeyCode) -> Self {
        Self {
            shift: true,
            ..Self::plain(key)
        }
    }
    const fn primary_shift(key: KeyCode) -> Self {
        Self {
            shift: true,
            ..Self::primary(key)
        }
    }
    pub fn from_input(key: KeyCode, keys: &ButtonInput<KeyCode>) -> Self {
        Self {
            key,
            primary: [
                KeyCode::ControlLeft,
                KeyCode::ControlRight,
                KeyCode::SuperLeft,
                KeyCode::SuperRight,
            ]
            .iter()
            .any(|k| keys.pressed(*k)),
            shift: keys.pressed(KeyCode::ShiftLeft) || keys.pressed(KeyCode::ShiftRight),
            alt: keys.pressed(KeyCode::AltLeft) || keys.pressed(KeyCode::AltRight),
        }
    }
    pub fn label(self) -> String {
        let raw = format!("{:?}", self.key);
        let key = raw
            .strip_prefix("Key")
            .or_else(|| raw.strip_prefix("Digit"))
            .unwrap_or(&raw);
        format!(
            "{}{}{}{key}",
            if self.primary { "Cmd/Ctrl+" } else { "" },
            if self.alt { "Alt+" } else { "" },
            if self.shift { "Shift+" } else { "" }
        )
    }
    fn matches(self, keys: &ButtonInput<KeyCode>) -> bool {
        keys.just_pressed(self.key) && self == Self::from_input(self.key, keys)
    }
    fn character(self) -> bool {
        !self.primary
            && !self.alt
            && matches!(
                self.key,
                KeyCode::KeyA
                    | KeyCode::KeyB
                    | KeyCode::KeyC
                    | KeyCode::KeyD
                    | KeyCode::KeyE
                    | KeyCode::KeyF
                    | KeyCode::KeyG
                    | KeyCode::KeyH
                    | KeyCode::KeyI
                    | KeyCode::KeyJ
                    | KeyCode::KeyK
                    | KeyCode::KeyL
                    | KeyCode::KeyM
                    | KeyCode::KeyN
                    | KeyCode::KeyO
                    | KeyCode::KeyP
                    | KeyCode::KeyQ
                    | KeyCode::KeyR
                    | KeyCode::KeyS
                    | KeyCode::KeyT
                    | KeyCode::KeyU
                    | KeyCode::KeyV
                    | KeyCode::KeyW
                    | KeyCode::KeyX
                    | KeyCode::KeyY
                    | KeyCode::KeyZ
                    | KeyCode::Digit0
                    | KeyCode::Digit1
                    | KeyCode::Digit2
                    | KeyCode::Digit3
                    | KeyCode::Digit4
                    | KeyCode::Digit5
                    | KeyCode::Digit6
                    | KeyCode::Digit7
                    | KeyCode::Digit8
                    | KeyCode::Digit9
                    | KeyCode::Minus
                    | KeyCode::Equal
                    | KeyCode::BracketLeft
                    | KeyCode::BracketRight
                    | KeyCode::Backslash
                    | KeyCode::Semicolon
                    | KeyCode::Quote
                    | KeyCode::Backquote
                    | KeyCode::Comma
                    | KeyCode::Period
                    | KeyCode::Slash
                    | KeyCode::IntlBackslash
                    | KeyCode::IntlRo
                    | KeyCode::IntlYen
            )
    }
    pub fn assignable(self) -> bool {
        self.character()
            || Self::plain(self.key).character()
            || matches!(
                self.key,
                KeyCode::Space
                    | KeyCode::Enter
                    | KeyCode::Insert
                    | KeyCode::Delete
                    | KeyCode::Backspace
                    | KeyCode::Home
                    | KeyCode::End
                    | KeyCode::PageUp
                    | KeyCode::PageDown
                    | KeyCode::ArrowLeft
                    | KeyCode::ArrowRight
                    | KeyCode::ArrowUp
                    | KeyCode::ArrowDown
                    | KeyCode::F1
                    | KeyCode::F2
                    | KeyCode::F3
                    | KeyCode::F4
                    | KeyCode::F5
                    | KeyCode::F6
                    | KeyCode::F7
                    | KeyCode::F8
                    | KeyCode::F9
                    | KeyCode::F10
                    | KeyCode::F11
                    | KeyCode::F12
            )
    }
    fn function_key(self) -> bool {
        matches!(
            self.key,
            KeyCode::F1
                | KeyCode::F2
                | KeyCode::F3
                | KeyCode::F4
                | KeyCode::F5
                | KeyCode::F6
                | KeyCode::F7
                | KeyCode::F8
                | KeyCode::F9
                | KeyCode::F10
                | KeyCode::F11
                | KeyCode::F12
        )
    }
}

fn defaults(action: Action, vim: bool) -> &'static [Chord] {
    use KeyCode::*;
    match (action, vim) {
        (Action::Left, false) => const { &[Chord::plain(ArrowLeft)] },
        (Action::Left, true) => const { &[Chord::plain(ArrowLeft), Chord::plain(KeyH)] },
        (Action::Right, false) => const { &[Chord::plain(ArrowRight)] },
        (Action::Right, true) => const { &[Chord::plain(ArrowRight), Chord::plain(KeyL)] },
        (Action::Up, false) => const { &[Chord::plain(ArrowUp)] },
        (Action::Up, true) => const { &[Chord::plain(ArrowUp), Chord::plain(KeyK)] },
        (Action::Down, false) => const { &[Chord::plain(ArrowDown)] },
        (Action::Down, true) => const { &[Chord::plain(ArrowDown), Chord::plain(KeyJ)] },
        (Action::SelectLeft, _) => const { &[Chord::shifted(ArrowLeft)] },
        (Action::SelectRight, _) => const { &[Chord::shifted(ArrowRight)] },
        (Action::SelectUp, _) => const { &[Chord::shifted(ArrowUp)] },
        (Action::SelectDown, _) => const { &[Chord::shifted(ArrowDown)] },
        (Action::NextExpression, true) => const { &[Chord::plain(KeyW)] },
        (Action::PreviousExpression, true) => const { &[Chord::plain(KeyB)] },
        (Action::LastExpression, true) => const { &[Chord::shifted(KeyG)] },
        (Action::SelectMode, true) => const { &[Chord::plain(KeyV)] },
        (Action::Inspect, _) => const { &[Chord::plain(Enter)] },
        (Action::Parent, true) => const { &[Chord::plain(Minus)] },
        (Action::Undo, false) => const { &[Chord::primary(KeyZ)] },
        (Action::Undo, true) => const { &[Chord::primary(KeyZ), Chord::plain(KeyU)] },
        (Action::Redo, false) => const { &[Chord::primary_shift(KeyZ), Chord::primary(KeyY)] },
        (Action::Redo, true) => {
            const {
                &[
                    Chord::primary_shift(KeyZ),
                    Chord::primary(KeyY),
                    Chord::primary(KeyR),
                ]
            }
        }
        (Action::Copy, false) => const { &[Chord::primary(KeyC)] },
        (Action::Copy, true) => const { &[Chord::primary(KeyC), Chord::plain(KeyY)] },
        (Action::Paste, false) => const { &[Chord::primary(KeyV)] },
        (Action::Paste, true) => const { &[Chord::primary(KeyV), Chord::plain(KeyP)] },
        (Action::Duplicate, _) => const { &[Chord::primary(KeyD)] },
        (Action::Group, _) => const { &[Chord::primary(KeyG)] },
        (Action::Delete, false) => {
            const { &[Chord::plain(KeyCode::Delete), Chord::plain(Backspace)] }
        }
        (Action::Delete, true) => {
            const {
                &[
                    Chord::plain(KeyCode::Delete),
                    Chord::plain(Backspace),
                    Chord::plain(KeyX),
                ]
            }
        }
        (Action::PlayPause, _) => const { &[Chord::plain(Space)] },
        (Action::Stop, _) => const { &[Chord::shifted(Space)] },
        (Action::Restart, _) => const { &[Chord::plain(Home)] },
        (Action::Help, false) => const { &[Chord::plain(F1)] },
        (Action::Help, true) => const { &[Chord::plain(F1), Chord::shifted(Slash)] },
        (Action::InsertBefore, false) => const { &[Chord::plain(Insert)] },
        (Action::InsertBefore, true) => const { &[Chord::plain(Insert), Chord::plain(KeyI)] },
        (Action::InsertAfter, true) => const { &[Chord::plain(KeyA)] },
        (Action::Library, false) => const { &[Chord::plain(KeyD)] },
        (Action::Minimap, false) => const { &[Chord::plain(KeyM)] },
        (Action::Search, _) => const { &[Chord::primary(KeyK)] },
        (Action::NextRegion, _) => const { &[Chord::plain(F6)] },
        (Action::PreviousRegion, _) => const { &[Chord::shifted(F6)] },
        (Action::Save, _) => const { &[Chord::primary(KeyS)] },
        (Action::New, _) => const { &[Chord::primary(KeyN)] },
        (Action::Open, _) => const { &[Chord::primary(KeyO)] },
        _ => const { &[] },
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Keymap {
    pub character_shortcuts: bool,
    overrides: BTreeMap<Action, Vec<Chord>>,
}
impl Default for Keymap {
    fn default() -> Self {
        Self {
            character_shortcuts: true,
            overrides: BTreeMap::new(),
        }
    }
}
impl Keymap {
    pub fn bindings(&self, action: Action, vim: bool) -> &[Chord] {
        self.overrides
            .get(&action)
            .map(Vec::as_slice)
            .unwrap_or_else(|| defaults(action, vim))
    }
    pub fn pressed(
        &self,
        action: Action,
        vim: bool,
        keys: &ButtonInput<KeyCode>,
    ) -> Option<KeyCode> {
        self.bindings(action, vim)
            .iter()
            .filter(|c| self.character_shortcuts || !c.character())
            .find(|c| c.matches(keys))
            .map(|c| c.key)
    }
    pub fn gg_enabled(&self, vim: bool) -> bool {
        vim && self.character_shortcuts && !self.overrides.contains_key(&Action::FirstExpression)
    }
    pub fn describe(&self, action: Action, vim: bool) -> String {
        if action == Action::FirstExpression && self.gg_enabled(vim) {
            return "g then g".into();
        }
        let values: Vec<_> = self
            .bindings(action, vim)
            .iter()
            .filter(|c| self.character_shortcuts || !c.character())
            .map(|c| c.label())
            .collect();
        if values.is_empty() {
            "Unbound".into()
        } else {
            values.join(" / ")
        }
    }
    pub fn replace(&mut self, action: Action, chords: Vec<Chord>) -> Result<(), String> {
        let mut candidate = self.clone();
        candidate.overrides.insert(action, chords);
        candidate.validate()?;
        *self = candidate;
        Ok(())
    }
    pub fn reset(&mut self, action: Action) -> Result<(), String> {
        let mut candidate = self.clone();
        candidate.overrides.remove(&action);
        candidate.validate()?;
        *self = candidate;
        Ok(())
    }
    pub fn validate(&self) -> Result<(), String> {
        for (&action, bindings) in &self.overrides {
            if bindings.len() > 4 {
                return Err("Use at most four shortcuts per action".into());
            }
            for chord in bindings {
                if !chord.assignable() {
                    return Err(
                        "Escape, Tab and modifier-only keys keep their built-in roles".into(),
                    );
                }
                if action.is_region() && !chord.function_key() {
                    return Err("Region navigation uses F1–F12 so it cannot replace typing or field editing".into());
                }
                if chord.primary && matches!(chord.key, KeyCode::KeyQ | KeyCode::KeyW) {
                    return Err(
                        "Cmd/Ctrl+Q and W are reserved for window/application control".into(),
                    );
                }
                if !self.overrides.contains_key(&Action::FirstExpression)
                    && *chord == Chord::plain(KeyCode::KeyG)
                {
                    return Err("G starts the Vim gg motion; assign First expression before using G elsewhere".into());
                }
                if chord.character()
                    && !chord.shift
                    && matches!(
                        chord.key,
                        KeyCode::Digit0
                            | KeyCode::Digit1
                            | KeyCode::Digit2
                            | KeyCode::Digit3
                            | KeyCode::Digit4
                            | KeyCode::Digit5
                            | KeyCode::Digit6
                            | KeyCode::Digit7
                            | KeyCode::Digit8
                            | KeyCode::Digit9
                    )
                {
                    return Err("Unmodified digits are reserved for Vim motion counts".into());
                }
            }
        }
        for vim in [false, true] {
            let mut used: Vec<(Chord, Action)> = Vec::new();
            for &action in Action::ALL {
                for &chord in self.bindings(action, vim) {
                    if let Some((_, other)) = used.iter().find(|(key, _)| *key == chord) {
                        return Err(format!(
                            "{} conflicts with {} in the {} profile",
                            chord.label(),
                            other.label(),
                            if vim { "Vim" } else { "Standard" }
                        ));
                    }
                    used.push((chord, action));
                }
            }
        }
        Ok(())
    }
    pub fn export(&self) -> Result<String, String> {
        self.validate()?;
        serde_json::to_string_pretty(self).map_err(|e| e.to_string())
    }
    pub fn import(text: &str) -> Result<Self, String> {
        if text.len() > 32_768 {
            return Err("Shortcut settings must be smaller than 32 KiB".into());
        }
        let value: Self = serde_json::from_str(text).map_err(|e| e.to_string())?;
        value.validate()?;
        Ok(value)
    }
}

impl super::EditorPreferences {
    pub fn shortcut(&self, action: Action, keys: &ButtonInput<KeyCode>) -> Option<KeyCode> {
        self.keymap.pressed(action, self.vim_navigation, keys)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn default_profiles_are_unambiguous_and_exact_modifiers_distinguish_stop() {
        let map = Keymap::default();
        map.validate().unwrap();
        let mut keys = ButtonInput::default();
        keys.press(KeyCode::Space);
        assert_eq!(
            map.pressed(Action::PlayPause, false, &keys),
            Some(KeyCode::Space)
        );
        keys.press(KeyCode::ShiftLeft);
        assert_eq!(map.pressed(Action::PlayPause, false, &keys), None);
        assert_eq!(
            map.pressed(Action::Stop, false, &keys),
            Some(KeyCode::Space)
        );
        keys.press(KeyCode::AltLeft);
        assert_eq!(map.pressed(Action::Stop, false, &keys), None);
    }
    #[test]
    fn conflicts_across_inactive_profile_and_invalid_keys_leave_map_unchanged() {
        let mut map = Keymap::default();
        for (action, chord) in [
            (Action::PlayPause, Chord::plain(KeyCode::KeyH)),
            (Action::Stop, Chord::plain(KeyCode::Space)),
            (Action::NextRegion, Chord::primary(KeyCode::KeyB)),
            (Action::PlayPause, Chord::plain(KeyCode::Escape)),
            (Action::PlayPause, Chord::primary(KeyCode::KeyW)),
            (Action::Stop, Chord::plain(KeyCode::KeyG)),
        ] {
            let old = map.clone();
            assert!(map.replace(action, vec![chord]).is_err());
            assert_eq!(map, old);
        }
        map.replace(Action::PlayPause, vec![Chord::plain(KeyCode::F8)])
            .unwrap();
        map.replace(Action::Stop, vec![Chord::plain(KeyCode::Space)])
            .unwrap();
        let old = map.clone();
        assert!(map.reset(Action::PlayPause).is_err());
        assert_eq!(map, old);
    }
    #[test]
    fn remapping_replaces_aliases_and_disabling_characters_keeps_modified_commands() {
        let mut map = Keymap::default();
        map.replace(Action::Right, vec![Chord::plain(KeyCode::F8)])
            .unwrap();
        let mut keys = ButtonInput::default();
        keys.press(KeyCode::KeyL);
        keys.press(KeyCode::ArrowRight);
        assert_eq!(map.pressed(Action::Right, true, &keys), None);
        keys.press(KeyCode::F8);
        assert_eq!(map.pressed(Action::Right, true, &keys), Some(KeyCode::F8));
        map.character_shortcuts = false;
        keys.reset_all();
        keys.press(KeyCode::KeyU);
        assert_eq!(map.pressed(Action::Undo, true, &keys), None);
        assert!(!map.gg_enabled(true));
        keys.reset_all();
        keys.press(KeyCode::SuperLeft);
        keys.press(KeyCode::KeyZ);
        assert_eq!(map.pressed(Action::Undo, true, &keys), Some(KeyCode::KeyZ));
    }
    #[test]
    fn exported_keymaps_round_trip_and_invalid_imports_cannot_activate() {
        let mut map = Keymap::default();
        map.replace(Action::FirstExpression, vec![]).unwrap();
        map.replace(Action::PlayPause, vec![Chord::plain(KeyCode::KeyG)])
            .unwrap();
        assert!(!map.gg_enabled(true));
        assert_eq!(Keymap::import(&map.export().unwrap()).unwrap(), map);
        let bad = r#"{"overrides":{"play_pause":[{"key":"F6"}]}}"#;
        assert!(Keymap::import(bad).is_err());
        assert!(Keymap::import(&" ".repeat(32_769)).is_err());
        assert!(Keymap::import(r#"{"overrides":{"nonexistent":[]}}"#).is_err());
    }
}
