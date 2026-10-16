// SPDX-License-Identifier: GPL-3.0-only

use cosmic::{iced::advanced::graphics::text::font_system, widget::icon};
use cosmic_files::mime_icon::{FALLBACK_MIME_ICON, mime_for_path, mime_icon};
use cosmic_text::{Attrs, Buffer, Cursor, Edit, Selection, Shaping, SyntaxEditor, ViEditor, Wrap};
use regex::Regex;
use std::{
    fmt, fs,
    io::{self, Write},
    path::{self, Path, PathBuf},
    process::{Command, Stdio},
    sync::{Arc, Mutex},
};

use crate::{
    Config, SYNTAX_SYSTEM,
    encoding::{self, DecodeError, Decoded, LineEnding, TextEncoding},
    fl,
};

fn editor_text(editor: &ViEditor<'static, 'static>) -> String {
    editor.with_buffer(|buffer| {
        let mut text = String::new();
        for line in buffer.lines.iter() {
            text.push_str(line.text());
            text.push_str(line.ending().as_str());
        }
        text
    })
}

/// WMDE: writes `bytes` to `path` as root through `pkexec tee`, for files the user may not
/// write. The error is a short sentence for the save error dialog.
fn save_elevated(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let mut child = Command::new("pkexec")
        .arg("tee")
        .arg(path)
        .stdin(Stdio::piped())
        .stdout(Stdio::null()) // Redirect stdout to /dev/null
        .stderr(Stdio::inherit()) // Retain stderr for error visibility
        .spawn()
        .map_err(|err| {
            log::error!("Failed to spawn pkexec process: {}", err);
            format!("pkexec: {err}")
        })?;
    if let Some(mut stdin) = child.stdin.take() {
        if let Err(err) = stdin.write_all(bytes) {
            log::error!("Failed to write to stdin: {}", err);
        }
    } else {
        log::error!("Failed to access stdin of pkexec process.");
    }
    // Ensure the child process is reaped
    match child.wait() {
        Ok(status) if status.success() => Ok(()),
        Ok(status) => {
            log::error!("pkexec process exited with a non-zero status: {:?}", status);
            Err(format!("pkexec: {status}"))
        }
        Err(err) => {
            log::error!("Failed to wait on pkexec process: {}", err);
            Err(format!("pkexec: {err}"))
        }
    }
}

// WMDE: a single variant since git diff tabs were dropped; kept as an enum so the many
// `Tab::Editor` matches stay as upstream writes them.
pub enum Tab {
    Editor(EditorTab),
}

impl Tab {
    pub fn title(&self) -> String {
        match self {
            Self::Editor(tab) => tab.title(),
        }
    }
}

/// WMDE: why a file did not open. Shown to the user, so it reads as a sentence.
#[derive(Debug)]
pub enum OpenError {
    Io(io::Error),
    Binary,
}

impl fmt::Display for OpenError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(err) => write!(f, "{err}"),
            Self::Binary => f.write_str(&fl!("binary-file")),
        }
    }
}

/// WMDE: why a document was not saved.
#[derive(Debug)]
pub enum SaveError {
    /// The document's encoding has no code for some of its characters.
    Unmappable(TextEncoding),
    Io(io::Error),
    /// Writing through pkexec failed or was cancelled.
    Elevated(String),
}

impl fmt::Display for SaveError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unmappable(encoding) => {
                f.write_str(&fl!("unmappable-characters", encoding = encoding.name()))
            }
            Self::Io(err) => write!(f, "{err}"),
            Self::Elevated(message) => f.write_str(message),
        }
    }
}

pub struct EditorTab {
    pub path_opt: Option<PathBuf>,
    attrs: Attrs<'static>,
    pub editor: Mutex<ViEditor<'static, 'static>>,
    pub context_menu_open: bool,
    pub zoom_adj: i8,
    /// WMDE: what the file is written back as.
    pub encoding: TextEncoding,
    /// WMDE: every line break is written as this on save.
    pub line_ending: LineEnding,
}

