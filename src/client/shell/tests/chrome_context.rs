use super::*;

#[test]
fn tab_overflow_controls_scroll_the_client_owned_tab_bar() {
    let mut snapshot = snapshot();
    snapshot.tabs.extend((2..=8).map(|number| ClientShellTab {
        tab_id: format!("tab_{number}"),
        workspace_id: "ws_1".into(),
        number,
        label: number.to_string(),
        custom_label: false,
        zoomed: false,
        focused: false,
        agent_status: AgentStatus::Idle,
    }));
    let mut state = ClientShellState::new(ClientShellConfig::from_config(&Config::default()));
    state.set_snapshot(Box::new(snapshot));
    state.set_pane_surface(surface());
    state.compose(80, 20).expect("overflow tab bar");

    assert!(state.hits.tab_scroll_right.width > 0);
    let scroll_right = state.hits.tab_scroll_right;
    let outcome =
        state.handle_raw_events(vec![RawInputEvent::Mouse(crossterm::event::MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: scroll_right.x + 1,
            row: scroll_right.y,
            modifiers: KeyModifiers::empty(),
        })]);
    assert!(outcome.repaint);
    assert_eq!(state.tab_scroll, 1);

    let mut update = state.snapshot.as_deref().expect("snapshot").clone();
    update.focused_tab_id = Some("tab_8".into());
    for tab in &mut update.tabs {
        tab.focused = tab.tab_id == "tab_8";
    }
    state.set_snapshot(Box::new(update));
    state.compose(80, 20).expect("focused overflow tab");
    assert!(state.hits.tabs.iter().any(|(_, tab_id)| tab_id == "tab_8"));

    state.compose(300, 20).expect("tabs without overflow");
    assert_eq!(state.tab_scroll, 0);
    assert_eq!(state.hits.tabs.len(), 8);
    state.compose(80, 20).expect("focused tab after narrowing");
    assert!(state.hits.tabs.iter().any(|(_, tab_id)| tab_id == "tab_8"));
}

#[test]
fn focused_workspace_change_reveals_new_workspace_in_full_sidebar() {
    let mut initial = snapshot();
    let template = initial.workspaces[0].clone();
    initial.workspaces = (1..=12)
        .map(|number| ClientShellWorkspace {
            workspace_id: format!("ws_{number}"),
            number,
            label: format!("space-{number}"),
            branch: None,
            focused: number == 1,
            ..template.clone()
        })
        .collect();

    let mut state = ClientShellState::new(ClientShellConfig::from_config(&Config::default()));
    state.set_snapshot(Box::new(initial));
    state.set_pane_surface(surface());
    state.compose(106, 20).expect("full sidebar");
    assert!(state.hits.workspace_max_scroll > 0);
    assert!(state
        .hits
        .workspaces
        .iter()
        .all(|hit| hit.workspace_id != "ws_12"));

    let mut update = state.snapshot.as_deref().expect("snapshot").clone();
    update.revision = 2;
    update.focused_workspace_id = Some("ws_12".into());
    for workspace in &mut update.workspaces {
        workspace.focused = workspace.workspace_id == "ws_12";
    }
    let mut updated_surface = surface();
    updated_surface.projection_revision = 2;
    state.set_snapshot(Box::new(update));
    state.set_pane_surface(updated_surface);
    state.compose(106, 2).expect("zero-height workspace body");
    assert!(state.reveal_focused_workspace);
    state.compose(106, 20).expect("updated full sidebar");

    assert!(state
        .hits
        .workspaces
        .iter()
        .any(|hit| hit.workspace_id == "ws_12"));
}

