//! Fork: the folder browser behind the workspace strip's "+" button.
//!
//! It walks this machine's filesystem, so it is offered for the Local endpoint
//! only; on a saved SSH machine the "+" falls back to upstream's plain new
//! workspace, whose cwd the endpoint picks for itself.
// ponytail: a remote browser needs a directory-listing method on the endpoint;
// add one if browsing a saved machine's disks ever matters.

use std::path::{Path, PathBuf};

use super::*;

/// What a highlighted row does when activated.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum DirPickerRow {
    /// Create the workspace in the directory currently being browsed.
    Here,
    /// Step out to the parent directory.
    Up,
    /// A subdirectory of the current directory.
    Dir(String),
}

#[derive(Debug)]
pub(super) struct ClientDirPickerOverlay {
    pub(super) dir: PathBuf,
    /// Subdirectory names, sorted, dotfiles skipped.
    pub(super) entries: Vec<String>,
    pub(super) query: String,
    pub(super) selected: usize,
    pub(super) error: Option<String>,
}

impl ClientDirPickerOverlay {
    pub(super) fn open(dir: PathBuf) -> Self {
        let mut state = Self {
            dir,
            entries: Vec::new(),
            query: String::new(),
            selected: 0,
            error: None,
        };
        state.reload();
        state
    }

    fn reload(&mut self) {
        self.query.clear();
        self.selected = 0;
        self.error = None;
        // ponytail: plain read_dir, no caching; a home directory listing is milliseconds.
        match std::fs::read_dir(&self.dir) {
            Ok(read) => {
                let mut names: Vec<String> = read
                    .filter_map(Result::ok)
                    .filter(|entry| entry.path().is_dir())
                    .filter_map(|entry| entry.file_name().into_string().ok())
                    .filter(|name| !name.starts_with('.'))
                    .collect();
                names.sort_by_key(|name| name.to_lowercase());
                self.entries = names;
            }
            Err(err) => {
                self.entries.clear();
                self.error = Some(err.to_string());
            }
        }
    }

    /// Visible rows: "create here", "up", then the filtered subdirectories.
    pub(super) fn rows(&self) -> Vec<DirPickerRow> {
        let query = self.query.trim().to_lowercase();
        let mut rows = vec![DirPickerRow::Here];
        if self.dir.parent().is_some() {
            rows.push(DirPickerRow::Up);
        }
        rows.extend(
            self.entries
                .iter()
                .filter(|name| query.is_empty() || name.to_lowercase().contains(&query))
                .map(|name| DirPickerRow::Dir(name.clone())),
        );
        rows
    }

    fn selected_row(&self) -> Option<DirPickerRow> {
        self.rows().get(self.selected).cloned()
    }

    pub(super) fn move_prev(&mut self) {
        self.selected = self.selected.saturating_sub(1);
    }

    pub(super) fn move_next(&mut self) {
        let count = self.rows().len();
        if count > 0 {
            self.selected = (self.selected + 1).min(count - 1);
        }
    }

    pub(super) fn push_query(&mut self, ch: char) {
        self.query.push(ch);
        self.selected = 0;
    }

    pub(super) fn pop_query(&mut self) -> bool {
        let popped = self.query.pop().is_some();
        if popped {
            self.selected = 0;
        }
        popped
    }

    /// Descend into the highlighted row; returns false when it is not navigable.
    pub(super) fn descend(&mut self) -> bool {
        match self.selected_row() {
            Some(DirPickerRow::Dir(name)) => {
                self.dir = self.dir.join(name);
                self.reload();
                true
            }
            Some(DirPickerRow::Up) => self.ascend(),
            _ => false,
        }
    }

    pub(super) fn ascend(&mut self) -> bool {
        let Some(parent) = self.dir.parent().map(Path::to_path_buf) else {
            return false;
        };
        let leaving = self
            .dir
            .file_name()
            .and_then(|name| name.to_str())
            .map(str::to_string);
        self.dir = parent;
        self.reload();
        // Land on the directory we just left so ←/→ retrace the same path.
        if let Some(leaving) = leaving {
            if let Some(idx) = self
                .rows()
                .iter()
                .position(|row| *row == DirPickerRow::Dir(leaving.clone()))
            {
                self.selected = idx;
            }
        }
        true
    }

