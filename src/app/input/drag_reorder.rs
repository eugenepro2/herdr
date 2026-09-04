//! Fork: `ui.drag_reorder` — drag a workspace cell in the workspace bar, or an
//! agent row in the sidebar agent panel, to reorder it.
//!
//! Both gestures reuse the upstream drag targets (`WorkspaceReorder` and
//! `TabReorder`) and their mouse-up handling, so all that lives here is the two
//! hit tests that say where a drop would land. An agent row stands for its tab,
//! so reordering an agent moves the whole tab; a tab holding several panes
//! moves as one block.

use ratatui::layout::Rect;

use crate::app::state::{AgentPanelSort, AppState, WorkspaceDropTarget};

impl AppState {
    /// Drop slots between workspace bar cells: the target and the column of the
    /// gap it would open, left of each cell plus one past the last.
    pub(crate) fn workspace_bar_drop_slots(&self) -> Vec<(WorkspaceDropTarget, u16)> {
        let mut slots = Vec::new();
        if !self.drag_reorder {
            return slots;
        }
        let cells = &self.view.workspace_bar_hit_areas;
        for (idx, rect) in cells.iter().enumerate() {
            if rect.width == 0 {
                continue;
            }
            slots.push((WorkspaceDropTarget::Before(idx), rect.x.saturating_sub(1)));
        }
        if let Some(last) = cells.iter().rev().find(|rect| rect.width > 0) {
            slots.push((WorkspaceDropTarget::End, last.x + last.width));
        }
        slots
    }

    pub(super) fn workspace_bar_drop_target_at(
        &self,
        col: u16,
        row: u16,
    ) -> Option<WorkspaceDropTarget> {
        let bar_row = self
            .view
            .workspace_bar_hit_areas
            .iter()
            .find(|rect| rect.width > 0)?
            .y;
        if row != bar_row {
            return None;
        }
        self.workspace_bar_drop_slots()
            .into_iter()
            .min_by_key(|(_, slot_col)| col.abs_diff(*slot_col))
            .map(|(target, _)| target)
    }

    /// Drop slots in the agent panel: the tab index a drop would insert before,
    /// and the row it would open at. Empty unless the panel still mirrors tab
    /// order — a filtered or priority-sorted list has no order to rewrite.
    pub(crate) fn agent_drop_slots(&self) -> Vec<(usize, u16)> {
        let mut slots = Vec::new();
        if !self.drag_reorder
            || self.sidebar_collapsed
            || self.agent_view_override.is_some()
            || !matches!(self.agent_panel_sort, AgentPanelSort::Spaces)
        {
            return slots;
        }
        let Some(active) = self.active else {
            return slots;
        };
        let detail_area = self.agent_panel_rect();
        let metrics = crate::ui::agent_panel_scroll_metrics(self, detail_area);
        let body = crate::ui::agent_panel_body_rect(
            detail_area,
            crate::ui::should_show_scrollbar(metrics),
        );
        if body.height == 0 {
            return slots;
        }

        let entries = crate::ui::agent_panel_entries(self);
        let scroll = self.agent_panel_scroll.min(metrics.max_offset_from_bottom);
        let body_bottom = body.y + body.height;
        let mut row_y = body.y;
        let mut last_tab = None;
        for (index, entry) in entries.iter().enumerate().skip(scroll) {
            let height = crate::ui::agent_entry_height_in_body(self, entry, body.height);
            if row_y.saturating_add(height) > body_bottom {
                break;
            }
            if entry.ws_idx == active && last_tab != Some(entry.tab_idx) {
                slots.push((entry.tab_idx, row_y));
                last_tab = Some(entry.tab_idx);
            }
            row_y = row_y
                .saturating_add(height)
                .saturating_add(crate::ui::agent_entry_gap(self, index, entries.len()))
                .min(body_bottom);
        }
        if last_tab.is_some() && row_y < body_bottom {
            if let Some(tabs) = self.workspaces.get(active).map(|ws| ws.tabs.len()) {
                slots.push((tabs, row_y));
            }
        }
        slots
    }

    pub(super) fn agent_drop_tab_index_at(&self, row: u16) -> Option<usize> {
        let area = self.agent_panel_rect();
        if area == Rect::default() || row < area.y || row >= area.y + area.height {
            return None;
        }
        self.agent_drop_slots()
            .into_iter()
            .min_by_key(|(_, slot_row)| row.abs_diff(*slot_row))
            .map(|(tab_idx, _)| tab_idx)
    }
}

#[cfg(test)]
mod tests {
    use crossterm::event::{MouseButton, MouseEventKind};
    use ratatui::layout::Rect;

    use super::super::{app_for_mouse_test, mouse};
    use crate::app::state::{DragTarget, WorkspaceDropTarget};
    use crate::workspace::Workspace;