#[test]
fn client_owned_sidebar_dividers_resize_live() {
    let mut state = ClientShellState::new(ClientShellConfig::from_config(&Config::default()));
    state.set_snapshot(Box::new(snapshot()));
    state.set_pane_surface(surface());
    state.compose(106, 30).expect("expanded sidebar");
    let workspace_body = state.hits.workspace_body;
    let needless_scroll =
        state.handle_raw_events(vec![RawInputEvent::Mouse(crossterm::event::MouseEvent {
            kind: MouseEventKind::ScrollDown,
            column: workspace_body.x,
            row: workspace_body.y,
            modifiers: KeyModifiers::empty(),
        })]);
    assert_eq!(state.hits.workspace_max_scroll, 0);
    assert_eq!(state.workspace_scroll, 0);
    assert!(!needless_scroll.repaint);
    let width_divider = state.hits.sidebar_divider;
    state.handle_raw_events(vec![RawInputEvent::Mouse(crossterm::event::MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: width_divider.x,
        row: width_divider.y + 2,
        modifiers: KeyModifiers::empty(),
    })]);
    let resize =
        state.handle_raw_events(vec![RawInputEvent::Mouse(crossterm::event::MouseEvent {
            kind: MouseEventKind::Drag(MouseButton::Left),
            column: 31,
            row: width_divider.y + 2,
            modifiers: KeyModifiers::empty(),
        })]);
    assert_eq!(state.sidebar_width, 32);
    assert!(state.sidebar_width_manual);
    assert!(resize.repaint);
    assert!(resize.resize);
    state.handle_raw_events(vec![RawInputEvent::Mouse(crossterm::event::MouseEvent {
        kind: MouseEventKind::Up(MouseButton::Left),
        column: 31,
        row: width_divider.y + 2,
        modifiers: KeyModifiers::empty(),
    })]);

    state.set_pane_surface(surface());
    state.compose(106, 30).expect("resized sidebar");
    let section_divider = state.hits.sidebar_section_divider;
    state.handle_raw_events(vec![RawInputEvent::Mouse(crossterm::event::MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: section_divider.x + 2,
        row: section_divider.y,
        modifiers: KeyModifiers::empty(),
    })]);
    let split = state.handle_raw_events(vec![RawInputEvent::Mouse(crossterm::event::MouseEvent {
        kind: MouseEventKind::Drag(MouseButton::Left),
        column: section_divider.x + 2,
        row: 20,
        modifiers: KeyModifiers::empty(),
    })]);
    assert!(state.sidebar_section_split > 0.6);
    assert!(split.repaint);
    assert!(!split.resize);
}

#[test]
fn context_menus_capture_stable_targets_and_route_actions() {
    let mut state = ClientShellState::new(ClientShellConfig::from_config(&Config::default()));
    state.set_snapshot(Box::new(snapshot()));
    state.set_pane_surface(surface());
    state.compose(106, 20).expect("composed frame");

    let workspace = state.hits.workspaces[0].rect;
    let open_workspace_menu =
        state.handle_raw_events(vec![RawInputEvent::Mouse(crossterm::event::MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Right),
            column: workspace.x + 2,
            row: workspace.y,
            modifiers: KeyModifiers::empty(),
        })]);
    assert!(open_workspace_menu.actions.is_empty());
    assert!(matches!(
        state.overlay,
        Some(ClientShellOverlay::ContextMenu(ClientContextMenuOverlay {
            target: ClientContextMenuTarget::Workspace { ref workspace_id, .. },
            ..
        })) if workspace_id == "ws_1"
    ));
    let workspace_items = match state.overlay.as_ref() {
        Some(ClientShellOverlay::ContextMenu(menu)) => menu.items(),
        _ => panic!("workspace context menu"),
    };
    assert!(workspace_items
        .iter()
        .any(|item| item.action == ClientContextMenuAction::NewWorktree));
    state.compose(106, 20).expect("workspace context menu");
    let rename = state.hits.context_menu_rows[0].0;
    state.handle_raw_events(vec![RawInputEvent::Mouse(crossterm::event::MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: rename.x + 1,
        row: rename.y,
        modifiers: KeyModifiers::empty(),
    })]);
    assert!(matches!(
        state.overlay,
        Some(ClientShellOverlay::Rename(ClientRenameOverlay {
            target: ClientRenameTarget::Workspace { ref workspace_id },
            ..
        })) if workspace_id == "ws_1"
    ));

    state.overlay = None;
    state.compose(106, 20).expect("composed frame");
    let pane = state.hits.panes[0].rect;
    state.handle_raw_events(vec![RawInputEvent::Mouse(crossterm::event::MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Right),
        column: pane.x + 1,
        row: pane.y,
        modifiers: KeyModifiers::empty(),
    })]);
    state.compose(106, 20).expect("pane context menu");
    let split_index = match state.overlay.as_ref() {
        Some(ClientShellOverlay::ContextMenu(menu)) => menu
            .items()
            .iter()
            .position(|item| item.action == ClientContextMenuAction::SplitRight)
            .expect("split right item"),
        _ => panic!("pane context menu"),
    };
    let split = state.hits.context_menu_rows[split_index].0;
    let outcome =
        state.handle_raw_events(vec![RawInputEvent::Mouse(crossterm::event::MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: split.x + 1,
            row: split.y,
            modifiers: KeyModifiers::empty(),
        })]);
    let [ClientShellAction::Endpoint { request, .. }] = &outcome.actions[..] else {
        panic!("pane split context action should use endpoint API");
    };
    assert!(matches!(
        &request.method,
        crate::api::schema::Method::PaneSplit(params)
            if params.target_pane_id.as_deref() == Some("pane_1")
                && params.direction == crate::api::schema::SplitDirection::Right
    ));
}

