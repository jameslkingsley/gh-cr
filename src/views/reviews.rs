use std::u16;

use ratatui::{
    buffer::Buffer,
    layout::{Constraint, Flex, Layout, Rect},
    style::{Style, Stylize},
    text::{Line, Span, Text},
    widgets::{Paragraph, StatefulWidget, StatefulWidgetRef, Widget},
};

use crate::{
    context::Context,
    states::AppState,
    utils::highlight_diff_hunk,
    widgets::{
        comment::comment_into_text, control_hints::ControlHints, pr_header::PullRequestHeader,
        spinner::Spinner,
    },
};

/// A view of review threads on the pull request
#[derive(Debug)]
pub struct ReviewsView<'ctx> {
    pub ctx: &'ctx Context,
}

impl<'ctx> StatefulWidgetRef for ReviewsView<'ctx> {
    type State = AppState;

    fn render_ref(&self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        let [_, header_wide, _, main, _, footer_wide] = Layout::vertical([
            Constraint::Length(1), // Spacer
            Constraint::Length(3), // Header
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
            self.render_diff(&mut content, state);
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
            self.render_minimised_diff(&mut content);
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
            show_thread_info: true,
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

impl<'ctx> ReviewsView<'ctx> {
    fn render_diff(&self, text: &mut Text, state: &mut AppState) {
        let Some(thread) = self.ctx.threads.current_thread() else {
            return;
        };

        let comments = &thread.comments.nodes;

        if comments.is_empty() {
            return;
        }

        if comments[0].diff_hunk.is_empty() {
            return;
        }

        highlight_diff_hunk(text, &comments[0].diff_hunk, state.theme.syntect.clone());
    }

    fn render_minimised_diff(&self, text: &mut Text) {
        let Some(thread) = self.ctx.threads.current_thread() else {
            return;
        };

        let comments = &thread.comments.nodes;

        if comments.is_empty() {
            return;
        }

        if comments[0].diff_hunk.is_empty() {
            return;
        }

        let lines = comments[0].diff_hunk.lines().count();

        text.extend([Line::from_iter([
            Span::raw("  "),
            Span::styled(
                format!("({} lines hidden, ", lines),
                Style::new().dark_gray(),
            ),
            Span::styled("d", Style::new().gray()),
            Span::styled(" to show)", Style::new().dark_gray()),
        ])]);
    }

    fn render_comments(&'ctx self, text: &mut Text<'ctx>, area: Rect) {
        let Some(thread) = self.ctx.threads.current_thread() else {
            return;
        };

        let comments = &thread.comments.nodes;

        if comments.is_empty() {
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