impl EditorTab {
    pub fn new(config: &Config) -> Self {
        let attrs = crate::monospace_attrs();
        let zoom_adj = Default::default();
        let mut buffer = Buffer::new_empty(config.metrics(zoom_adj));
        // Set a minimal size before it is updated by draw
        buffer.set_size(Some(0.0), Some(0.0));
        buffer.set_text("", &attrs, Shaping::Advanced, None);

        let editor = SyntaxEditor::new(
            Arc::new(buffer),
            SYNTAX_SYSTEM.get().unwrap(),
            config.syntax_theme(),
        )
        .unwrap();

        let mut tab = Self {
            path_opt: None,
            attrs,
            editor: Mutex::new(ViEditor::new(editor)),
            context_menu_open: false,
            zoom_adj,
            encoding: TextEncoding::Utf8,
            line_ending: LineEnding::default(),
        };

        // Update any other config settings
        tab.set_config(config);

        tab
    }

    pub fn set_config(&mut self, config: &Config) {
        let mut editor = self.editor.lock().unwrap();
        let mut font_system = font_system().write().unwrap();
        let mut editor = editor.borrow_with(font_system.raw());
        editor.set_auto_indent(config.auto_indent);
        editor.set_passthrough(!config.vim_bindings);
        editor.set_tab_width(config.tab_width);
        editor.with_buffer_mut(|buffer| {
            buffer.set_wrap(if config.word_wrap {
                Wrap::WordOrGlyph
            } else {
                Wrap::None
            })
        });
        //TODO: dynamically discover light/dark changes
        editor.update_theme(config.syntax_theme());
    }

    /// Opens `path` into this tab. A file that does not exist yet opens as a new document
    /// under that name, as upstream does.
    pub fn open(&mut self, path: PathBuf) -> Result<(), OpenError> {
        let absolute = match fs::canonicalize(&path) {
            Ok(ok) => ok,
            Err(err) => match path::absolute(&path) {
                Ok(ok) => ok,
                Err(_) => {
                    log::error!("failed to canonicalize {:?}: {}", path, err);
                    path
                }
            },
        };
        let bytes = match fs::read(&absolute) {
            Ok(bytes) => bytes,
            Err(err) if err.kind() == io::ErrorKind::NotFound => {
                log::warn!("opened non-existant file {:?}", absolute);
                let mut editor = self.editor.lock().unwrap();
                let mut font_system = font_system().write().unwrap();
                let mut editor = editor.borrow_with(font_system.raw());
                // Picks the syntax from the name; the read itself fails, as expected.
                let _ = editor.load_text(&absolute, self.attrs.clone());
                editor.set_changed(true);
                self.path_opt = Some(absolute);
                return Ok(());
            }
            Err(err) => {
                log::error!("failed to open {:?}: {}", absolute, err);
                return Err(OpenError::Io(err));
            }
        };
        let decoded = match encoding::decode(&bytes, None) {
            Ok(decoded) => decoded,
            Err(DecodeError::Binary) => {
                log::warn!("not opening {:?}: binary file", absolute);
                return Err(OpenError::Binary);
            }
        };
        self.load_decoded(&absolute, decoded);
        log::info!("opened {:?} as {}", absolute, self.encoding);
        self.path_opt = Some(absolute);
        Ok(())
    }