#[test]
fn global_menu_opens_from_sidebar_and_routes_client_actions() {
    let mut state = ClientShellState::new(ClientShellConfig::from_config(&Config::default()));
    state.set_snapshot(Box::new(snapshot()));
    state.set_pane_surface(surface());
    state.compose(106, 30).expect("shell frame");
    let launcher = state.hits.global_launcher;
    assert_ne!(launcher, Rect::default());

    let open = state.handle_raw_events(vec![RawInputEvent::Mouse(crossterm::event::MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: launcher.x,
        row: launcher.y,
        modifiers: KeyModifiers::empty(),
    })]);
    assert!(open.repaint);
    let menu = state.compose(106, 30).expect("global menu");
    let text = menu
        .cells
        .chunks(menu.width as usize)
        .map(|row| {
            row.iter()
                .map(|cell| cell.symbol.as_str())
                .collect::<String>()
        })
        .collect::<Vec<_>>()
        .join("\n");
    assert!(text.contains("settings"));
    assert!(text.contains("keybinds"));
    assert!(text.contains("reload config"));
    assert!(text.contains("detach"));

    let keybinds = state.hits.global_menu_rows[1].0;
    let help = state.handle_raw_events(vec![RawInputEvent::Mouse(crossterm::event::MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: keybinds.x,
        row: keybinds.y,
        modifiers: KeyModifiers::empty(),
    })]);
    assert!(help.actions.is_empty());
    assert!(matches!(state.overlay, Some(ClientShellOverlay::Help(_))));

    state.overlay = Some(ClientShellOverlay::GlobalMenu(ClientGlobalMenuOverlay {
        highlighted: 3,
    }));
    let detach = state.handle_input_bytes(b"\r");
    assert!(detach.detach);
    assert!(state.overlay.is_none());
}

#[test]
fn new_tab_overlay_owns_text_cursor_and_submits_public_api_request() {
    let mut state = ClientShellState::new(ClientShellConfig::from_config(&Config::default()));
    state.set_snapshot(Box::new(snapshot()));
    state.set_pane_surface(surface());
    let mut open = ClientShellInput::default();
    state.record_binding(
        crate::input::KeybindMatch::Action(crate::input::KeybindAction::NewTab),
        &mut open,
    );
    assert!(open.actions.is_empty());
    let frame = state.compose(106, 20).expect("new tab overlay");
    let text = frame
        .cells
        .chunks(frame.width as usize)
        .map(|row| {
            row.iter()
                .map(|cell| cell.symbol.as_str())
                .collect::<String>()
        })
        .collect::<Vec<_>>()
        .join("\n");
    assert!(text.contains("new tab"));
    assert!(text.contains("save"));
    let restored = frame.to_ratatui_buffer().expect("overlay frame");
    assert!(!restored
        .cell((26, 7))
        .expect("overlay title cell")
        .modifier
        .contains(Modifier::DIM));
    assert!(frame.cursor.as_ref().is_some_and(|cursor| cursor.visible));

    assert!(state.handle_input_bytes(b"logs").actions.is_empty());
    let create = state.handle_input_bytes(b"\r");
    let [ClientShellAction::Endpoint { request, .. }] = &create.actions[..] else {
        panic!("new tab save should use endpoint API");
    };
    assert!(matches!(
        &request.method,
        crate::api::schema::Method::TabCreate(params)
            if params.workspace_id.as_deref() == Some("ws_1")
                && params.label.as_deref() == Some("logs")
    ));
    assert!(state.overlay.is_none());
}