    /// Directory a workspace should be created in, or None when nothing is picked.
    pub(super) fn chosen_path(&self) -> Option<PathBuf> {
        match self.selected_row()? {
            DirPickerRow::Here => Some(self.dir.clone()),
            DirPickerRow::Dir(name) => Some(self.dir.join(name)),
            DirPickerRow::Up => None,
        }
    }
}

impl ClientShellState {
    /// Fork: the workspace strip's "+" browses folders on this machine. A saved
    /// SSH machine has no browser, so there it keeps upstream's plain new
    /// workspace.
    pub(super) fn open_dir_picker(&mut self, outcome: &mut ClientShellInput) {
        if !self.active_endpoint_id.is_local() {
            self.record_binding(
                crate::input::KeybindMatch::Action(crate::input::KeybindAction::NewWorkspace),
                outcome,
            );
            return;
        }
        let start = self
            .snapshot
            .as_deref()
            .and_then(|snapshot| {
                snapshot
                    .workspaces
                    .iter()
                    .find(|workspace| workspace.focused)
                    .map(|workspace| PathBuf::from(&workspace.new_workspace_cwd))
            })
            .filter(|dir| dir.is_dir())
            .or_else(|| std::env::var_os("HOME").map(PathBuf::from))
            .unwrap_or_else(|| PathBuf::from("/"));
        self.overlay = Some(ClientShellOverlay::DirPicker(ClientDirPickerOverlay::open(
            start,
        )));
        outcome.repaint = true;
    }

    pub(super) fn handle_dir_picker_key(
        &mut self,
        key: &crate::input::TerminalKey,
        outcome: &mut ClientShellInput,
    ) {
        let Some(ClientShellOverlay::DirPicker(picker)) = self.overlay.as_mut() else {
            return;
        };
        match key.code {
            KeyCode::Esc => {
                self.overlay = None;
            }
            KeyCode::Up => picker.move_prev(),
            KeyCode::Down => picker.move_next(),
            KeyCode::Right => {
                picker.descend();
            }
            KeyCode::Left => {
                picker.ascend();
            }
            KeyCode::Backspace => {
                if !picker.pop_query() {
                    picker.ascend();
                }
            }
            KeyCode::Enter => {
                let chosen = picker.chosen_path();
                match chosen {
                    Some(dir) => {
                        self.overlay = None;
                        self.push_endpoint_method(
                            crate::api::schema::Method::WorkspaceCreate(
                                crate::api::schema::WorkspaceCreateParams {
                                    source_workspace_id: None,
                                    cwd: Some(dir.display().to_string()),
                                    focus: true,
                                    label: None,
                                    env: Default::default(),
                                },
                            ),
                            outcome,
                        );
                    }
                    // "up" is not a destination; Enter walks out instead.
                    None => {
                        picker.ascend();
                    }
                }
            }
            KeyCode::Char(ch) => picker.push_query(ch),
            _ => {}
        }
        outcome.repaint = true;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_tree() -> PathBuf {
        let root = std::env::temp_dir().join(format!("herdr-dir-picker-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join("beta")).expect("beta");
        std::fs::create_dir_all(root.join("alpha")).expect("alpha");
        std::fs::create_dir_all(root.join(".hidden")).expect("hidden");
        std::fs::write(root.join("file.txt"), "x").expect("file");
        root
    }

    #[test]
    fn lists_sorted_subdirs_and_walks_in_and_out() {
        let root = temp_tree();
        let mut picker = ClientDirPickerOverlay::open(root.clone());
        assert_eq!(picker.entries, vec!["alpha".to_owned(), "beta".to_owned()]);
        assert_eq!(
            picker.rows(),
            vec![
                DirPickerRow::Here,
                DirPickerRow::Up,
                DirPickerRow::Dir("alpha".into()),
                DirPickerRow::Dir("beta".into()),
            ]
        );

        // Enter on "create here" picks the directory being browsed.
        assert_eq!(picker.chosen_path().as_deref(), Some(root.as_path()));

        picker.move_next();
        picker.move_next();
        assert!(picker.descend());
        assert_eq!(picker.dir, root.join("alpha"));
        // Walking out lands back on the folder just left.
        assert!(picker.ascend());
        assert_eq!(picker.dir, root);
        assert_eq!(picker.selected_row(), Some(DirPickerRow::Dir("alpha".into())));

        picker.push_query('b');
        assert_eq!(
            picker.rows(),
            vec![
                DirPickerRow::Here,
                DirPickerRow::Up,
                DirPickerRow::Dir("beta".into()),
            ]
        );

        let _ = std::fs::remove_dir_all(&root);
    }
}
