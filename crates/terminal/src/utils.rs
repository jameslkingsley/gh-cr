use std::{
    env,
    fmt::{Display, Write},
    fs,
    io::{Write as _, stdout},
    process::Command,
};

use anyhow::{Result, anyhow};
use crossterm::{
    cursor::{Hide, Show},
    event::DisableMouseCapture,
    execute,
    style::{Color, Stylize},
    terminal::{
        Clear, ClearType, EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode,
        enable_raw_mode,
    },
};
use tempfile::NamedTempFile;

pub fn write_stylized_block(buf: &mut String, block: String, color: Color) -> std::fmt::Result {
    let lines = block.lines().collect::<Vec<_>>();

    if lines.is_empty() {
        writeln!(buf, "{}", "│".with(color))?;
        return Ok(());
    }

    for (i, line) in lines.iter().enumerate() {
        let block = match i {
            0 if lines.len() == 1 => "",
            0 if lines.len() > 1 => "╭",
            _ if i + 1 == lines.len() => "╰",
            _ => "│",
        };

        if line.is_empty() {
            writeln!(buf, "{}", block.with(color))?;
        } else {
            writeln!(buf, "{} {}", block.with(color), line)?;
        }
    }

    Ok(())
}

pub fn hyperlink<Text: Display, Url: Display>(
    buf: &mut String,
    text: Text,
    url: Url,
) -> Result<(), std::fmt::Error> {
    write!(buf, "\x1b]8;;{url}\x1b\\{text}\x1b]8;;\x1b\\")
}

pub fn enter_terminal() -> Result<()> {
    let mut out = stdout();
    enable_raw_mode()?;
    execute!(
        out,
        EnterAlternateScreen,
        Clear(ClearType::All),
        Hide,
        DisableMouseCapture
    )?;
    Ok(())
}

pub fn leave_terminal() -> Result<()> {
    let mut out = stdout();
    disable_raw_mode().ok();
    execute!(out, DisableMouseCapture, Show, LeaveAlternateScreen)?;
    Ok(())
}

pub fn suspend_for_editor(initial_contents: String) -> Result<String> {
    leave_terminal()?;

    let editor = env::var("EDITOR").unwrap_or_else(|_| "vim".into());

    let mut tempfile = NamedTempFile::new()?;
    tempfile.write_all(initial_contents.as_bytes())?;
    tempfile.flush()?;

    let status = Command::new(&editor).arg(tempfile.path()).status()?;
    if !status.success() {
        return Err(anyhow!("editor exited with {}", status));
    }

    let body = fs::read_to_string(tempfile.path())?;

    enter_terminal()?;

    Ok(body)
}
