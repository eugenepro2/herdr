//! Форк: подчёркивание кликабельного в выводе панели (`ui.pane_link_highlight`).
//!
//! Рисуется поверх уже отрисованного содержимого: символы читаются из буфера
//! кадра, поэтому терминал ни о чём не спрашивают. Подчёркивается ровно то,
//! что откроет ctrl+клик — http(s)-ссылки и, когда включён
//! `ui.pane_file_links`, пути к существующим файлам.
//!
//! Ссылка, перенесённая на следующую строку, подчёркивается только своей
//! первой строкой: в буфере кадра переноса не видно.

use std::path::Path;

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Modifier;

/// Хвостовая пунктуация, которая почти всегда принадлежит предложению, а не
/// ссылке: «смотри http://example.com/x.» Тот же набор режет `url_at_column`.
const TRAILING: [char; 12] = ['"', '\'', '`', '.', ',', ';', ':', '!', '?', ')', ']', '}'];

/// Символ и его колонка внутри панели.
struct Cell {
    ch: char,
    col: u16,
}

pub(super) fn render_link_underlines(buf: &mut Buffer, inner: Rect, cwd: Option<&Path>, files: bool) {
    for row in 0..inner.height {
        let cells = read_row(buf, inner, row);
        if cells.is_empty() {
            continue;
        }
        let line: String = cells.iter().map(|cell| cell.ch).collect();
        for (start, end) in link_spans(&cells, &line, cwd, files) {
            for cell in &cells[start..end] {
                let target = &mut buf[(inner.x + cell.col, inner.y + row)];
                target.set_style(target.style().add_modifier(Modifier::UNDERLINED));
            }
        }
    }
}

/// Строка панели как символы с колонками. Вторая половина широкого символа
/// приходит пустой ячейкой — она не свой символ, а его продолжение.
fn read_row(buf: &Buffer, inner: Rect, row: u16) -> Vec<Cell> {
    let y = inner.y + row;
    (0..inner.width)
        .filter_map(|col| {
            let ch = buf[(inner.x + col, y)].symbol().chars().next()?;
            Some(Cell { ch, col })
        })
        .collect()
}

/// Диапазоны ячеек (полуинтервалы), которые стоит подчеркнуть.
fn link_spans(
    cells: &[Cell],
    line: &str,
    cwd: Option<&Path>,
    files: bool,
) -> Vec<(usize, usize)> {
    let mut spans = Vec::new();
    let mut idx = 0;
    while idx < cells.len() {
        if cells[idx].ch.is_whitespace() {
            idx += 1;
            continue;
        }
        let start = idx;
        while idx < cells.len() && !cells[idx].ch.is_whitespace() {
            idx += 1;
        }
        let word: String = cells[start..idx].iter().map(|cell| cell.ch).collect();
        if word.starts_with("http://") || word.starts_with("https://") {
            let trimmed = word.trim_end_matches(TRAILING).chars().count();
            if trimmed > 0 {
                spans.push((start, start + trimmed));
            }
        } else if files
            && word.contains('/')
            && crate::app::file_links::file_link_at_column(line, cells[start].col, cwd).is_some()
        {
            spans.push((start, idx));
        }
    }
    spans
}

#[cfg(test)]
mod tests {
    use super::*;

    fn underlined(buf: &Buffer, area: Rect) -> String {
        (0..area.width)
            .map(|x| {
                let cell = &buf[(area.x + x, area.y)];
                if cell.style().add_modifier.contains(Modifier::UNDERLINED) {
                    '^'
                } else {
                    ' '
                }
            })
            .collect()
    }

    #[test]
    fn only_the_url_gets_underlined_without_its_trailing_period() {
        let area = Rect::new(0, 0, 30, 1);
        let mut buf = Buffer::empty(area);
        buf.set_string(0, 0, "тут http://a.io/b?c=d. хвост", ratatui::style::Style::default());

        render_link_underlines(&mut buf, area, None, false);

        assert_eq!(underlined(&buf, area), "    ^^^^^^^^^^^^^^^^^         ");
    }

    #[test]
    fn a_file_path_underlines_only_when_the_file_exists() {
        let dir = std::env::temp_dir().join(format!("herdr-pane-links-{}", std::process::id()));
        std::fs::create_dir_all(dir.join("docs")).expect("docs");
        std::fs::write(dir.join("docs/a.md"), "x").expect("write");

        let area = Rect::new(0, 0, 24, 1);
        let mut buf = Buffer::empty(area);
        buf.set_string(0, 0, "см docs/a.md и docs/b.md", ratatui::style::Style::default());

        render_link_underlines(&mut buf, area, Some(dir.as_path()), true);

        assert_eq!(underlined(&buf, area), "   ^^^^^^^^^            ");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
