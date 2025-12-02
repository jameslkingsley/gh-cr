mod scroll;
mod theme;

use std::{
    env, fs,
    io::{Write, stdout},
    process::Command,
    time::Duration,
};

use anyhow::{Result, anyhow};
use ratatui::{
    Terminal,
    crossterm::{
        cursor::{Hide, Show},
        event::Event,
        execute,
        terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
    },
    prelude::Backend,
};
use tempfile::NamedTempFile;
use throbber_widgets_tui::ThrobberState;

use crate::{
    states::{scroll::ScrollState, theme::ThemeState},
    utils::dirty,
};

#[derive(Debug, Default, Copy, Clone)]
pub enum View {
    #[default]
    Threads,
}

#[allow(dead_code)]
#[derive(Debug)]
pub struct AppState {
    pub scroll: ScrollState,
    pub theme: ThemeState,
    pub view: View,
    pub throbber: ThrobberState,
    pub render_time: Duration,
    pub show_diffs: bool,
    pub force_render: bool,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            scroll: Default::default(),
            theme: Default::default(),
            view: Default::default(),
            throbber: Default::default(),
            render_time: Duration::ZERO,
            show_diffs: true,
            force_render: false,
        }
    }
}

impl AppState {
    /// Force clear the terminal and redraw on the next frame if the state's
    /// `force_render` is set.
    pub fn maybe_force_redraw<B: Backend>(&mut self, terminal: &mut Terminal<B>) -> Result<()> {
        if self.force_render {
            self.force_render = false;
            terminal.clear()?;
            dirty();
        }
        Ok(())
    }

    pub fn tick(&mut self, event: &Event) -> Result<bool> {
        self.scroll.tick(event)
    }

    pub fn suspend_for_editor(&mut self, initial_contents: String) -> Result<String> {
        Self::leave()?;

        // Mark the app state as needing a force render, which will be picked up
        // in the next main loop
        self.force_render = true;
        dirty();

        let editor = env::var("EDITOR").unwrap_or_else(|_| "vim".into());

        let mut tempfile = NamedTempFile::new()?;
        tempfile.write_all(initial_contents.as_bytes())?;
        tempfile.flush()?;

        let status = Command::new(&editor).arg(tempfile.path()).status()?;
        if !status.success() {
            Self::enter()?;
            return Err(anyhow!("editor exited with {}", status));
        }

        let body = fs::read_to_string(tempfile.path())?;

        Self::enter()?;

        Ok(body)
    }

    fn leave() -> Result<()> {
        disable_raw_mode()?;
        execute!(stdout(), LeaveAlternateScreen, Show)?;
        Ok(())
    }

    fn enter() -> Result<()> {
        enable_raw_mode()?;
        execute!(stdout(), EnterAlternateScreen, Hide)?;
        Ok(())
    }
}