#[test]
fn close_confirmation_error_becomes_client_owned_overlay_and_stable_group_close() {
    let mut state = ClientShellState::new(ClientShellConfig::from_config(&Config::default()));
    state.set_snapshot(Box::new(snapshot()));
    state.set_pane_surface(surface());
    let mut close = ClientShellInput::default();
    state.record_binding(
        crate::input::KeybindMatch::Action(crate::input::KeybindAction::ClosePane),
        &mut close,
    );
    let [ClientShellAction::Endpoint { request, .. }] = &close.actions[..] else {
        panic!("pane close should use endpoint API");
    };
    let request_id = request.id.clone();
    assert!(
        state
            .handle_endpoint_result(
                "boot-1",
                &request_id,
                Err(ClientShellEndpointError {
                    code: Some("confirmation_required".into()),
                    message: "confirmation required".into(),
                }),
            )
            .0
    );
    let frame = state.compose(106, 20).expect("confirmation overlay");
    let text = frame
        .cells
        .chunks(frame.width as usize)
        .map(|row| {
            row.iter()
                .map(|cell| cell.symbol.as_str())
                .collect::<String>()
        })
        .collect::<Vec<_>>()
        .join("\n");
    assert!(text.contains("Close workspace?"));
    assert!(text.contains("1 pane"));

    let confirm = state.handle_input_bytes(b"\r");
    let [ClientShellAction::Endpoint { request, .. }] = &confirm.actions[..] else {
        panic!("confirmation should use endpoint API");
    };
    assert!(matches!(
        &request.method,
        crate::api::schema::Method::WorkspaceClose(params)
            if params.workspace_id == "ws_1" && params.close_group
    ));
}

#[test]
fn workspace_bar_takes_the_top_row_and_switches_spaces_on_click() {
    let mut snapshot = snapshot();
    let template = snapshot.workspaces[0].clone();
    snapshot.workspaces = (1..=2)
        .map(|number| ClientShellWorkspace {
            workspace_id: format!("ws_{number}"),
            number,
            label: format!("space-{number}"),
            focused: number == 1,
            ..template.clone()
        })
        .collect();
    snapshot.agents = vec![ClientShellAgent {
        pane_id: "pane_1".into(),
        workspace_id: "ws_2".into(),
        tab_id: "tab_1".into(),
        name: None,
        display_agent: None,
        agent: None,
        title: None,
        terminal_title: None,
        terminal_title_stripped: None,
        agent_status: AgentStatus::Blocked,
        state_change_seq: 0,
        state_labels: Vec::new(),
        tokens: Vec::new(),
        focused: false,
    }];

    let mut config = Config::default();
    config.ui.workspace_bar = true;
    config.ui.workspace_bar_agent_counts = true;
    let mut state = ClientShellState::new(ClientShellConfig::from_config(&config));
    state.set_snapshot(Box::new(snapshot));
    state.set_pane_surface(surface());
    let frame = state.compose(80, 20).expect("composed with a workspace bar");

    let layout = state.layout(80, 20);
    assert_eq!(layout.workspace_bar, Rect::new(0, 0, 80, 1));
    // The strip pushes the rest of the chrome down and keeps the pane surface below it.
    assert!(layout.sidebar.y >= 1 && layout.pane_surface.y >= 1);

    let bar_hits = state
        .hits
        .workspaces
        .iter()
        .filter(|hit| hit.in_workspace_bar)
        .map(|hit| (hit.rect, hit.workspace_id.clone()))
        .collect::<Vec<_>>();
    assert_eq!(bar_hits.len(), 2);
    assert!(bar_hits.iter().all(|(rect, _)| rect.y == 0));
    assert!(state.hits.workspace_bar_new.width > 0);

    // `ui.workspace_bar_agent_counts` shows the blocked agent waiting in space two.
    let top_row = frame.cells[..80]
        .iter()
        .map(|cell| cell.symbol.as_str())
        .collect::<String>();
    assert!(top_row.contains("space-2 (1)"), "top row: {top_row:?}");
    assert!(!top_row.contains("space-1 ("), "top row: {top_row:?}");

    let second = bar_hits[1].0;
    let outcome = state.handle_raw_events(vec![RawInputEvent::Mouse(MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: second.x + 1,
        row: 0,
        modifiers: KeyModifiers::empty(),
    })]);
    let released = state.handle_raw_events(vec![RawInputEvent::Mouse(MouseEvent {
        kind: MouseEventKind::Up(MouseButton::Left),
        column: second.x + 1,
        row: 0,
        modifiers: KeyModifiers::empty(),
    })]);
    let traffic = format!("{:?}{:?}{:?}{:?}", outcome.requests, outcome.actions, released.requests, released.actions);
    assert!(
        traffic.contains("ws_2"),
        "clicking a strip cell asks the endpoint to focus that workspace: {traffic}"
    );
}

