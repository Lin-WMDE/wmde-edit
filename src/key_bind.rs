// SPDX-License-Identifier: GPL-3.0-only

use cosmic::shortcuts::{Binding, ModifierName, ShortcutAction};
use cosmic::widget::menu::key_bind::KeyBind;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::Action;

// WMDE: the bindings go through cosmic::shortcuts, shared with the rest of the stack: the
// shipped map below, someone's own `shortcuts_custom` from the notepad's config laid over it,
// and the declaration in res/app-shortcuts/fun.wmde.Edit.ron that the settings application
// builds the notepad's page from. The tests at the bottom keep the declaration and the code
// saying the same thing.

/// The stored map: what goes into the `shortcuts_custom` config key.
pub type Shortcuts = cosmic::shortcuts::Shortcuts<KeyBindAction>;

/// An action a key combination can be bound to: the menu actions that make sense on a key, and
/// `Disable`, which takes a shipped binding away instead of running something.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub enum KeyBindAction {
    Disable,
    NewFile,
    NewWindow,
    OpenFileDialog,
    Save,
    SaveAsDialog,
    CloseFile,
    Quit,
    Undo,
    Redo,
    Cut,
    Copy,
    Paste,
    SelectAll,
    Find,
    FindAndReplace,
    ZoomIn,
    ZoomOut,
    ZoomReset,
    ToggleWordWrap,
    ToggleSettingsPage,
    TabNext,
    TabPrev,
    TabActivate0,
    TabActivate1,
    TabActivate2,
    TabActivate3,
    TabActivate4,
    TabActivate5,
    TabActivate6,
    TabActivate7,
    TabActivate8,
}

impl ShortcutAction for KeyBindAction {
    fn is_disable(&self) -> bool {
        matches!(self, Self::Disable)
    }
}

impl KeyBindAction {
    /// The menu action a binding runs, which is also what the menu labels its key with; none for
    /// `Disable`.
    #[must_use]
    pub const fn action(self) -> Option<Action> {
        Some(match self {
            Self::Disable => return None,
            Self::NewFile => Action::NewFile,
            Self::NewWindow => Action::NewWindow,
            Self::OpenFileDialog => Action::OpenFileDialog,
            Self::Save => Action::Save,
            Self::SaveAsDialog => Action::SaveAsDialog,
            Self::CloseFile => Action::CloseFile,
            Self::Quit => Action::Quit,
            Self::Undo => Action::Undo,
            Self::Redo => Action::Redo,
            Self::Cut => Action::Cut,
            Self::Copy => Action::Copy,
            Self::Paste => Action::Paste,
            Self::SelectAll => Action::SelectAll,
            Self::Find => Action::Find,
            Self::FindAndReplace => Action::FindAndReplace,
            Self::ZoomIn => Action::ZoomIn,
            Self::ZoomOut => Action::ZoomOut,
            Self::ZoomReset => Action::ZoomReset,
            Self::ToggleWordWrap => Action::ToggleWordWrap,
            Self::ToggleSettingsPage => Action::ToggleSettingsPage,
            Self::TabNext => Action::TabNext,
            Self::TabPrev => Action::TabPrev,
            Self::TabActivate0 => Action::TabActivate0,
            Self::TabActivate1 => Action::TabActivate1,
            Self::TabActivate2 => Action::TabActivate2,
            Self::TabActivate3 => Action::TabActivate3,
            Self::TabActivate4 => Action::TabActivate4,
            Self::TabActivate5 => Action::TabActivate5,
            Self::TabActivate6 => Action::TabActivate6,
            Self::TabActivate7 => Action::TabActivate7,
            Self::TabActivate8 => Action::TabActivate8,
        })
    }
}

/// The bindings the notepad ships: upstream's, plus Ctrl+Y for redo as in Notepad. Latin
/// letters are written upper case, as a captured key is spelled, so rebinding one replaces
/// this very entry instead of settling beside it.
#[must_use]
pub fn fallback_shortcuts() -> Shortcuts {
    use KeyBindAction::{
        CloseFile, Copy, Cut, Find, FindAndReplace, NewFile, NewWindow, OpenFileDialog, Paste,
        Quit, Redo, Save, SaveAsDialog, SelectAll, TabActivate0, TabActivate1, TabActivate2,
        TabActivate3, TabActivate4, TabActivate5, TabActivate6, TabActivate7, TabActivate8,
        TabNext, TabPrev, ToggleSettingsPage, ToggleWordWrap, Undo, ZoomIn, ZoomOut, ZoomReset,
    };
    use ModifierName::{Alt, Ctrl, Shift};

    [
        (vec![Ctrl], "W", CloseFile),
        (vec![Ctrl], "X", Cut),
        (vec![Ctrl], "C", Copy),
        (vec![Ctrl], "F", Find),
        (vec![Ctrl], "H", FindAndReplace),
        (vec![Ctrl], "V", Paste),
        (vec![Shift], "Insert", Paste),
        (vec![Ctrl], "Insert", Copy),
        (vec![Shift], "Delete", Cut),
        (vec![Ctrl], "T", NewFile),
        (vec![Ctrl], "N", NewWindow),
        (vec![Ctrl], "O", OpenFileDialog),
        (vec![Ctrl], "Q", Quit),
        (vec![Ctrl, Shift], "Z", Redo),
        (vec![Ctrl], "Y", Redo),
        (vec![Ctrl], "S", Save),
        (vec![Ctrl, Shift], "S", SaveAsDialog),
        (vec![Ctrl], "A", SelectAll),
        (vec![Ctrl], "0", ZoomReset),
        (vec![Ctrl], "-", ZoomOut),
        (vec![Ctrl], "=", ZoomIn),
        (vec![Ctrl], "+", ZoomIn),
        (vec![Ctrl], "1", TabActivate0),
        (vec![Ctrl], "2", TabActivate1),
        (vec![Ctrl], "3", TabActivate2),
        (vec![Ctrl], "4", TabActivate3),
        (vec![Ctrl], "5", TabActivate4),
        (vec![Ctrl], "6", TabActivate5),
        (vec![Ctrl], "7", TabActivate6),
        (vec![Ctrl], "8", TabActivate7),
        (vec![Ctrl], "9", TabActivate8),
        (vec![Ctrl], "Tab", TabNext),
        (vec![Ctrl, Shift], "Tab", TabPrev),
        (vec![Ctrl], ",", ToggleSettingsPage),
        (vec![Alt], "Z", ToggleWordWrap),
        (vec![Ctrl], "Z", Undo),
    ]
    .into_iter()
    .map(|(modifiers, key, action)| (Binding::new(modifiers, key), action))
    .collect()
}

