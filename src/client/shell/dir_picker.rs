//! Fork: the folder browser behind the workspace strip's "+" button.
//!
//! Folders come from the endpoint (`fs.list_dirs`), so it walks the disks of
//! whichever machine the space will live on — Local or a saved SSH machine.
//! A server without that method keeps upstream's plain new workspace.

use std::path::PathBuf;

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
    /// A listing is on its way; keys wait for it so they act on what is shown.
    pub(super) loading: bool,
    /// Folder to highlight once the pending listing arrives (the one just left).
    land_on: Option<String>,
}

impl ClientDirPickerOverlay {
    pub(super) fn open(dir: PathBuf) -> Self {
        Self {
            dir,
            entries: Vec::new(),
            query: String::new(),
            selected: 0,
            error: None,
            loading: true,
            land_on: None,
        }
    }

    /// Show a listing that arrived from the endpoint.
    pub(super) fn show(&mut self, dir: PathBuf, entries: Vec<String>) {
        self.dir = dir;
        self.entries = entries;
        self.query.clear();
        self.error = None;
        self.loading = false;
        self.selected = 0;
        // Land on the directory we just left so ←/→ retrace the same path.
        if let Some(leaving) = self.land_on.take() {
            if let Some(idx) = self
                .rows()
                .iter()
                .position(|row| *row == DirPickerRow::Dir(leaving.clone()))
            {
                self.selected = idx;
            }
        }
    }

    /// The listing failed; the folder shown before stays behind the error.
    pub(super) fn fail(&mut self, error: String) {
        self.error = Some(error);
        self.loading = false;
        self.land_on = None;
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

    /// Folder to list when walking into the highlighted row; None when it is
    /// not navigable.
    pub(super) fn descend(&mut self) -> Option<PathBuf> {
        match self.selected_row() {
            Some(DirPickerRow::Dir(name)) => Some(self.dir.join(name)),
            Some(DirPickerRow::Up) => self.ascend(),
            _ => None,
        }
    }

    /// Parent folder to list, remembering which folder to land back on.
    pub(super) fn ascend(&mut self) -> Option<PathBuf> {
        let parent = self.dir.parent()?.to_path_buf();
        self.land_on = self
            .dir
            .file_name()
            .and_then(|name| name.to_str())
            .map(str::to_string);
        Some(parent)
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
    /// Fork: the workspace strip's "+" browses the endpoint's folders. A server
    /// that cannot list them keeps upstream's plain new workspace.
    pub(super) fn open_dir_picker(&mut self, outcome: &mut ClientShellInput) {
        let probe = crate::api::schema::Method::FsListDirs(Default::default());
        if !self.supports_endpoint_method(&probe) {
            self.record_binding(
                crate::input::KeybindMatch::Action(crate::input::KeybindAction::NewWorkspace),
                outcome,
            );
            return;
        }
        // The focused space's cwd lives on the endpoint; None lets it pick home.
        let start = self
            .snapshot
            .as_deref()
            .and_then(|snapshot| {
                snapshot
                    .workspaces
                    .iter()
                    .find(|workspace| workspace.focused)
                    .map(|workspace| workspace.new_workspace_cwd.clone())
            })
            .filter(|cwd| !cwd.is_empty());
        self.overlay = Some(ClientShellOverlay::DirPicker(ClientDirPickerOverlay::open(
            start.clone().map(PathBuf::from).unwrap_or_default(),
        )));
        self.request_dir_list(start, outcome);
        outcome.repaint = true;
    }

    fn request_dir_list(&mut self, path: Option<String>, outcome: &mut ClientShellInput) {
        let sent = self.push_endpoint_method_with_kind(
            crate::api::schema::Method::FsListDirs(crate::api::schema::FsListDirsParams { path }),
            PendingEndpointKind::DirList,
            outcome,
        );
        if !sent {
            if let Some(ClientShellOverlay::DirPicker(picker)) = self.overlay.as_mut() {
                picker.fail("the machine is not ready".to_owned());
            }
        }
    }

    pub(super) fn receive_dir_list(
        &mut self,
        result: Result<crate::api::schema::ResponseResult, ClientShellEndpointError>,
    ) -> bool {
        let Some(ClientShellOverlay::DirPicker(picker)) = self.overlay.as_mut() else {
            return false;
        };
        match result {
            Ok(crate::api::schema::ResponseResult::DirList { path, dirs }) => {
                picker.show(PathBuf::from(path), dirs);
            }
            Ok(_) => picker.fail("the machine returned an unexpected folder listing".to_owned()),
            Err(error) => picker.fail(error.message),
        }
        true
    }

    pub(super) fn handle_dir_picker_key(
        &mut self,
        key: &crate::input::TerminalKey,
        outcome: &mut ClientShellInput,
    ) {
        let Some(ClientShellOverlay::DirPicker(picker)) = self.overlay.as_mut() else {
            return;
        };
        if matches!(key.code, KeyCode::Esc) {
            self.overlay = None;
            outcome.repaint = true;
            return;
        }
        if picker.loading {
            return;
        }
        // The next key dismisses a shown error; the folder behind it stays.
        if picker.error.take().is_some() {
            outcome.repaint = true;
            return;
        }
        let target = match key.code {
            KeyCode::Up => {
                picker.move_prev();
                None
            }
            KeyCode::Down => {
                picker.move_next();
                None
            }
            KeyCode::Right => picker.descend(),
            KeyCode::Left => picker.ascend(),
            KeyCode::Backspace => {
                if picker.pop_query() {
                    None
                } else {
                    picker.ascend()
                }
            }
            KeyCode::Enter => match picker.chosen_path() {
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
                    outcome.repaint = true;
                    return;
                }
                // "up" is not a destination; Enter walks out instead.
                None => picker.ascend(),
            },
            KeyCode::Char(ch) => {
                picker.push_query(ch);
                None
            }
            _ => None,
        };
        if let Some(dir) = target {
            picker.loading = true;
            self.request_dir_list(Some(dir.display().to_string()), outcome);
        }
        outcome.repaint = true;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn walks_in_and_out_of_listings_from_the_endpoint() {
        let root = PathBuf::from("/srv/code");
        let mut picker = ClientDirPickerOverlay::open(root.clone());
        assert!(picker.loading);
        picker.show(root.clone(), vec!["alpha".to_owned(), "beta".to_owned()]);
        assert!(!picker.loading);
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
        assert_eq!(picker.descend(), Some(root.join("alpha")));
        picker.show(root.join("alpha"), Vec::new());
        // Walking out lands back on the folder just left.
        assert_eq!(picker.ascend(), Some(root.clone()));
        picker.show(root.clone(), vec!["alpha".to_owned(), "beta".to_owned()]);
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

        // A failed listing keeps the folder shown before it.
        picker.fail("permission denied".to_owned());
        assert_eq!(picker.dir, root);
        assert_eq!(picker.error.as_deref(), Some("permission denied"));
    }
}