#[test]
fn working_status_animation_only_runs_while_enabled_and_an_agent_works() {
    let mut base = snapshot();
    base.agents = vec![ClientShellAgent {
        pane_id: "pane_1".into(),
        workspace_id: "ws_1".into(),
        tab_id: "tab_1".into(),
        name: None,
        display_agent: None,
        agent: None,
        title: None,
        terminal_title: None,
        terminal_title_stripped: None,
        agent_status: AgentStatus::Working,
        state_change_seq: 0,
        state_labels: Vec::new(),
        tokens: Vec::new(),
        focused: false,
    }];

    let now = std::time::Instant::now();

    // Off by config: no frames, no deadline, even with a working agent.
    let mut off = ClientShellState::new(ClientShellConfig::from_config(&Config::default()));
    off.set_snapshot(Box::new(base.clone()));
    assert!(!off.tick_working_animation(now));
    assert_eq!(off.next_working_anim_tick, None);

    let mut config = Config::default();
    config.ui.status_indicator_animation = true;
    let mut state = ClientShellState::new(ClientShellConfig::from_config(&config));

    // Enabled, but nothing is working.
    let mut resting = base.clone();
    resting.agents[0].agent_status = AgentStatus::Idle;
    state.set_snapshot(Box::new(resting));
    assert!(!state.tick_working_animation(now));
    assert_eq!(state.next_working_anim_tick, None);

    // First tick arms the next frame without skipping a step.
    state.set_snapshot(Box::new(base.clone()));
    assert!(state.tick_working_animation(now));
    assert_eq!(state.working_anim_frame, 0);
    let deadline = state.next_working_anim_tick.expect("armed");

    assert!(!state.tick_working_animation(deadline - std::time::Duration::from_millis(1)));
    assert_eq!(state.working_anim_frame, 0);
    assert!(state.tick_working_animation(deadline));
    assert_eq!(state.working_anim_frame, 1);

    // The strip and sidebar draw the spinner frame, not the resting glyph.
    state.set_pane_surface(surface());
    let mut with_bar = Config::default();
    with_bar.ui.status_indicator_animation = true;
    with_bar.ui.workspace_bar = true;
    let mut bar_state = ClientShellState::new(ClientShellConfig::from_config(&with_bar));
    bar_state.set_snapshot(Box::new(base.clone()));
    bar_state.set_pane_surface(surface());
    bar_state.tick_working_animation(now);
    bar_state.working_anim_frame = 2;
    bar_state.tick_working_animation(now + std::time::Duration::from_secs(1));
    let frame = bar_state.compose(80, 20).expect("composed while working");
    let top_row = frame.cells[..80]
        .iter()
        .map(|cell| cell.symbol.as_str())
        .collect::<String>();
    assert!(top_row.contains('\u{2217}'), "top row: {top_row:?}");

    // Work stops: the animation disarms itself.
    let mut stopped = base;
    stopped.agents[0].agent_status = AgentStatus::Idle;
    state.set_snapshot(Box::new(stopped));
    assert!(!state.tick_working_animation(deadline + std::time::Duration::from_secs(1)));
    assert_eq!(state.next_working_anim_tick, None);
    assert_eq!(state.config.working_anim_frame, None);
}

