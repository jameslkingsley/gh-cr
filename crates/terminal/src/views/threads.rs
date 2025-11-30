use std::u16;

use ratatui::{
    buffer::Buffer,
    layout::{Constraint, Layout, Rect},
    style::Color,
    text::Text,
    widgets::{StatefulWidget, StatefulWidgetRef, Widget},
};
use syntect_assets::assets::HighlightingAssets;
use tui_syntax_highlight::Highlighter;

use crate::{
    context::Context,
    states::AppState,
    utils::blit_content,
    widgets::{pr_header::PullRequestHeader, spinner::Spinner},
};

#[derive(Debug)]
pub struct ThreadsView<'ctx> {
    pub ctx: &'ctx Context,
}

impl StatefulWidgetRef for ThreadsView<'_> {
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
        .areas(area);

        let [_, header] =
            Layout::horizontal([Constraint::Length(2), Constraint::Fill(1)]).areas(header_wide);

        let [_, footer] =
            Layout::horizontal([Constraint::Length(2), Constraint::Fill(1)]).areas(footer_wide);

        let [content_viewport, _, _side] = Layout::horizontal([
            Constraint::Length(82),
            Constraint::Length(4),
            Constraint::Fill(1),
        ])
        .areas(main);

        let content_height = self.content_height(content_viewport);
        state
            .scroll
            .update_scrollbar_state(content_height, content_viewport.height);

        let pr_header = PullRequestHeader { ctx: self.ctx };
        pr_header.render_ref(header, buf, state);
        Text::raw("Footer line").render(footer, buf);

        let virtual_height = content_height.max(1);
        let mut content_buf =
            Buffer::empty(Rect::new(0, 0, content_viewport.width, virtual_height));

        if state.show_diffs {
            self.render_diff(content_buf.area, &mut content_buf, state);
        }

        self.render_comments(content_buf.area, &mut content_buf, state);

        blit_content(&content_buf, content_viewport, buf, state.scroll.offset);
    }
}

impl ThreadsView<'_> {
    fn diff_height(&self) -> u16 {
        let Ok((_, comments)) = self.ctx.threads.current_thread() else {
            return 0;
        };

        if comments.is_empty() {
            return 0;
        }

        let diff = comments[0].diff_hunk.lines().count();

        u16::try_from(diff).unwrap_or(u16::MAX)
    }

    fn render_diff(&self, area: Rect, buf: &mut Buffer, state: &mut AppState) {
        let Ok((_, comments)) = self.ctx.threads.current_thread() else {
            Spinner::new()
                .left_margin(2)
                .label("Fetching comments")
                .render(area, buf, state);
            return;
        };

        if comments.is_empty() {
            return;
        }

        let diff = &comments[0].diff_hunk;

        let assets = HighlightingAssets::from_binary();
        let theme = assets.get_theme("Nord").clone();
        let highlighter = Highlighter::new(theme);
        let syntax_set = assets.get_syntax_set().unwrap();
        let syntax = syntax_set
            .find_syntax_by_token("diff")
            .unwrap_or(syntax_set.find_syntax_plain_text());

        let highlight = highlighter
            .override_background(Color::Reset)
            .line_numbers(false)
            .highlight_lines(diff.lines(), syntax, &syntax_set)
            .unwrap();

        highlight.render(area, buf);
    }

    fn render_comments(&self, area: Rect, buf: &mut Buffer, state: &mut AppState) {
        let Ok((_, comments)) = self.ctx.threads.current_thread() else {
            Spinner::new()
                .left_margin(2)
                .label("Fetching comments")
                .render(area, buf, state);
            return;
        };

        if comments.is_empty() {
            return;
        }

        const GAP: u16 = 1;

        let mut y = 0;

        for (idx, comment) in comments.iter().enumerate() {
            if y >= area.height {
                break;
            }

            let height = comment.layout_height(area.width);
            let remaining_height = area.height.saturating_sub(y);
            if remaining_height == 0 {
                break;
            }

            let comment_area =
                Rect::new(area.x, area.y + y, area.width, height.min(remaining_height));

            comment.render_ref(comment_area, buf, state);
            y = y.saturating_add(height);

            if idx + 1 < comments.len() && y < area.height {
                y = y.saturating_add(GAP.min(area.height.saturating_sub(y)));
            }
        }
    }

    fn content_height(&self, area: Rect) -> u16 {
        const GAP: usize = 1;

        let Ok((_, comments)) = self.ctx.threads.current_thread() else {
            return area.height;
        };

        if comments.is_empty() {
            return area.height;
        }

        let diff = self.diff_height();
        let gaps = comments.len().saturating_sub(1) * GAP;

        comments
            .iter()
            .fold(gaps as u16, |acc, comment| {
                acc.saturating_add(comment.layout_height(area.width))
            })
            .min(u16::MAX)
            + diff
            + 2
    }
}
