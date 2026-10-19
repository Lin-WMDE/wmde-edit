// SPDX-License-Identifier: GPL-3.0-only

use cosmic::{
    cosmic_config::{self, CosmicConfigEntry, cosmic_config_derive::CosmicConfigEntry},
    theme,
};
use cosmic_text::Metrics;
use serde::{Deserialize, Serialize};
use std::{collections::VecDeque, path::PathBuf};

pub const CONFIG_VERSION: u64 = 1;

#[derive(Clone, CosmicConfigEntry, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Config {
    pub auto_indent: bool,
    pub find_case_sensitive: bool,
    pub find_use_regex: bool,
    pub find_wrap_around: bool,
    pub font_name: String,
    pub font_size: u16,
    pub font_size_zoom_step_mul_100: u16,
    pub highlight_current_line: bool,
    pub line_numbers: bool,
    /// WMDE: someone's own key bindings over the shipped ones, written by the settings
    /// application (res/app-shortcuts/fun.wmde.Edit.ron).
    pub shortcuts_custom: crate::key_bind::Shortcuts,
    /// WMDE: the status bar under the text.
    pub status_bar: bool,
    pub syntax_theme_dark: String,
    pub syntax_theme_light: String,
    pub tab_width: u16,
    pub vim_bindings: bool,
    pub word_wrap: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            auto_indent: true,
            find_case_sensitive: false,
            find_use_regex: false,
            find_wrap_around: true,
            // WMDE: the desktop's monospace family rather than a name written into the app,
            // and the 12 px of `monotext` (.doc/appearance/02-typography.md).
            font_name: cosmic::config::monospace_font().family,
            font_size: 12,
            font_size_zoom_step_mul_100: 100,
            highlight_current_line: true,
            line_numbers: true,
            shortcuts_custom: crate::key_bind::Shortcuts::new(),
            status_bar: true,
            syntax_theme_dark: "WMDE Dark".to_string(),
            syntax_theme_light: "WMDE Light".to_string(),
            tab_width: 4,
            vim_bindings: false,
            word_wrap: true,
        }
    }
}

impl Config {
    pub fn font_size_adjusted(&self, zoom_adj: i8) -> f32 {
        let font_size = f32::from(self.font_size).max(1.0);
        let adj = f32::from(zoom_adj);
        let adj_step = f32::from(self.font_size_zoom_step_mul_100) / 100.0;
        (font_size + adj * adj_step).max(1.0)
    }

    pub fn find_regex(&self, pattern: &str) -> Result<regex::Regex, regex::Error> {
        let mut builder = if self.find_use_regex {
            regex::RegexBuilder::new(pattern)
        } else {
            regex::RegexBuilder::new(&regex::escape(pattern))
        };
        builder.case_insensitive(!self.find_case_sensitive);
        builder.build()
    }

    // Calculate metrics from font size
    pub fn metrics(&self, zoom_adj: i8) -> Metrics {
        let font_size = self.font_size_adjusted(zoom_adj);
        let line_height = (font_size * 1.4).ceil();
        Metrics::new(font_size, line_height)
    }

    // Get current syntax theme based on dark mode
    pub fn syntax_theme(&self) -> &str {
        // WMDE: the window follows the desktop theme; there is no per-app override.
        // Read the system preference rather than the global theme: the latter may not be
        // updated yet when the theme mode subscription fires.
        let dark = theme::system_preference().theme_type.is_dark();
        if dark {
            &self.syntax_theme_dark
        } else {
            &self.syntax_theme_light
        }
    }
}

#[derive(Clone, CosmicConfigEntry, Debug, Deserialize, Eq, PartialEq, Serialize, Default)]
pub struct ConfigState {
    /// WMDE: the filter last picked in the Open dialog, an index into the list the dialog
    /// gets; 0, text documents, until someone picks another.
    pub open_filter: usize,
    pub recent_files: VecDeque<PathBuf>,
}