#[test]
fn command_buttons_draw_in_the_strip_and_the_sidebar_footer() {
    let mut config = Config::default();
    config.ui.workspace_bar = true;
    config.ui.sidebar_git_footer = true;
    config.keys.command = vec![
        crate::config::CommandKeybindConfig {
            key: crate::config::BindingConfig::One("prefix+t".into()),
            command: "tasks-sidebar".into(),
            action_type: crate::config::CommandKeybindType::Shell,
            description: None,
            width: None,
            height: None,
            button: Some("задачи".into()),
            button_position: crate::config::ButtonPosition::Bar,
        },
        crate::config::CommandKeybindConfig {
            key: crate::config::BindingConfig::One("prefix+g".into()),
            command: "git pull".into(),
            action_type: crate::config::CommandKeybindType::Pane,
            description: None,
            width: None,
            height: None,
            button: Some("pull".into()),
            button_position: crate::config::ButtonPosition::Sidebar,
        },
    ];

    let mut state = ClientShellState::new(ClientShellConfig::from_config(&config));
    let mut projection = snapshot();
    projection.workspaces[0].branch = Some("custom".into());
    projection.workspaces[0].git_ahead_behind = Some((2, 3));
    // The client rebuilds its command list from the endpoint, so the buttons only
    // survive because they travel on the wire.
    projection.commands = vec![
        crate::protocol::ClientShellCommand {
            command_id: "tasks-sidebar".into(),
            binding_label: "prefix+t".into(),
            binding_labels: vec!["prefix+t".into()],
            action: crate::protocol::ClientShellCommandAction::Shell,
            description: None,
            button: Some("задачи".into()),
            button_position: crate::config::ButtonPosition::Bar,
        },
        crate::protocol::ClientShellCommand {
            command_id: "git pull".into(),
            binding_label: "prefix+g".into(),
            binding_labels: vec!["prefix+g".into()],
            action: crate::protocol::ClientShellCommandAction::Pane,
            description: None,
            button: Some("pull".into()),
            button_position: crate::config::ButtonPosition::Sidebar,
        },
    ];
    state.set_snapshot(Box::new(projection));
    state.set_pane_surface(surface());
    let frame = state.compose(100, 24).expect("composed with buttons");

    let layout = state.layout(100, 24);
    // The footer claims one row for git and one for the sidebar button.
    assert_eq!(layout.sidebar_footer.height, 2);
    assert_eq!(layout.sidebar_footer.bottom(), 24);
    assert_eq!(layout.sidebar.bottom(), layout.sidebar_footer.y);

    let row = |y: usize| {
        frame.cells[y * 100..(y + 1) * 100]
            .iter()
            .map(|cell| cell.symbol.as_str())
            .collect::<String>()
    };
    assert!(row(0).contains("задачи"), "strip: {:?}", row(0));
    let git_row = row(usize::from(layout.sidebar_footer.y));
    assert!(git_row.contains("custom"), "git row: {git_row:?}");
    assert!(git_row.contains("↓3"), "git row: {git_row:?}");
    assert!(git_row.contains("↑2"), "git row: {git_row:?}");
    assert!(row(23).contains("pull"), "footer buttons: {:?}", row(23));

    // Both buttons are clickable and reach the endpoint as a command invoke.
    assert_eq!(state.hits.command_buttons.len(), 2);
    let bar_button = state
        .hits
        .command_buttons
        .iter()
        .find(|(rect, _)| rect.y == 0)
        .expect("strip button")
        .0;
    let outcome = state.handle_raw_events(vec![RawInputEvent::Mouse(MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: bar_button.x + 1,
        row: 0,
        modifiers: KeyModifiers::empty(),
    })]);
    let traffic = format!("{:?}{:?}", outcome.requests, outcome.actions);
    assert!(
        traffic.contains("tasks-sidebar"),
        "clicking a button invokes its command: {traffic}"
    );
}

