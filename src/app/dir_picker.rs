//! Folder picker backing `Mode::PickWorkspaceDir` — the workspace bar "+" button.

use std::path::{Path, PathBuf};

/// What a highlighted row does when activated.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DirPickerRow {
    /// Create the workspace in the directory currently being browsed.
    Here,
    /// Step out to the parent directory.
    Up,
    /// A subdirectory of the current directory.
    Dir(String),
}

pub struct DirPickerState {
    pub dir: PathBuf,
    /// Subdirectory names, sorted, dotfiles skipped.
    pub entries: Vec<String>,
    pub query: String,
    pub selected: usize,
    pub error: Option<String>,
}

impl DirPickerState {
    pub fn open(dir: PathBuf) -> Self {
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
    pub fn rows(&self) -> Vec<DirPickerRow> {
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

    pub fn selected_row(&self) -> Option<DirPickerRow> {
        self.rows().get(self.selected).cloned()
    }

    pub fn move_prev(&mut self) {
        self.selected = self.selected.saturating_sub(1);
    }

    pub fn move_next(&mut self) {
        let count = self.rows().len();
        if count > 0 {
            self.selected = (self.selected + 1).min(count - 1);
        }
    }

    pub fn push_query(&mut self, ch: char) {
        self.query.push(ch);
        self.selected = 0;
    }

    pub fn pop_query(&mut self) -> bool {
        let popped = self.query.pop().is_some();
        if popped {
            self.selected = 0;
        }
        popped
    }

    /// Descend into the highlighted row; returns false when it is not navigable.
    pub fn descend(&mut self) -> bool {
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

    pub fn ascend(&mut self) -> bool {
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
    pub fn chosen_path(&self) -> Option<PathBuf> {
        match self.selected_row()? {
            DirPickerRow::Here => Some(self.dir.clone()),
            DirPickerRow::Dir(name) => Some(self.dir.join(name)),
            DirPickerRow::Up => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_tree() -> PathBuf {
        let root = std::env::temp_dir().join(format!("herdr-dir-picker-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        for name in ["beta", "Alpha", ".hidden"] {
            std::fs::create_dir_all(root.join(name)).unwrap();
        }
        std::fs::write(root.join("file.txt"), b"x").unwrap();
        root
    }

    #[test]
    fn lists_sorted_subdirs_and_walks_in_and_out() {
        let root = temp_tree();
        let mut picker = DirPickerState::open(root.clone());
        assert_eq!(picker.entries, vec!["Alpha".to_string(), "beta".to_string()]);
        assert_eq!(
            picker.rows(),
            vec![
                DirPickerRow::Here,
                DirPickerRow::Up,
                DirPickerRow::Dir("Alpha".into()),
                DirPickerRow::Dir("beta".into()),
            ]
        );

        // filter narrows the subdirs only
        picker.push_query('b');
        assert_eq!(
            picker.rows(),
            vec![
                DirPickerRow::Here,
                DirPickerRow::Up,
                DirPickerRow::Dir("beta".into()),
            ]
        );
        assert!(picker.pop_query());

        // Enter on a subdir picks it, → walks into it
        picker.selected = 3;
        assert_eq!(picker.chosen_path(), Some(root.join("beta")));
        assert!(picker.descend());
        assert_eq!(picker.dir, root.join("beta"));
        assert_eq!(picker.chosen_path(), Some(root.join("beta")));

        // ← walks back out and lands on the directory we left
        assert!(picker.ascend());
        assert_eq!(picker.dir, root);
        assert_eq!(picker.selected_row(), Some(DirPickerRow::Dir("beta".into())));

        let _ = std::fs::remove_dir_all(&root);
    }
}