/// The bindings the notepad matches against and the menu labels its items with: the shipped
/// map with `custom` laid over it.
#[must_use]
pub fn key_binds(custom: &Shortcuts) -> HashMap<KeyBind, Action> {
    cosmic::shortcuts::ShortcutsConfig::new(fallback_shortcuts(), custom.clone())
        .key_binds(|action| action.action())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{CONFIG_VERSION, Config};

    fn bound(binds: &HashMap<KeyBind, Action>, keys: &[ModifierName], key: &str) -> Option<Action> {
        binds
            .get(&Binding::new(keys.to_vec(), key).to_key_bind().unwrap())
            .copied()
    }

    #[test]
    fn someone_s_own_binding_disables_and_adds() {
        let mut custom = Shortcuts::new();
        custom.0.insert(
            Binding::new([ModifierName::Ctrl], "Q"),
            KeyBindAction::Disable,
        );
        custom
            .0
            .insert(Binding::new([ModifierName::Ctrl], "D"), KeyBindAction::Find);
        let binds = key_binds(&custom);
        assert_eq!(bound(&binds, &[ModifierName::Ctrl], "Q"), None);
        assert_eq!(
            bound(&binds, &[ModifierName::Ctrl], "D"),
            Some(Action::Find)
        );
        assert_eq!(
            bound(&binds, &[ModifierName::Ctrl], "F"),
            Some(Action::Find)
        );
    }

    #[test]
    fn every_default_key_is_spelled_the_way_capture_spells_it() {
        // A default spelled otherwise could not be replaced by a captured binding: the two would
        // sit side by side in the map and both fire.
        for (binding, action) in fallback_shortcuts().iter() {
            let key = cosmic::shortcuts::key_from_string(&binding.key)
                .unwrap_or_else(|| panic!("{action:?}: key {:?} does not resolve", binding.key));
            let spelled = cosmic::shortcuts::key_to_string(&key)
                .unwrap_or_else(|| panic!("{action:?}: key {:?} has no spelling", binding.key));
            assert_eq!(binding.key, spelled, "{action:?}: captured as {spelled:?}");
        }
    }

    #[test]
    fn every_default_binding_runs_something() {
        for action in fallback_shortcuts().0.values() {
            assert!(
                action.action().is_some(),
                "{action:?} is bound but runs nothing"
            );
        }
    }

    #[test]
    fn the_declaration_matches_the_code() {
        #[derive(serde::Deserialize)]
        struct Declaration {
            config: Target,
            groups: Vec<Group>,
        }
        #[derive(serde::Deserialize)]
        struct Target {
            id: String,
            version: u64,
            key: String,
        }
        #[derive(serde::Deserialize)]
        struct Group {
            actions: Vec<Declared>,
        }
        #[derive(serde::Deserialize)]
        struct Declared {
            // Quoted in the file: the settings application reads it as text.
            action: String,
            // Named so RON has somewhere to put it.
            #[allow(dead_code)]
            label: HashMap<String, String>,
            #[serde(default)]
            defaults: Vec<Binding>,
        }

        let path = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/res/app-shortcuts/fun.wmde.Edit.ron"
        );
        let text = std::fs::read_to_string(path).expect("the declaration has to ship");
        let declared: Declaration = ron::from_str(&text).expect("the declaration has to parse");

        // Where the settings application writes is where the notepad reads.
        assert_eq!(
            declared.config.id,
            <crate::App as cosmic::Application>::APP_ID
        );
        assert_eq!(declared.config.version, CONFIG_VERSION);
        assert_eq!(declared.config.key, "shortcuts_custom");
        // The key really is a field of the notepad's config.
        let _ = Config::default().shortcuts_custom;

        let mut from_declaration = Shortcuts::new();
        for entry in declared.groups.iter().flat_map(|group| &group.actions) {
            let action: KeyBindAction = ron::from_str(&entry.action)
                .unwrap_or_else(|_| panic!("{} is not an action", entry.action));
            for binding in &entry.defaults {
                from_declaration.0.insert(binding.clone(), action);
            }
        }
        assert_eq!(
            from_declaration,
            fallback_shortcuts(),
            "the shortcuts declaration and the code disagree"
        );
    }
}
