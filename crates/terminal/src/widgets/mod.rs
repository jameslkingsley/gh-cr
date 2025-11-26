#![allow(dead_code, unused_variables)]

use anyhow::Result;
use crossterm::event::{Event, KeyCode};
use std::fmt::Write;

pub trait Widget: std::fmt::Debug {
    fn tick(&mut self, event: &Event) -> Result<bool>;

    fn render(&self, buf: &mut String) -> Result<()>;
}

#[derive(Debug, Default)]
pub struct Example {
    pub lines: Vec<String>,
}

impl Example {
    pub fn add_line(&mut self) {
        self.lines.push("Here is a line".to_string());
    }

    pub fn remove_line(&mut self) {
        self.lines.pop();
    }
}

impl Widget for Example {
    fn tick(&mut self, event: &Event) -> Result<bool> {
        if let Event::Key(key) = event {
            match key.code {
                KeyCode::Char('a') => self.add_line(),
                KeyCode::Char('r') => self.remove_line(),
                KeyCode::Char('q') => return Ok(true),
                _ => {}
            }
        }

        Ok(false)
    }

    fn render(&self, buf: &mut String) -> Result<()> {
        for line in &self.lines {
            writeln!(buf, "{}", line)?;
        }

        Ok(())
    }
}