    /// Puts a freshly decoded file into the editor.
    fn load_decoded(&mut self, path: &Path, decoded: Decoded) {
        if decoded.had_errors {
            log::warn!(
                "{:?} has bytes that are not {}; they show as U+FFFD",
                path,
                decoded.encoding
            );
        }
        let mut editor = self.editor.lock().unwrap();
        let mut font_system = font_system().write().unwrap();
        let mut editor = editor.borrow_with(font_system.raw());
        // Upstream's loader picks the syntax from the file name and first line, then reads
        // the file as UTF-8. That read already gives the right text for plain UTF-8; for
        // anything else it fails or keeps the BOM, and the decoded text replaces it.
        let loaded = editor.load_text(path, self.attrs.clone()).is_ok();
        if !loaded || decoded.encoding != TextEncoding::Utf8 {
            let attrs = self.attrs.clone().color(editor.foreground_color());
            editor.with_buffer_mut(|buffer| {
                buffer.set_text(&decoded.text, &attrs, Shaping::Advanced, None);
            });
        }
        self.encoding = decoded.encoding;
        self.line_ending = decoded.line_ending;
    }

    /// Reads the file again, keeping the change undoable. `forced` is the encoding the user
    /// chose to reopen the file in; without it the tab keeps its encoding unless the new
    /// bytes say otherwise.
    pub fn reload(&mut self, forced: Option<TextEncoding>) -> Result<(), OpenError> {
        let Some(path) = self.path_opt.clone() else {
            log::warn!("tried to reload with no path");
            return Ok(());
        };
        let bytes = fs::read(&path).map_err(|err| {
            log::error!("failed to reload {:?}: {}", path, err);
            OpenError::Io(err)
        })?;
        let encoding = match forced {
            Some(encoding) => encoding,
            None => encoding::reload_encoding(&bytes, self.encoding).map_err(|_| {
                log::warn!("not reloading {:?}: binary file", path);
                OpenError::Binary
            })?,
        };
        let decoded = encoding::decode(&bytes, Some(encoding)).map_err(|_| OpenError::Binary)?;
        if decoded.had_errors {
            log::warn!("{:?} has bytes that are not {}", path, encoding);
        }
        let switched = encoding != self.encoding;
        self.encoding = decoded.encoding;
        self.line_ending = decoded.line_ending;
        let file_content = decoded.text;

        let mut editor = self.editor.lock().unwrap();
        let mut font_system = font_system().write().unwrap();
        let mut editor = editor.borrow_with(font_system.raw());
        // Save scroll
        let scroll = editor.with_buffer(|buffer| buffer.scroll());
        //TODO: save/restore more?

        log::info!("reloaded {:?} as {}", path, encoding);

        //TODO: compare using line iterator to prevent allocations
        if file_content == editor_text(&editor) {
            log::info!("text not changed");
            if !switched {
                // WMDE: the text is what is on disk, so the document is not modified -
                // Revert all changes on a tab flagged as changed must clear the flag.
                editor.set_changed(false);
                return Ok(());
            }
        } else {
            // Store the entire operation as a single change for undo
            editor.start_change();

            // Grab everything in the buffer
            let cursor_start: Cursor = cosmic_text::Cursor::new(0, 0);
            let cursor_end = editor.with_buffer(|buffer| {
                let last_line = buffer.lines.len().saturating_sub(1);
                cosmic_text::Cursor::new(
                    last_line,
                    buffer
                        .lines
                        .get(last_line)
                        .map(|line| line.text().len())
                        .unwrap_or(0),
                )
            });

            // Replace everything in the buffer with the content from disk
            editor.delete_range(cursor_start, cursor_end);
            editor.insert_at(cursor_start, &file_content, None);

            // Adjust cursor to closest position
            let mut cursor = editor.cursor();
            editor.with_buffer(|buffer| {
                cursor.line = cursor.line.min(buffer.lines.len().saturating_sub(1));
                cursor.index = if let Some(line) = buffer.lines.get(cursor.line) {
                    let mut closest = line.text().len();
                    for (i, _) in line.text().char_indices().rev() {
                        if i >= cursor.index {
                            closest = i;
                        } else {
                            // i < cursor.index
                            if cursor.index - i < closest - cursor.index {
                                closest = i;
                            }
                            break;
                        }
                    }
                    closest
                } else {
                    0
                }
            });
            editor.set_cursor(cursor);

            editor.finish_change();
        }
        editor.set_changed(false);

        // Restore scroll
        editor.with_buffer_mut(|buffer| buffer.set_scroll(scroll));
        Ok(())
    }

