//! Fork: `fs.list_dirs` feeds the workspace strip's folder browser, so it walks
//! the disks of the machine the space will live on, not the client's.

use std::path::{Path, PathBuf};

use crate::api::schema::{FsListDirsParams, ResponseResult};
use crate::app::App;

use super::responses::{encode_error, encode_success};

impl App {
    pub(super) fn handle_fs_list_dirs(&mut self, id: String, params: FsListDirsParams) -> String {
        let dir = params
            .path
            .map(PathBuf::from)
            .or_else(|| std::env::var_os("HOME").map(PathBuf::from))
            .unwrap_or_else(|| PathBuf::from("/"));
        match list_subdirs(&dir) {
            Ok(dirs) => encode_success(
                id,
                ResponseResult::DirList {
                    path: dir.display().to_string(),
                    dirs,
                },
            ),
            Err(err) => encode_error(id, "fs_list_failed", err.to_string()),
        }
    }
}

/// Subdirectory names of `dir`, sorted case-insensitively, dotfiles skipped.
fn list_subdirs(dir: &Path) -> std::io::Result<Vec<String>> {
    // ponytail: plain read_dir, no caching; a home directory listing is milliseconds.
    let mut names: Vec<String> = std::fs::read_dir(dir)?
        .filter_map(Result::ok)
        .filter(|entry| entry.path().is_dir())
        .filter_map(|entry| entry.file_name().into_string().ok())
        .filter(|name| !name.starts_with('.'))
        .collect();
    names.sort_by_key(|name| name.to_lowercase());
    Ok(names)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lists_sorted_visible_subdirs_only() {
        let root = std::env::temp_dir().join(format!("herdr-fs-list-dirs-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join("beta")).expect("beta");
        std::fs::create_dir_all(root.join("Alpha")).expect("alpha");
        std::fs::create_dir_all(root.join(".hidden")).expect("hidden");
        std::fs::write(root.join("file.txt"), "x").expect("file");

        assert_eq!(
            list_subdirs(&root).expect("list"),
            vec!["Alpha".to_owned(), "beta".to_owned()]
        );
        assert!(list_subdirs(&root.join("missing")).is_err());

        let _ = std::fs::remove_dir_all(&root);
    }
}
