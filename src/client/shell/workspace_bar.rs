use super::*;

/// Width of the trailing " + " cell in the workspace strip.
const NEW_WORKSPACE_WIDTH: u16 = 3;

/// Agents in one workspace that either want an answer or finished unseen.
fn attention_count(snapshot: &ClientShellSnapshot, workspace_id: &str) -> usize {
    use crate::api::schema::AgentStatus;
    snapshot
        .agents
        .iter()
        .filter(|agent| agent.workspace_id == workspace_id)
        .filter(|agent| matches!(agent.agent_status, AgentStatus::Blocked | AgentStatus::Done))
        .count()
}

fn cell_label(
    snapshot: &ClientShellSnapshot,
    workspace: &ClientShellWorkspace,
    with_counts: bool,
) -> String {
    let count = if with_counts {
        attention_count(snapshot, &workspace.workspace_id)
    } else {
        0
    };
    let suffix = if count > 0 {
        format!(" ({count})")
    } else {
        String::new()
    };
    format!("{} {}{suffix}", workspace.number, workspace.label)
}

/// Cells stop short of the row end so the " + " cell always has room.
fn cells_area(area: Rect) -> Rect {
    Rect {
        width: area.width.saturating_sub(NEW_WORKSPACE_WIDTH + 1),
        ..area
    }
}

/// Left-to-right cells, one per workspace, truncated at the right edge.
// ponytail: no scroll buttons; copy tabs.rs scrolling when spaces stop fitting.
pub(super) fn render_workspace_bar(
    buffer: &mut Buffer,
    area: Rect,
    snapshot: &ClientShellSnapshot,
    config: &ClientShellConfig,
    active_endpoint_id: &ClientEndpointId,
    hits: &mut ShellHitMap,
) {
    if area.width == 0 || area.height == 0 {
        return;
    }
    let palette = &config.palette;
    buffer.set_style(area, Style::default().bg(palette.panel_bg));

    let cells = cells_area(area);
    let right = cells.right();
    let mut x = cells.x;
    let mut last_cell_right = None;
    for workspace in &snapshot.workspaces {
        if x >= right {
            break;
        }
        let label = cell_label(snapshot, workspace, config.workspace_bar_agent_counts);
        // " {icon} {label} " -> label plus the icon and three padding cells.
        let width = display_width(&label)
            .saturating_add(4)
            .min(right.saturating_sub(x))
            .max(1);
        let rect = Rect::new(x, area.y, width, 1);
        let active = workspace.focused;
        let cell_style = if active {
            Style::default()
                .fg(panel_contrast_fg(palette))
                .bg(palette.accent)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(palette.overlay1).bg(palette.surface0)
        };
        let icon_style = if active {
            cell_style
        } else {
            Style::default()
                .fg(status_color(workspace.agent_status, palette))
                .bg(palette.surface0)
        };
        buffer.set_style(rect, cell_style);
        put_text(buffer, rect.x, rect.y, rect.width, " ", cell_style);
        put_text(
            buffer,
            rect.x.saturating_add(1),
            rect.y,
            rect.width.saturating_sub(1),
            status_icon_anim(workspace.agent_status, config.status_indicators, config.working_anim_frame),
            icon_style,
        );
        put_text(
            buffer,
            rect.x.saturating_add(2),
            rect.y,
            rect.width.saturating_sub(2),
            &format!("{label} "),
            cell_style,
        );
        hits.workspaces.push(WorkspaceHit {
            rect,
            endpoint_id: active_endpoint_id.clone(),
            workspace_id: workspace.workspace_id.clone(),
            indented: false,
            in_workspace_bar: true,
            group_toggle: None,
        });
        last_cell_right = Some(rect.right());
        x = rect.right().saturating_add(1);
    }

    if !config.mouse_capture {
        return;
    }
    let new_x = last_cell_right.map_or(cells.x, |right| right.saturating_add(1));
    if new_x.saturating_add(NEW_WORKSPACE_WIDTH) <= area.right() {
        hits.workspace_bar_new = Rect::new(new_x, area.y, NEW_WORKSPACE_WIDTH, 1);
        put_text(
            buffer,
            new_x,
            area.y,
            NEW_WORKSPACE_WIDTH,
            " + ",
            Style::default().fg(palette.overlay1).bg(palette.panel_bg),
        );
    }
}