    /// Writes the document in its encoding and line ending. A file the user may not write
    /// is written through pkexec, as upstream does.
    pub fn save(&mut self) -> Result<(), SaveError> {
        let Some(path) = &self.path_opt else {
            log::warn!("tab has no path yet");
            return Ok(());
        };
        let mut editor = self.editor.lock().unwrap();
        let text = encoding::normalize_line_endings(&editor_text(&editor), self.line_ending);
        let bytes = encoding::encode(&text, self.encoding).map_err(|_| {
            log::warn!("{:?} does not fit in {}", path, self.encoding);
            SaveError::Unmappable(self.encoding)
        })?;
        match fs::write(path, &bytes) {
            Ok(()) => {
                editor.save_point();
                log::info!("saved {:?} as {}", path, self.encoding);
                Ok(())
            }
            Err(err) if err.kind() == io::ErrorKind::PermissionDenied => {
                log::warn!("Permission denied. Attempting to save with pkexec.");
                save_elevated(path, &bytes).map_err(SaveError::Elevated)?;
                editor.save_point();
                log::info!("File saved successfully with pkexec.");
                Ok(())
            }
            Err(err) => {
                log::error!("failed to save {:?}: {}", path, err);
                Err(SaveError::Io(err))
            }
        }
    }

    pub fn changed(&self) -> bool {
        let editor = self.editor.lock().unwrap();
        editor.changed()
    }

    pub fn icon(&self, size: u16) -> icon::Icon {
        match &self.path_opt {
            Some(path) => icon::icon(mime_icon(mime_for_path(path, None, false), size)).size(size),
            None => icon::from_name(FALLBACK_MIME_ICON).size(size).icon(),
        }
    }

    pub fn title(&self) -> String {
        //TODO: show full title when there is a conflict
        if let Some(path) = &self.path_opt {
            match path.file_name() {
                Some(file_name_os) => match file_name_os.to_str() {
                    Some(file_name) => match file_name {
                        "mod.rs" => title_with_parent(path, file_name),
                        _ => file_name.to_string(),
                    },
                    None => format!("{}", path.display()),
                },
                None => format!("{}", path.display()),
            }
        } else {
            fl!("new-document")
        }
    }

    pub fn replace(&self, regex: &Regex, replace: &str, wrap_around: bool) -> bool {
        let mut editor = self.editor.lock().unwrap();
        let mut cursor = editor.cursor();
        let mut wrapped = false; // Keeps track of whether the search has wrapped around yet.
        let start_line = cursor.line;
        while cursor.line < editor.with_buffer(|buffer| buffer.lines.len()) {
            if let Some((index, len)) = editor.with_buffer(|buffer| {
                regex
                    .find_iter(buffer.lines[cursor.line].text())
                    .filter_map(|m| {
                        if cursor.line != start_line
                            || m.start() >= cursor.index
                            || m.start() < cursor.index && wrapped
                        {
                            Some((m.start(), m.len()))
                        } else {
                            None
                        }
                    })
                    .next()
            }) {
                cursor.index = index;
                let mut end = cursor;
                end.index = index + len;

                editor.start_change();
                // if index = 0 and len = 0, we are targeting and deleting an empty line
                // we'll move either cursor or end to delete the newline
                if index == 0 && len == 0 {
                    if cursor.line > 0 {
                        // move the cursor up one line
                        cursor.line -= 1;
                        cursor.index =
                            editor.with_buffer(|buffer| buffer.lines[cursor.line].text().len());
                    } else if cursor.line + 1 < editor.with_buffer(|buffer| buffer.lines.len()) {
                        // move the end down one line
                        end.line += 1;
                        end.index = 0;
                    }
                }
                editor.delete_range(cursor, end);
                cursor = editor.insert_at(cursor, replace, None);
                editor.set_cursor(cursor);
                // Need to disable selection to prevent the new cursor showing selection to old location
                editor.set_selection(Selection::None);
                editor.finish_change();
                return true;
            }

            cursor.line += 1;

            // If we haven't wrapped yet and we've reached the last line, reset cursor line to 0 and
            // set wrapped to true so we don't wrap again
            if wrap_around
                && !wrapped
                && cursor.line == editor.with_buffer(|buffer| buffer.lines.len())
            {
                cursor.line = 0;
                wrapped = true;
            }
        }
        false
    }