#[test]
fn sidebar_divider_reserves_a_gutter_and_tab_contrast_tints_the_row() {
    let plain = ClientShellState::new(ClientShellConfig::from_config(&Config::default()));
    let plain_layout = plain.layout(100, 24);
    assert_eq!(plain_layout.sidebar_gutter.width, 0);
    assert_eq!(plain_layout.pane_surface.x, plain_layout.sidebar.right());

    let mut config = Config::default();
    config.ui.sidebar_divider = true;
    config.ui.tab_bar_contrast = true;
    let mut state = ClientShellState::new(ClientShellConfig::from_config(&config));
    state.set_snapshot(Box::new(snapshot()));
    state.set_pane_surface(surface());
    let frame = state.compose(100, 24).expect("composed with a divider");

    let layout = state.layout(100, 24);
    assert_eq!(layout.sidebar_gutter.width, 1);
    assert_eq!(layout.sidebar_gutter.x, layout.sidebar.right());
    assert_eq!(layout.pane_surface.x, layout.sidebar_gutter.right());

    let gutter_cell = |y: u16| {
        frame.cells[usize::from(y) * 100 + usize::from(layout.sidebar_gutter.x)]
            .symbol
            .clone()
    };
    assert_eq!(gutter_cell(1), "\u{2502}");
    assert_eq!(gutter_cell(layout.sidebar.bottom() - 1), "\u{2502}");

    // The tab row is tinted apart from the panel background the sidebar uses.
    let tab_bg = frame.cells[usize::from(layout.tab_bar.y) * 100 + usize::from(layout.tab_bar.right() - 1)].bg;
    let mut plain_state = ClientShellState::new(ClientShellConfig::from_config(&Config::default()));
    plain_state.set_snapshot(Box::new(snapshot()));
    plain_state.set_pane_surface(surface());
    let plain_frame = plain_state.compose(100, 24).expect("composed plain");
    let plain_layout = plain_state.layout(100, 24);
    let plain_bg = plain_frame.cells
        [usize::from(plain_layout.tab_bar.y) * 100 + usize::from(plain_layout.tab_bar.right() - 1)]
        .bg;
    assert_ne!(tab_bg, plain_bg);
}

#[test]
fn alt_click_in_a_pane_asks_for_the_folder_and_ctrl_click_does_not() {
    let mut state = ClientShellState::new(ClientShellConfig::from_config(&Config::default()));
    state.set_snapshot(Box::new(snapshot()));
    state.set_pane_surface(surface());
    state.compose(120, 30).expect("composed with a pane");
    let inner = state.hits.panes.first().expect("pane hit").inner_rect;

    let click = |state: &mut ClientShellState, modifiers| {
        state.handle_raw_events(vec![RawInputEvent::Mouse(MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: inner.x,
            row: inner.y,
            modifiers,
        })])
    };

    let ctrl = click(&mut state, KeyModifiers::CONTROL);
    let traffic = format!("{:?}{:?}", ctrl.requests, ctrl.actions);
    assert!(
        traffic.contains("PaneLinkActivate") || traffic.contains("pane.link_activate"),
        "ctrl+click asks the endpoint to resolve the link: {traffic}"
    );
    assert!(
        traffic.contains("reveal_dir: false"),
        "ctrl+click follows the link itself: {traffic}"
    );

    let mut with_links = Config::default();
    with_links.ui.pane_file_links = true;
    let mut alt_state = ClientShellState::new(ClientShellConfig::from_config(&with_links));
    alt_state.set_snapshot(Box::new(snapshot()));
    alt_state.set_pane_surface(surface());
    alt_state.compose(120, 30).expect("composed with a pane");
    let alt = click(&mut alt_state, KeyModifiers::ALT);
    let traffic = format!("{:?}{:?}", alt.requests, alt.actions);
    assert!(
        traffic.contains("reveal_dir: true"),
        "alt+click asks for the folder holding the file: {traffic}"
    );
}
