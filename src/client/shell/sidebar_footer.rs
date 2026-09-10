//! Fork: rows pinned under the sidebar — the focused space's git readout and
//! the command buttons that opted into `button_position = "sidebar"`.

use super::*;

/// Left-to-right button cells on the footer's last row, with their index into
/// `config.keybinds.keybinds.custom_commands`.
pub(super) fn button_hit_areas(config: &ClientShellConfig, footer: Rect) -> Vec<(Rect, usize)> {
    let mut out = Vec::new();
    if footer.width < 2 || footer.height == 0 {
        return out;
    }
    let mut x = footer.x;
    // the sidebar keeps its rightmost column for the separator rule
    let right = footer.right().saturating_sub(1);
    let y = footer.bottom().saturating_sub(1);
    for (index, label) in sidebar_buttons(config) {
        let width = display_width(label).saturating_add(2);
        if width == 0 || right.saturating_sub(x) < width {
            break;
        }
        out.push((Rect::new(x, y, width, 1), index));
        x = x.saturating_add(width + 1);
    }
    out
}

fn sidebar_buttons(config: &ClientShellConfig) -> Vec<(usize, &str)> {
    config
        .keybinds
        .keybinds
        .custom_commands
        .iter()
        .enumerate()
        .filter(|(_, command)| {
            command.button_position == crate::config::ButtonPosition::Sidebar
        })
        .filter_map(|(index, command)| command.button.as_deref().map(|label| (index, label)))
        .collect()
}

/// Rows the footer claims from the bottom of the sidebar: a git row, a button row.
/// The git row is reserved whenever the flag is on, so the layout does not jump
/// when a space without a repo is focused.
pub(in crate::client::shell) fn footer_height(config: &ClientShellConfig, sidebar_collapsed: bool) -> u16 {
    if sidebar_collapsed {
        return 0;
    }
    u16::from(config.sidebar_git_footer) + u16::from(!sidebar_buttons(config).is_empty())
}

pub(super) fn render_sidebar_footer(
    buffer: &mut Buffer,
    footer: Rect,
    snapshot: Option<&ClientShellSnapshot>,
    config: &ClientShellConfig,
    hits: &mut ShellHitMap,
) {
    if footer.width == 0 || footer.height == 0 {
        return;
    }
    let palette = &config.palette;
    buffer.set_style(footer, Style::default().bg(palette.sidebar_bg));

    // a squeezed footer keeps the buttons and drops the branch line
    let buttons = sidebar_buttons(config);
    let squeezed = footer.height == 1 && !buttons.is_empty();
    if config.sidebar_git_footer && !squeezed {
        render_git_row(
            buffer,
            Rect::new(footer.x, footer.y, footer.width.saturating_sub(1), 1),
            snapshot,
            palette,
        );
    }

    if !config.mouse_capture {
        return;
    }
    let rects = button_hit_areas(config, footer);
    super::render::render_command_buttons(buffer, config, &rects);
    hits.command_buttons.extend(rects);
}

/// " branch ↓behind ↑ahead" for the focused space, blank when it has no repo.
fn render_git_row(
    buffer: &mut Buffer,
    row: Rect,
    snapshot: Option<&ClientShellSnapshot>,
    palette: &Palette,
) {
    let Some(workspace) = snapshot.and_then(|snapshot| {
        snapshot
            .workspaces
            .iter()
            .find(|workspace| Some(workspace.workspace_id.as_str()) == snapshot.focused_workspace_id.as_deref())
    }) else {
        return;
    };
    let Some(branch) = workspace.branch.as_deref() else {
        return;
    };
    let base = Style::default().bg(palette.sidebar_bg);
    let mut x = row.x;
    let mut put = |text: &str, style: Style| {
        let width = display_width(text).min(row.right().saturating_sub(x));
        if width == 0 {
            return;
        }
        put_text(buffer, x, row.y, width, text, style);
        x = x.saturating_add(width);
    };
    put(&format!(" {branch}"), base.fg(palette.overlay1));
    let (ahead, behind) = workspace.git_ahead_behind.unwrap_or((0, 0));
    if behind > 0 {
        put(&format!(" ↓{behind}"), base.fg(palette.red));
    }
    if ahead > 0 {
        put(&format!(" ↑{ahead}"), base.fg(palette.green));
    }
}