    pub fn zoom_adj(&self) -> i8 {
        self.zoom_adj
    }

    pub fn set_zoom_adj(&mut self, value: i8) {
        self.zoom_adj = value;
    }

    // Code adapted from cosmic-text ViEditor search
    pub fn search(&self, regex: &Regex, forwards: bool, wrap_around: bool) -> bool {
        let mut editor = self.editor.lock().unwrap();
        let mut cursor = editor.cursor();
        let mut wrapped = false; // Keeps track of whether the search has wrapped around yet.
        let start_line = cursor.line;
        let current_selection = editor.selection();

        if forwards {
            while cursor.line < editor.with_buffer(|buffer| buffer.lines.len()) {
                if let Some((start, end)) = editor.with_buffer(|buffer| {
                    regex
                        .find_iter(buffer.lines[cursor.line].text())
                        .filter_map(|m| {
                            if cursor.line != start_line
                                || m.start() > cursor.index
                                || m.start() == cursor.index && current_selection == Selection::None
                                || m.start() < cursor.index && wrapped
                            {
                                Some((m.start(), m.end()))
                            } else {
                                None
                            }
                        })
                        .next()
                }) {
                    cursor.index = start;
                    editor.set_cursor(cursor);

                    // Highlight searched text
                    let selection = Selection::Normal(Cursor::new(cursor.line, end));
                    editor.set_selection(selection);

                    return true;
                }

                cursor.line += 1;

                // If we haven't wrapped yet and we've reached the last line, reset cursor line to 0 and
                // set wrapped to true so we don't wrap again
                if wrap_around
                    && !wrapped
                    && cursor.line == editor.with_buffer(|buffer| buffer.lines.len())
                {
                    cursor.line = 0;
                    wrapped = true;
                }
            }
        } else {
            cursor.line += 1;
            while cursor.line > 0 {
                cursor.line -= 1;

                if let Some((start, end)) = editor.with_buffer(|buffer| {
                    regex
                        .find_iter(buffer.lines[cursor.line].text())
                        .filter_map(|m| {
                            if cursor.line != start_line
                                || m.start() < cursor.index
                                || m.start() == cursor.index && current_selection == Selection::None
                                || m.start() > cursor.index && wrapped
                            {
                                Some((m.start(), m.end()))
                            } else {
                                None
                            }
                        })
                        .last()
                }) {
                    cursor.index = start;
                    editor.set_cursor(cursor);

                    // Highlight searched text
                    let selection = Selection::Normal(Cursor::new(cursor.line, end));
                    editor.set_selection(selection);

                    return true;
                }

                // If we haven't wrapped yet and we've reached the first line, reset cursor line to the
                // last line and set wrapped to true so we don't wrap again
                if wrap_around && !wrapped && cursor.line == 0 {
                    cursor.line = editor.with_buffer(|buffer| buffer.lines.len());
                    wrapped = true;
                }
            }
        }
        false
    }
}

/// Includes parent name in tab title
///
/// Useful for distinguishing between Rust modules named `mod.rs`
fn title_with_parent(path: &std::path::Path, file_name: &str) -> String {
    let parent_name = path
        .parent()
        .and_then(|path| path.file_name())
        .and_then(|os_str| os_str.to_str());

    match parent_name {
        Some(parent) => [parent, "/", file_name].concat(),
        None => file_name.to_string(),
    }
}
