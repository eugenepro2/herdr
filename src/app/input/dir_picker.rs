//! Keyboard and mouse handling for the new-workspace folder picker.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::layout::Rect;

use crate::app::dir_picker::{DirPickerRow, DirPickerState};
use crate::app::state::{AppState, Mode};

pub(crate) fn open_dir_picker(state: &mut AppState, dir: std::path::PathBuf) {
    state.dir_picker = Some(DirPickerState::open(dir));
    state.mode = Mode::PickWorkspaceDir;
}

fn close_dir_picker(state: &mut AppState) {
    state.dir_picker = None;
    state.mode = if state.active.is_some() {
        Mode::Terminal
    } else {
        Mode::Navigate
    };
}

/// Enter on the highlighted row: create a workspace there, or step out on "..".
fn activate(state: &mut AppState) {
    let Some(picker) = &mut state.dir_picker else {
        return;
    };
    match picker.chosen_path() {
        Some(path) => {
            state.request_new_workspace_cwd = Some(path);
            close_dir_picker(state);
        }
        None => {
            picker.ascend();
        }
    }
}

pub(crate) fn handle_dir_picker_key(state: &mut AppState, key: KeyEvent) {
    let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
    match key.code {
        KeyCode::Esc => {
            let cleared = state
                .dir_picker
                .as_mut()
                .is_some_and(|picker| !std::mem::take(&mut picker.query).is_empty());
            if cleared {
                if let Some(picker) = &mut state.dir_picker {
                    picker.selected = 0;
                }
            } else {
                close_dir_picker(state);
            }
        }
        KeyCode::Enter => activate(state),
        KeyCode::Up => {
            if let Some(picker) = &mut state.dir_picker {
                picker.move_prev();
            }
        }
        KeyCode::Down => {
            if let Some(picker) = &mut state.dir_picker {
                picker.move_next();
            }
        }
        KeyCode::Right | KeyCode::Tab => {
            if let Some(picker) = &mut state.dir_picker {
                picker.descend();
            }
        }
        KeyCode::Left => {
            if let Some(picker) = &mut state.dir_picker {
                picker.ascend();
            }
        }
        KeyCode::Backspace => {
            if let Some(picker) = &mut state.dir_picker {
                if !picker.pop_query() {
                    picker.ascend();
                }
            }
        }
        KeyCode::Char('p') if ctrl => {
            if let Some(picker) = &mut state.dir_picker {
                picker.move_prev();
            }
        }
        KeyCode::Char('n') if ctrl => {
            if let Some(picker) = &mut state.dir_picker {
                picker.move_next();
            }
        }
        KeyCode::Char(ch) if !ctrl => {
            if let Some(picker) = &mut state.dir_picker {
                picker.push_query(ch);
            }
        }
        _ => {}
    }
}

pub(crate) fn insert_dir_picker_text(state: &mut AppState, text: &str) {
    if let Some(picker) = &mut state.dir_picker {
        for ch in text.chars().filter(|ch| !ch.is_control()) {
            picker.push_query(ch);
        }
    }
}

/// Row index under the cursor inside the picker's list area.
pub(crate) fn dir_picker_row_at(state: &AppState, col: u16, row: u16) -> Option<usize> {
    let picker = state.dir_picker.as_ref()?;
    let list = crate::ui::dir_picker_list_rect(state)?;
    if col < list.x || col >= list.x + list.width || row < list.y || row >= list.y + list.height {
        return None;
    }
    let idx = crate::ui::dir_picker_scroll_start(picker, list.height as usize)
        + (row - list.y) as usize;
    (idx < picker.rows().len()).then_some(idx)
}

/// Left click inside the picker: a row activates, anything outside cancels.
pub(crate) fn handle_dir_picker_click(state: &mut AppState, col: u16, row: u16) {
    if let Some(idx) = dir_picker_row_at(state, col, row) {
        if let Some(picker) = &mut state.dir_picker {
            picker.selected = idx;
        }
        // A folder row walks in; "here" and ".." do what Enter does.
        let descend = matches!(
            state
                .dir_picker
                .as_ref()
                .and_then(DirPickerState::selected_row),
            Some(DirPickerRow::Dir(_))
        );
        if descend {
            if let Some(picker) = &mut state.dir_picker {
                picker.descend();
            }
        } else {
            activate(state);
        }
        return;
    }
    if !inside_popup(state, col, row) {
        close_dir_picker(state);
    }
}

fn inside_popup(state: &AppState, col: u16, row: u16) -> bool {
    let popup: Rect = crate::ui::dir_picker_popup_rect(state).unwrap_or_default();
    popup.width > 0
        && col >= popup.x
        && col < popup.x + popup.width
        && row >= popup.y
        && row < popup.y + popup.height
}
