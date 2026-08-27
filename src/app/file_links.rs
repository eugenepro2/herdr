//! Форк: ctrl+клик по пути файла в выводе панели.
//!
//! Терминал печатает путь обычным текстом («Готово. Файл: docs/x.md»), OSC 8
//! там нет — [`file_link_at_column`] узнаёт такой путь под курсором и отдаёт
//! его как `file://…`, дальше он идёт тем же путём, что и http-ссылка:
//! плагин с `[[link_handlers]]` на схему `file` открывает его чем хочет.
//! За флагом `ui.pane_file_links`, дефолт — поведение апстрима.

use std::path::{Path, PathBuf};

/// Обрамление, которым текст окружает путь: кавычки, скобки, «Файл: `x.md`,».
const OPENERS: [char; 8] = ['`', '"', '\'', '«', '(', '[', '{', '<'];
const CLOSERS: [char; 11] = ['`', '"', '\'', '»', ')', ']', '}', '>', ',', ';', '.'];

/// Путь под колонкой `col` строки `col`-ки панели, как `file://<абсолютный>`.
/// `cwd` — рабочая папка панели, ей мерятся относительные пути. Возвращает
/// ссылку только на реально существующий файл: иначе любое слово стало бы
/// кликабельным.
pub(crate) fn file_link_at_column(line: &str, col: u16, cwd: Option<&Path>) -> Option<String> {
    let token = token_at_column(line, col)?;
    let path = resolve(token, cwd)?;
    Some(format!("file://{}", path.display()))
}

/// Слово под колонкой — от пробела до пробела, без обрамления и без
/// хвоста `:строка:колонка`, который печатают компиляторы и grep.
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

/// Байтовый индекс символа, накрывающего колонку — ширина считается так же,
/// как её считает терминал, иначе клик по строке с кириллицей промахивается.
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

/// Токен → существующий файл. `~` раскрывается, относительный путь мерится
/// от папки панели.
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn file_link_at_column_resolves_a_relative_path_in_prose() {
        let dir = std::env::temp_dir().join(format!("herdr-file-links-{}", std::process::id()));
        std::fs::create_dir_all(dir.join("docs")).expect("docs");
        let file = dir.join("docs/встречи.md");
        std::fs::write(&file, "# заметки").expect("write");

        let line = "Готово. Файл: docs/встречи.md";
        let col = u16::try_from("Готово. Файл: doc".chars().count()).expect("col");

        assert_eq!(
            file_link_at_column(line, col, Some(dir.as_path())),
            Some(format!("file://{}", file.display()))
        );
        // Слово, за которым нет файла, ссылкой не становится.
        assert_eq!(file_link_at_column(line, 0, Some(dir.as_path())), None);

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn token_at_column_drops_wrapping_and_line_numbers() {
        assert_eq!(token_at_column("см. `src/ui.rs:42:8`,", 6), Some("src/ui.rs"));
    }
}
