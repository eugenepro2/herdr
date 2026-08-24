//! Fork: two rows pinned under the sidebar — the active space's git readout and
//! command buttons that opted into `button_position = "sidebar"`.

use ratatui::{
    layout::Rect,
    style::Style,
    text::{Line, Span},
    widgets::Paragraph,
    Frame,
};

use super::text::display_width_u16;
use super::widgets::panel_contrast_fg;
use crate::app::AppState;

/// " branch ↓behind ↑ahead" for the active space, empty when it has no repo.
fn git_spans(app: &AppState) -> Vec<Span<'static>> {
    if !app.sidebar_git_footer {
        return Vec::new();
    }
    let Some(ws) = app.active.and_then(|idx| app.workspaces.get(idx)) else {
        return Vec::new();
    };
    let Some(branch) = ws.branch() else {
        return Vec::new();
    };
    let p = &app.palette;
    let base = Style::default().bg(p.sidebar_bg);
    let mut spans = vec![Span::styled(format!(" {branch}"), base.fg(p.overlay1))];
    let (ahead, behind) = ws.git_ahead_behind().unwrap_or((0, 0));
    if behind > 0 {
        spans.push(Span::styled(format!(" ↓{behind}"), base.fg(p.red)));
    }
    if ahead > 0 {
        spans.push(Span::styled(format!(" ↑{ahead}"), base.fg(p.green)));
    }
    spans
}

fn sidebar_buttons(app: &AppState) -> Vec<(usize, &str)> {
    app.keybinds
        .custom_commands
        .iter()
        .enumerate()
        .filter(|(_, cmd)| cmd.button_position == crate::config::ButtonPosition::Sidebar)
        .filter_map(|(idx, cmd)| cmd.button.as_deref().map(|label| (idx, label)))
        .collect()
}

/// Rows the footer claims from the bottom of the sidebar: a git row, a button row.
pub(crate) fn footer_height(app: &AppState) -> u16 {
    if app.sidebar_collapsed {
        return 0;
    }
    u16::from(!git_spans(app).is_empty()) + u16::from(!sidebar_buttons(app).is_empty())
}

/// Left-to-right button cells on the footer's last row, with their index into
/// `keybinds.custom_commands`.
pub(crate) fn button_hit_areas(app: &AppState) -> Vec<(Rect, usize)> {
    let footer = app.view.sidebar_footer_rect;
    let mut out = Vec::new();
    if footer.width < 2 || footer.height == 0 {
        return out;
    }
    let mut x = footer.x;
    // the sidebar keeps its rightmost column for the separator rule
    let right = footer.x + footer.width - 1;
    let y = footer.y + footer.height - 1;
    for (idx, label) in sidebar_buttons(app) {
        let width = display_width_u16(label).saturating_add(2);
        if width == 0 || right.saturating_sub(x) < width {
            break;
        }
        out.push((Rect::new(x, y, width, 1), idx));
        x += width + 1;
    }
    out
}

pub(super) fn render_sidebar_footer(app: &AppState, frame: &mut Frame) {
    let footer = app.view.sidebar_footer_rect;
    if footer.width == 0 || footer.height == 0 {
        return;
    }
    let p = &app.palette;
    frame
        .buffer_mut()
        .set_style(footer, Style::default().bg(p.sidebar_bg));

    // carry the sidebar rules down through the footer rows
    let sep_x = footer.x + footer.width - 1;
    let gutter_x = app.sidebar_divider.then(|| footer.x + footer.width);
    let frame_area = frame.area();
    let buf = frame.buffer_mut();
    for y in footer.y..footer.y + footer.height {
        buf[(sep_x, y)]
            .set_symbol("│")
            .set_style(Style::default().fg(p.surface_dim));
        if let Some(x) = gutter_x.filter(|x| *x < frame_area.x + frame_area.width) {
            buf[(x, y)]
                .set_symbol("│")
                .set_style(Style::default().fg(p.surface_dim));
        }
    }

    let spans = git_spans(app);
    // a squeezed sidebar keeps the buttons and drops the branch line
    let squeezed = footer.height == 1 && !sidebar_buttons(app).is_empty();
    if !spans.is_empty() && !squeezed {
        frame.render_widget(
            Paragraph::new(Line::from(spans)),
            Rect::new(footer.x, footer.y, footer.width - 1, 1),
        );
    }

    let button_style = Style::default().fg(panel_contrast_fg(p)).bg(p.overlay1);
    for (rect, idx) in button_hit_areas(app) {
        let Some(label) = app
            .keybinds
            .custom_commands
            .get(idx)
            .and_then(|cmd| cmd.button.as_deref())
        else {
            continue;
        };
        frame.render_widget(
            Paragraph::new(Span::styled(format!(" {label} "), button_style)),
            rect,
        );
    }
}
