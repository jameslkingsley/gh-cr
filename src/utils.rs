use std::fmt::Write;

use crossterm::style::{Color, Stylize};

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
