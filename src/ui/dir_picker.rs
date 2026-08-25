//! Folder picker overlay for the workspace bar "+" button.

use ratatui::{
    layout::Rect,
    style::{Modifier, Style},
    widgets::Paragraph,
    Frame,
};

use super::text::{middle_elide, truncate_end};
use super::widgets::{centered_popup_rect, render_modal_header, render_panel_shell};
use crate::app::dir_picker::{DirPickerRow, DirPickerState};
use crate::app::state::AppState;

const POPUP_W: u16 = 72;
const POPUP_H: u16 = 22;
/// Header, path, filter, separator above the list plus one footer row below it.
const CHROME_ROWS: u16 = 5;

pub(crate) fn dir_picker_popup_rect(app: &AppState) -> Option<Rect> {
    app.dir_picker.as_ref()?;
    centered_popup_rect(app.screen_rect(), POPUP_W, POPUP_H)
}

pub(crate) fn dir_picker_list_rect(app: &AppState) -> Option<Rect> {
    let popup = dir_picker_popup_rect(app)?;
    let inner = Rect::new(
        popup.x + 1,
        popup.y + 1,
        popup.width.saturating_sub(2),
        popup.height.saturating_sub(2),
    );
    let height = inner.height.checked_sub(CHROME_ROWS)?;
    (height > 0).then(|| Rect::new(inner.x, inner.y + 4, inner.width, height))
}

/// First visible row, so the highlighted one stays inside the list.
pub(crate) fn dir_picker_scroll_start(picker: &DirPickerState, visible: usize) -> usize {
    let total = picker.rows().len();
    if visible == 0 || total <= visible {
        return 0;
    }
    picker
        .selected
        .saturating_sub(visible - 1)
        .min(total - visible)
}

fn row_label(row: &DirPickerRow, dir: &std::path::Path) -> String {
    match row {
        DirPickerRow::Here => match dir.file_name().and_then(|name| name.to_str()) {
            Some(name) => format!("· создать здесь — {name}"),
            None => "· создать здесь".to_string(),
        },
        DirPickerRow::Up => "..".to_string(),
        DirPickerRow::Dir(name) => format!("▸ {name}"),
    }
}

fn home_relative(dir: &std::path::Path) -> String {
    let display = dir.display().to_string();
    match std::env::var("HOME") {
        Ok(home) if !home.is_empty() && display.starts_with(&home) => {
            format!("~{}", &display[home.len()..])
        }
        _ => display,
    }
}

pub(super) fn render_dir_picker_overlay(app: &AppState, frame: &mut Frame, area: Rect) {
    let Some(picker) = app.dir_picker.as_ref() else {
        return;
    };
    super::dim_background(frame, area);
    let Some(popup) = dir_picker_popup_rect(app) else {
        return;
    };
    let Some(inner) = render_panel_shell(frame, popup, app.palette.accent, app.palette.panel_bg)
    else {
        return;
    };
    let p = &app.palette;

    render_modal_header(
        frame,
        Rect::new(inner.x, inner.y, inner.width, 1),
        "новый спейс",
        p,
    );
    frame.render_widget(
        Paragraph::new(middle_elide(
            &home_relative(&picker.dir),
            inner.width as usize,
        ))
        .style(Style::default().fg(p.subtext0)),
        Rect::new(inner.x, inner.y + 1, inner.width, 1),
    );
    let filter = if picker.query.is_empty() {
        "фильтр: (начни печатать)".to_string()
    } else {
        format!("фильтр: {}", picker.query)
    };
    frame.render_widget(
        Paragraph::new(truncate_end(&filter, inner.width as usize)).style(Style::default().fg(
            if picker.query.is_empty() {
                p.overlay0
            } else {
                p.accent
            },
        )),
        Rect::new(inner.x, inner.y + 2, inner.width, 1),
    );
    frame.render_widget(
        Paragraph::new("─".repeat(inner.width as usize)).style(Style::default().fg(p.surface1)),
        Rect::new(inner.x, inner.y + 3, inner.width, 1),
    );

    if let Some(list) = dir_picker_list_rect(app) {
        let rows = picker.rows();
        let visible = list.height as usize;
        let start = dir_picker_scroll_start(picker, visible);
        for (offset, row) in rows.iter().skip(start).take(visible).enumerate() {
            let idx = start + offset;
            let style = if idx == picker.selected {
                Style::default()
                    .fg(p.text)
                    .bg(p.surface0)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(p.subtext0)
            };
            let mut label = row_label(row, &picker.dir);
            label = truncate_end(&format!(" {label}"), list.width as usize);
            if idx == picker.selected {
                let pad = (list.width as usize).saturating_sub(label.chars().count());
                label.push_str(&" ".repeat(pad));
            }
            frame.render_widget(
                Paragraph::new(label).style(style),
                Rect::new(list.x, list.y + offset as u16, list.width, 1),
            );
        }
        if rows.len() <= 1 {
            let message = picker
                .error
                .clone()
                .unwrap_or_else(|| "нет вложенных папок".to_string());
            frame.render_widget(
                Paragraph::new(truncate_end(&format!(" {message}"), list.width as usize))
                    .style(Style::default().fg(p.overlay0)),
                Rect::new(list.x, list.y + rows.len() as u16, list.width, 1),
            );
        }
    }

    frame.render_widget(
        Paragraph::new(truncate_end(
            " ↑↓ выбрать · → внутрь · ← назад · ↵ создать · esc отмена",
            inner.width as usize,
        ))
        .style(Style::default().fg(p.overlay0)),
        Rect::new(
            inner.x,
            inner.y + inner.height.saturating_sub(1),
            inner.width,
            1,
        ),
    );
}
