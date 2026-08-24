use ratatui::{
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::Paragraph,
    Frame,
};

use super::status::state_icon;
use super::text::display_width_u16;
use super::widgets::panel_contrast_fg;
use crate::app::AppState;

fn cell_label(app: &AppState, ws_idx: usize) -> String {
    let Some(ws) = app.workspaces.get(ws_idx) else {
        return String::new();
    };
    format!(
        "{} {}",
        ws_idx + 1,
        ws.display_name_from_terminals(&app.terminals)
    )
}

/// Left-to-right cells, one per workspace, truncated at the right edge.
// ponytail: no scroll buttons; copy tabs.rs scrolling when spaces stop fitting.
pub(crate) fn workspace_bar_hit_areas(app: &AppState, area: Rect) -> Vec<Rect> {
    let mut rects = vec![Rect::default(); app.workspaces.len()];
    if area.width == 0 || area.height == 0 {
        return rects;
    }
    let mut x = area.x;
    let right = area.x + area.width;
    for (idx, rect) in rects.iter_mut().enumerate() {
        if x >= right {
            break;
        }
        // " {icon} {n} {label} " -> label plus icon and padding cells.
        let desired = display_width_u16(&cell_label(app, idx)).saturating_add(4);
        let width = desired.min(right.saturating_sub(x)).max(1);
        *rect = Rect::new(x, area.y, width, 1);
        x = x.saturating_add(width + 1);
    }
    rects
}

/// Width of the trailing " + " cell in the workspace bar.
pub(crate) const NEW_WORKSPACE_WIDTH: u16 = 3;

/// Workspace cells stop short of the row end so the " + " cell always has room.
pub(crate) fn workspace_cells_area(area: Rect) -> Rect {
    Rect {
        width: area.width.saturating_sub(NEW_WORKSPACE_WIDTH + 1),
        ..area
    }
}

/// " + " cell right after the last workspace, bounded by the full cells row.
pub(crate) fn workspace_bar_new_hit_area(area: Rect, cells: &[Rect]) -> Rect {
    if area.width == 0 || area.height == 0 {
        return Rect::default();
    }
    let x = match cells.iter().rev().find(|rect| rect.width > 0) {
        Some(rect) => rect.x + rect.width + 1,
        None => area.x,
    };
    if x + NEW_WORKSPACE_WIDTH > area.x + area.width {
        return Rect::default();
    }
    Rect::new(x, area.y, NEW_WORKSPACE_WIDTH, 1)
}

/// Right-aligned clickable buttons for custom commands with a `button` label.
/// Returns (rect, index into keybinds.custom_commands), right-to-left.
pub(crate) fn workspace_bar_button_hit_areas(app: &AppState, area: Rect) -> Vec<(Rect, usize)> {
    let mut out = Vec::new();
    if area.width == 0 || area.height == 0 {
        return out;
    }
    let mut right = area.x + area.width;
    for (idx, cmd) in app.keybinds.custom_commands.iter().enumerate() {
        let Some(label) = cmd.button.as_deref() else {
            continue;
        };
        let width = display_width_u16(label).saturating_add(2);
        if width == 0 || right.saturating_sub(area.x) < width {
            break;
        }
        right -= width;
        out.push((Rect::new(right, area.y, width, 1), idx));
        if right == area.x {
            break;
        }
        right -= 1;
    }
    out
}

/// Right-aligned " branch ↓behind ↑ahead " for the active space; empty when the
/// flag is off, the space has no repo, or git data has not landed yet.
pub(crate) fn workspace_bar_git_spans(app: &AppState) -> Vec<Span<'static>> {
    if !app.workspace_bar_git {
        return Vec::new();
    }
    let Some(branch) = app
        .active
        .and_then(|idx| app.workspaces.get(idx))
        .and_then(|ws| ws.branch())
    else {
        return Vec::new();
    };
    let p = &app.palette;
    let base = Style::default().bg(p.panel_bg);
    let mut spans = vec![Span::styled(format!(" {branch}"), base.fg(p.overlay1))];
    let (ahead, behind) = app
        .active
        .and_then(|idx| app.workspaces.get(idx))
        .and_then(|ws| ws.git_ahead_behind())
        .unwrap_or((0, 0));
    if behind > 0 {
        spans.push(Span::styled(format!(" ↓{behind}"), base.fg(p.red)));
    }
    if ahead > 0 {
        spans.push(Span::styled(format!(" ↑{ahead}"), base.fg(p.green)));
    }
    spans.push(Span::styled(" ", base));
    spans
}

/// Cells stop short of the git readout so the two never overlap.
pub(crate) fn workspace_bar_git_width(app: &AppState) -> u16 {
    workspace_bar_git_spans(app)
        .iter()
        .map(|span| display_width_u16(&span.content))
        .sum()
}

pub(super) fn render_workspace_bar(app: &AppState, frame: &mut Frame, area: Rect) {
    if area.width == 0 || area.height == 0 {
        return;
    }
    let p = &app.palette;
    frame.render_widget(
        Paragraph::new(" ".repeat(area.width as usize)).style(Style::default().bg(p.panel_bg)),
        area,
    );

    for (idx, ws) in app.workspaces.iter().enumerate() {
        let Some(rect) = app.view.workspace_bar_hit_areas.get(idx).copied() else {
            break;
        };
        if rect.width == 0 {
            continue;
        }
        let active = app.active == Some(idx);
        let cell_style = if active {
            Style::default()
                .fg(panel_contrast_fg(p))
                .bg(p.accent)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(p.overlay1).bg(p.surface0)
        };
        let (state, seen) = ws.aggregate_state(&app.terminals);
        let (icon, icon_style) = state_icon(state, seen, app.status_indicators, p);
        let icon_style = if active {
            cell_style
        } else {
            icon_style.bg(p.surface0)
        };
        let line = Line::from(vec![
            Span::styled(" ", cell_style),
            Span::styled(icon, icon_style),
            Span::styled(format!(" {} ", cell_label(app, idx)), cell_style),
        ]);
        frame.render_widget(Paragraph::new(line), rect);
    }

    let new_rect = app.view.workspace_bar_new_hit_area;
    if app.mouse_capture && new_rect.width > 0 {
        frame.render_widget(
            Paragraph::new(Span::styled(
                " + ",
                Style::default().fg(p.overlay1).bg(p.panel_bg),
            )),
            new_rect,
        );
    }

    let git_spans = workspace_bar_git_spans(app);
    if !git_spans.is_empty() {
        let width: u16 = git_spans
            .iter()
            .map(|span| display_width_u16(&span.content))
            .sum();
        let right = match app.view.workspace_bar_button_hit_areas.last() {
            Some((rect, _)) => rect.x,
            None => area.x + area.width,
        };
        if right.saturating_sub(area.x) >= width {
            frame.render_widget(
                Paragraph::new(Line::from(git_spans)),
                Rect::new(right - width, area.y, width, 1),
            );
        }
    }

    let button_style = Style::default().fg(panel_contrast_fg(p)).bg(p.overlay1);
    for (rect, idx) in &app.view.workspace_bar_button_hit_areas {
        let Some(label) = app
            .keybinds
            .custom_commands
            .get(*idx)
            .and_then(|cmd| cmd.button.as_deref())
        else {
            continue;
        };
        frame.render_widget(
            Paragraph::new(Span::styled(format!(" {label} "), button_style)),
            *rect,
        );
    }
}
