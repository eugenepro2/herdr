//! Fork: ctrl+click a file path printed in pane output.
//!
//! Terminals print paths as plain prose ("Done. File: docs/x.md") with no OSC 8
//! around them, so [`file_link_at_column`] recognises such a path under the
//! cursor and hands it back as `file://…`. From there it travels the same road
//! as an http link: a plugin whose `[[link_handlers]]` pattern matches the
//! `file` scheme decides what opens it. Behind `ui.pane_file_links`; with the
//! flag off nothing here runs and upstream behaviour is unchanged.

use std::path::{Path, PathBuf};

/// Wrapping the surrounding prose puts around a path: quotes, brackets,
/// "File: `x.md`,".
const OPENERS: [char; 8] = ['`', '"', '\'', '«', '(', '[', '{', '<'];
const CLOSERS: [char; 11] = ['`', '"', '\'', '»', ')', ']', '}', '>', ',', ';', '.'];

/// The path under column `col` of `line`, as `file://<absolute>`. `cwd` is the
/// pane's working directory, which relative paths are measured from. Only an
/// existing file becomes a link — otherwise every word would be clickable.
pub(crate) fn file_link_at_column(line: &str, col: u16, cwd: Option<&Path>) -> Option<String> {
    let token = token_at_column(line, col)?;
    let path = resolve(token, cwd)?;
    Some(format!("file://{}", path.display()))
}

/// The word under the column — whitespace to whitespace, with the wrapping and
/// any `:line:col` suffix (as printed by compilers and grep) stripped.
fn token_at_column(line: &str, col: u16) -> Option<&str> {
    let hit = byte_index_at_column(line, col)?;
    if line[hit..].chars().next()?.is_whitespace() {
        return None;
    }
    let start = line[..hit]
        .char_indices()
        .rev()
        .find(|(_, ch)| ch.is_whitespace())
        .map_or(0, |(idx, ch)| idx + ch.len_utf8());
    let end = line[hit..]
        .find(char::is_whitespace)
        .map_or(line.len(), |idx| hit + idx);
    let token = line
        .get(start..end)?
        .trim_start_matches(OPENERS)
        .trim_end_matches(CLOSERS);
    Some(strip_position(token))
}

/// Byte index of the character covering `col`. Width is counted the way the
/// terminal counts it, or a click on a line with wide characters misses.
fn byte_index_at_column(line: &str, col: u16) -> Option<usize> {
    let mut width = 0u16;
    for (idx, ch) in line.char_indices() {
        let cell = u16::from(crate::ghostty::unicode_codepoint_width(ch as u32)).max(1);
        if col < width.saturating_add(cell) {
            return Some(idx);
        }
        width = width.saturating_add(cell);
    }
    None
}

/// `src/ui.rs:42:8` → `src/ui.rs`.
fn strip_position(token: &str) -> &str {
    let mut out = token;
    for _ in 0..2 {
        match out.rsplit_once(':') {
            Some((head, tail))
                if !tail.is_empty() && tail.chars().all(|ch| ch.is_ascii_digit()) =>
            {
                out = head;
            }
            _ => break,
        }
    }
    out
}

/// Token to an existing file. `~` expands; a relative path is measured from the
/// pane's directory.
fn resolve(token: &str, cwd: Option<&Path>) -> Option<PathBuf> {
    if token.len() < 2 {
        return None;
    }
    let expanded = match token.strip_prefix("~/") {
        Some(rest) => PathBuf::from(std::env::var_os("HOME")?).join(rest),
        None => PathBuf::from(token),
    };
    let absolute = if expanded.is_absolute() {
        expanded
    } else {
        cwd?.join(expanded)
    };
    absolute.is_file().then_some(absolute)
}

/// Fork: alt+click a file path to open its folder.
/// `file:///a/b/c.md` → `file:///a/b`. Not a file link, or the root: None.
pub(crate) fn parent_dir_url(url: &str) -> Option<String> {
    let path = url.strip_prefix("file://")?;
    let parent = Path::new(path).parent()?;
    if parent.as_os_str().is_empty() {
        return None;
    }
    Some(format!("file://{}", parent.display()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn file_link_at_column_resolves_a_relative_path_in_prose() {
        let dir = std::env::temp_dir().join(format!("herdr-file-links-{}", std::process::id()));
        std::fs::create_dir_all(dir.join("docs")).expect("docs");
        let file = dir.join("docs/встречи.md");
        std::fs::write(&file, "# notes").expect("write");

        let line = "Done. File: docs/встречи.md";
        let col = u16::try_from("Done. File: doc".chars().count()).expect("col");

        assert_eq!(
            file_link_at_column(line, col, Some(dir.as_path())),
            Some(format!("file://{}", file.display()))
        );
        // A word with no file behind it does not become a link.
        assert_eq!(file_link_at_column(line, 0, Some(dir.as_path())), None);

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn token_at_column_drops_wrapping_and_line_numbers() {
        assert_eq!(token_at_column("see `src/ui.rs:42:8`,", 6), Some("src/ui.rs"));
    }

    #[test]
    fn parent_dir_url_points_at_the_containing_folder() {
        assert_eq!(
            parent_dir_url("file:///a/b/c.md").as_deref(),
            Some("file:///a/b")
        );
        assert_eq!(parent_dir_url("https://example.com/x"), None);
        assert_eq!(parent_dir_url("file:///"), None);
    }
}
