use std::cell::LazyCell;

use ratatui::{
    buffer::{Buffer, Cell},
    layout::Rect,
    style::{Color, Style},
    text::{Line, Span, Text},
};
use syntect_assets::assets::HighlightingAssets;
use tui_syntax_highlight::{Highlighter, syntect::highlighting::Theme};

pub fn highlight_diff_hunk<'a>(diff: &'a str, theme: Theme) -> Text<'a> {
    let diff = textwrap::dedent(diff)
        .lines()
        .map(|line| {
            let Some(c) = line.chars().next() else {
                return format!(" {line}\n");
            };

            match c {
                '+' => format!("+ {}\n", line.split_once("+").unwrap().1),
                '-' => format!("- {}\n", line.split_once("-").unwrap().1),
                _ => format!(" {line}\n"),
            }
        })
        .collect::<String>();

    // let mut highlight = Text::raw(diff);
    let mut highlight = syntax_highlight(Some("diff"), &diff, theme);

    for line in &mut highlight.lines {
        line.spans.insert(0, Span::raw("  "));
    }

    highlight
}

thread_local! {
    static ASSETS: LazyCell<HighlightingAssets> = LazyCell::new(HighlightingAssets::from_binary);
}

pub fn syntax_highlight<'a>(lang: Option<&str>, src: &str, theme: Theme) -> Text<'a> {
    ASSETS.with(|assets| {
        let highlighter = Highlighter::new(theme);
        let syntax_set = assets.get_syntax_set().unwrap();
        let syntax = lang
            .and_then(|l| syntax_set.find_syntax_by_token(l))
            .unwrap_or(syntax_set.find_syntax_plain_text());

        highlighter
            .override_background(Color::Reset)
            .line_numbers(false)
            .highlight_lines(src.lines(), syntax, &syntax_set)
            .unwrap()
    })
}

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
