//! Fork: drives the working-state indicator animation.
//!
//! The frame counter lives in [`crate::app::state::AppState`] so the renderer
//! can read it; this module only advances it on a fixed cadence while at least
//! one agent is working, and stays completely idle otherwise.

use std::time::{Duration, Instant};

use super::App;
use crate::detect::AgentState;

const WORKING_ANIM_INTERVAL: Duration = Duration::from_millis(120);

impl App {
    fn any_agent_working(&self) -> bool {
        self.state
            .terminals
            .values()
            .any(|terminal| terminal.state == AgentState::Working)
    }

    /// Advances the animation frame when due. Returns whether the view changed.
    pub(crate) fn tick_working_animation(&mut self, now: Instant) -> bool {
        if !self.state.status_indicator_animation || !self.any_agent_working() {
            self.next_working_anim_tick = None;
            return false;
        }

        if self
            .next_working_anim_tick
            .is_some_and(|deadline| now < deadline)
        {
            return false;
        }

        let started = self.next_working_anim_tick.is_none();
        self.next_working_anim_tick = Some(now + WORKING_ANIM_INTERVAL);
        if started {
            // First working agent this cycle: schedule the next frame, but keep
            // the current one so an unrelated render does not skip a step.
            return false;
        }
        self.state.working_anim_frame = self.state.working_anim_frame.wrapping_add(1);
        true
    }

    pub(crate) fn next_working_animation_deadline(&self) -> Option<Instant> {
        self.next_working_anim_tick
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::terminal::TerminalState;
    use crate::workspace::Workspace;

    fn app_with_terminal() -> App {
        let mut app = App::new(
            &crate::config::Config::default(),
            true,
            None,
            tokio::sync::mpsc::unbounded_channel().1,
            crate::api::EventHub::default(),
        );
        let ws = Workspace::test_new("test");
        let pane_id = ws.tabs[0].root_pane;
        let terminal_id = ws.terminal_id(pane_id).expect("terminal id").clone();
        app.state.workspaces.push(ws);
        app.state.active = Some(0);
        app.state.terminals.insert(
            terminal_id.clone(),
            TerminalState::new(terminal_id, "/tmp".into()),
        );
        app
    }

    fn set_state(app: &mut App, state: AgentState) {
        for terminal in app.state.terminals.values_mut() {
            terminal.state = state;
        }
    }

    #[test]
    fn animation_only_runs_while_enabled_and_an_agent_works() {
        let mut app = app_with_terminal();
        let now = Instant::now();
        set_state(&mut app, AgentState::Working);

        // Disabled by config: no frames, no deadline.
        assert!(!app.tick_working_animation(now));
        assert_eq!(app.next_working_animation_deadline(), None);

        app.state.status_indicator_animation = true;
        set_state(&mut app, AgentState::Idle);
        // Enabled, but nothing is working.
        assert!(!app.tick_working_animation(now));
        assert_eq!(app.next_working_animation_deadline(), None);

        set_state(&mut app, AgentState::Working);
        // First tick arms the deadline without skipping a frame.
        assert!(!app.tick_working_animation(now));
        assert_eq!(app.state.working_anim_frame, 0);
        let deadline = app.next_working_animation_deadline().expect("armed");

        assert!(!app.tick_working_animation(deadline - Duration::from_millis(1)));
        assert_eq!(app.state.working_anim_frame, 0);

        assert!(app.tick_working_animation(deadline));
        assert_eq!(app.state.working_anim_frame, 1);

        // Work stops: the animation disarms itself.
        set_state(&mut app, AgentState::Idle);
        assert!(!app.tick_working_animation(deadline + WORKING_ANIM_INTERVAL));
        assert_eq!(app.next_working_animation_deadline(), None);
    }
}