    #[test]
    fn dragging_a_workspace_bar_cell_reorders_the_space() {
        let mut app = app_for_mouse_test();
        app.state.workspaces = vec![
            Workspace::test_new("a"),
            Workspace::test_new("b"),
            Workspace::test_new("c"),
        ];
        app.state.active = Some(0);
        app.state.selected = 0;
        app.state.workspace_bar = true;
        app.state.drag_reorder = true;
        crate::ui::compute_view(&mut app.state, Rect::new(0, 0, 106, 20));

        let cells = app.state.view.workspace_bar_hit_areas.clone();
        let (first, last) = (cells[0], cells[2]);
        app.handle_mouse(mouse(
            MouseEventKind::Down(MouseButton::Left),
            first.x + 1,
            first.y,
        ));
        assert!(matches!(
            app.state.drag.as_ref().map(|drag| &drag.target),
            Some(DragTarget::WorkspaceReorder {
                source_ws_idx: 0,
                drop_target: None,
                ..
            })
        ));

        let drop_col = last.x + last.width;
        app.handle_mouse(mouse(
            MouseEventKind::Drag(MouseButton::Left),
            drop_col,
            last.y,
        ));
        assert!(matches!(
            app.state.drag.as_ref().map(|drag| &drag.target),
            Some(DragTarget::WorkspaceReorder {
                drop_target: Some(WorkspaceDropTarget::End),
                ..
            })
        ));

        app.handle_mouse(mouse(MouseEventKind::Up(MouseButton::Left), drop_col, last.y));

        let names: Vec<_> = app
            .state
            .workspaces
            .iter()
            .map(|ws| ws.display_name())
            .collect();
        assert_eq!(names, vec!["b", "c", "a"]);
    }

    #[test]
    fn dragging_an_agent_row_reorders_its_tab() {
        let mut app = app_for_mouse_test();
        let mut ws = Workspace::test_new("one");
        ws.test_add_tab(Some("second"));
        ws.test_add_tab(Some("third"));
        app.state.workspaces = vec![ws];
        app.state.ensure_test_terminals();
        for tab_idx in 0..app.state.workspaces[0].tabs.len() {
            let pane_id = app.state.workspaces[0].tabs[tab_idx].root_pane;
            let terminal_id = app.state.workspaces[0].tabs[tab_idx].panes[&pane_id]
                .attached_terminal_id
                .clone();
            app.state
                .terminals
                .get_mut(&terminal_id)
                .unwrap()
                .detected_agent = Some(crate::detect::Agent::Claude);
        }
        app.state.active = Some(0);
        app.state.drag_reorder = true;
        app.state.sidebar_agents.rows = vec![vec![crate::config::AgentSidebarToken::Agent]];
        app.state.sidebar_agents.row_gap = 0;
        crate::ui::compute_view(&mut app.state, Rect::new(0, 0, 106, 30));

        let slots = app.state.agent_drop_slots();
        assert_eq!(
            slots.iter().map(|(tab, _)| *tab).collect::<Vec<_>>(),
            vec![0, 1, 2, 3]
        );
        let source_row = slots[0].1;
        let drop_row = slots[3].1;
        let first_tab_root = app.state.workspaces[0].tabs[0].root_pane;

        app.handle_mouse(mouse(
            MouseEventKind::Down(MouseButton::Left),
            2,
            source_row,
        ));
        assert!(matches!(
            app.state.drag.as_ref().map(|drag| &drag.target),
            Some(DragTarget::TabReorder {
                source_tab_idx: 0,
                insert_idx: None,
                ..
            })
        ));

        app.handle_mouse(mouse(MouseEventKind::Drag(MouseButton::Left), 2, drop_row));
        assert!(matches!(
            app.state.drag.as_ref().map(|drag| &drag.target),
            Some(DragTarget::TabReorder {
                insert_idx: Some(3),
                ..
            })
        ));

        app.handle_mouse(mouse(MouseEventKind::Up(MouseButton::Left), 2, drop_row));
        assert_eq!(app.state.workspaces[0].tabs[2].root_pane, first_tab_root);
    }

    #[test]
    fn workspace_bar_drag_stays_off_without_the_flag() {
        let mut app = app_for_mouse_test();
        app.state.workspaces = vec![Workspace::test_new("a"), Workspace::test_new("b")];
        app.state.active = Some(0);
        app.state.workspace_bar = true;
        crate::ui::compute_view(&mut app.state, Rect::new(0, 0, 106, 20));

        let first = app.state.view.workspace_bar_hit_areas[0];
        app.handle_mouse(mouse(
            MouseEventKind::Down(MouseButton::Left),
            first.x + 1,
            first.y,
        ));
        assert!(app.state.drag.is_none());
    }
}
