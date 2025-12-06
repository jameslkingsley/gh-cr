use std::u16;

use ratatui::{
    buffer::Buffer,
    layout::{Constraint, Flex, Layout, Rect},
    style::*,
    text::{Line, Span, Text},
    widgets::{Paragraph, StatefulWidget, StatefulWidgetRef, Widget},
};

use crate::{
    context::Context,
    states::AppState,
    utils::{sanitised_markdown, stylize_block, wrap_markdown_body},
    widgets::{
        comment::comment_into_text, control_hints::ControlHints, pr_header::PullRequestHeader,
        spinner::Spinner,
    },
};

/// The standard conversations view of a pull request
#[derive(Debug)]
pub struct ConversationsView<'ctx> {
    pub ctx: &'ctx Context,
}

impl<'ctx> StatefulWidgetRef for ConversationsView<'ctx> {
    type State = AppState;

    fn render_ref(&self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        let [_, header_wide, _, main, _, footer_wide] = Layout::vertical([
            Constraint::Length(1), // Spacer
            Constraint::Length(2), // Header
            Constraint::Length(1), // Spacer
            Constraint::Fill(1),   // Main
            Constraint::Length(1), // Spacer
            Constraint::Length(2), // Footer
        ])
        .flex(Flex::Start)
        .areas(area);

        if state.expand_context {
            let [main, _, _side] = Layout::horizontal([
                Constraint::Length(82),
                Constraint::Length(4),
                Constraint::Fill(1),
            ])
            .areas(main);

            let mut content = Text::default();
            self.render_description(&mut content, main);
            content.push_line("");
            self.render_comments(&mut content, main);

            state.scroll.content_length(content.lines.len());

            Paragraph::new(content)
                .scroll((state.scroll.vertical as u16, 0))
                .render(main, buf);
        } else {
            let [main, _, _side] = Layout::horizontal([
                Constraint::Length(82),
                Constraint::Length(4),
                Constraint::Fill(1),
            ])
            .areas(main);

            let mut content = Text::default();
            self.render_minimised_description(&mut content, main);
            content.push_line("");
            self.render_comments(&mut content, main);

            state.scroll.content_length(content.lines.len());

            Paragraph::new(content)
                .scroll((state.scroll.vertical as u16, 0))
                .render(main, buf);
        }

        let [_, header] =
            Layout::horizontal([Constraint::Length(2), Constraint::Fill(1)]).areas(header_wide);

        let pr_header = PullRequestHeader {
            ctx: self.ctx,
            show_thread_info: false,
        };
        pr_header.render_ref(header, buf, state);

        let [_, footer_full, _] = Layout::horizontal([
            Constraint::Length(2),
            Constraint::Length(82),
            Constraint::Fill(1),
        ])
        .areas(footer_wide);

        let [footer_left, footer_right] =
            Layout::horizontal([Constraint::Fill(1), Constraint::Fill(1)])
                .flex(Flex::Start)
                .areas(footer_full);

        ControlHints::default().render(footer_left, buf, state);

        if self.ctx.is_working() {
            Spinner::new()
                .label("Working")
                .render(footer_right, buf, state);
        }
    }
}

impl<'ctx> ConversationsView<'ctx> {
    fn render_description(&self, text: &mut Text, area: Rect) {
        let Some(pr) = self.ctx.pr.as_ref() else {
            return;
        };

        let Some(mut body) = pr.body.as_ref().cloned() else {
            return;
        };

        let mut lines: Vec<Line> = Vec::new();

        if body.trim().is_empty() {
            body = "Empty description".to_string();
        }

        let body = sanitised_markdown(&body);

        wrap_markdown_body(
            &body,
            wrap_width(area.width),
            &mut lines,
            Style::new().gray(),
            Style::new().dark_gray(),
        );

        lines.insert(0, Line::styled("PR Description", Style::new().dark_gray()));

        stylize_block(&mut lines, Style::new().dark_gray());

        text.extend(lines);
    }

    fn render_minimised_description(&self, text: &mut Text, area: Rect) {
        let Some(pr) = self.ctx.pr.as_ref() else {
            return;
        };

        let Some(body) = pr.body.as_ref() else {
            return;
        };

        let body = textwrap::wrap(body, wrap_width(area.width));

        text.extend([Line::from_iter([
            Span::raw("  "),
            Span::styled(
                format!("({} lines hidden, ", body.len()),
                Style::new().dark_gray(),
            ),
            Span::styled("d", Style::new().gray()),
            Span::styled(" to show)", Style::new().dark_gray()),
        ])]);
    }

    fn render_comments(&'ctx self, text: &mut Text<'ctx>, area: Rect) {
        let comments = self.ctx.threads.issue_comments();

        if comments.is_empty() || true {
            text.push_line(Line::from_iter([
                Span::raw("  "),
                Span::styled("No comments", Style::new().gray()),
            ]));
            return;
        }

        for comment in comments {
            comment_into_text(text, comment, area.width);
            text.push_line("");
        }
    }
}

pub fn wrap_width(area_width: u16) -> usize {
    // Stylize block consumes 2 columns; clamp to a sensible range.
    let available = area_width.saturating_sub(2).max(1);
    usize::from(available).min(80)
}
