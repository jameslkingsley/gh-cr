use std::{env, fs, io::Write, process::Command};

use anyhow::{Result, anyhow};
use ratatui::{
    buffer::{Buffer, Cell},
    layout::Rect,
    style::Style,
    text::{Line, Span, Text},
};
use tempfile::NamedTempFile;

pub fn blit_content(src: &Buffer, viewport: Rect, dest: &mut Buffer, scroll_offset: usize) {
    let blank = Cell::default();
    let src_height = src.area.height as usize;

    for y in 0..viewport.height {
        let dst_y = viewport.y + y;
        let src_y = scroll_offset.saturating_add(y as usize);

        for x in 0..viewport.width {
            let dst_x = viewport.x + x;
            let cell = if src_y < src_height {
                src.cell((x, src_y as u16)).cloned().unwrap_or_default()
            } else {
                blank.clone()
            };
            dest[(dst_x, dst_y)] = cell;
        }
    }
}

pub fn wrap_markdown_body(
    body: &str,
    width: usize,
    output: &mut Text,
    text_style: Style,
    code_style: Style,
) {
    let mut in_code_block = false;
    let mut current_text = String::with_capacity(body.len());

    for line in body.lines() {
        if line.trim_start().starts_with("```") {
            // Process accumulated text before code block
            if !current_text.is_empty() && !in_code_block {
                for wrapped_line in textwrap::wrap(&current_text, width) {
                    output.push_line(Line::raw(wrapped_line.into_owned()).style(text_style));
                }
                current_text.clear();
            }

            // Toggle code block state and add the line as-is
            in_code_block = !in_code_block;
            output.push_line(Line::raw(line.to_owned()).style(code_style));
        } else if in_code_block {
            // In code block: add line as-is without wrapping
            output.push_line(Line::raw(line.to_owned()).style(code_style));
        } else {
            // Not in code block: accumulate text for wrapping
            if !current_text.is_empty() {
                current_text.push('\n');
            }
            current_text.push_str(line);
        }
    }

    // Process any remaining accumulated text
    if !current_text.is_empty() {
        for wrapped_line in textwrap::wrap(&current_text, width) {
            output.push_line(Line::raw(wrapped_line.into_owned()).style(text_style));
        }
    }
}

pub fn stylize_block(text: &mut Text, style: Style) {
    let line_len = text.lines.len();
    for (index, line) in text.iter_mut().enumerate() {
        let block = match index {
            0 if line_len == 1 => "",
            0 if line_len > 1 => "╭ ",
            _ if index + 1 == line_len => "╰ ",
            _ => "│ ",
        };

        line.spans.insert(0, Span::raw(block).style(style));
    }
}

pub fn suspend_for_editor(initial_contents: String) -> Result<String> {
    // leave_terminal()?;

    let editor = env::var("EDITOR").unwrap_or_else(|_| "vim".into());

    let mut tempfile = NamedTempFile::new()?;
    tempfile.write_all(initial_contents.as_bytes())?;
    tempfile.flush()?;

    let status = Command::new(&editor).arg(tempfile.path()).status()?;
    if !status.success() {
        return Err(anyhow!("editor exited with {}", status));
    }

    let body = fs::read_to_string(tempfile.path())?;

    // enter_terminal()?;

    Ok(body)
}
