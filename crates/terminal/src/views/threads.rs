use std::u16;

use ratatui::{
    buffer::Buffer,
    layout::{Constraint, Layout, Rect},
    text::Text,
    widgets::{StatefulWidget, StatefulWidgetRef, Widget},
};

use crate::{
    context::Context,
    states::AppState,
    utils::{blit_content, highlight_diff_hunk},
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

        let diff_height = if state.show_diffs {
            self.diff_height()
        } else {
            0
        };

        let content_height = self.content_height(content_viewport, diff_height);
        state
            .scroll
            .update_scrollbar_state(content_height, content_viewport.height);

        let pr_header = PullRequestHeader { ctx: self.ctx };
        pr_header.render_ref(header, buf, state);
        Text::raw("Footer line").render(footer, buf);

        let virtual_height = content_height.max(1);
        let mut content_buf =
            Buffer::empty(Rect::new(0, 0, content_viewport.width, virtual_height));

        let mut y = 0u16;

        if state.show_diffs && diff_height > 0 {
            let diff_area_height = diff_height.min(content_buf.area.height) + 1;
            let diff_area = Rect::new(
                content_buf.area.x,
                content_buf.area.y,
                content_buf.area.width,
                diff_area_height,
            );
            self.render_diff(diff_area, &mut content_buf, state);
            y = y.saturating_add(diff_area_height);
        }

        if y < content_buf.area.height {
            let comments_area = Rect::new(
                content_buf.area.x,
                content_buf.area.y + y,
                content_buf.area.width,
                content_buf.area.height.saturating_sub(y),
            );
            self.render_comments(comments_area, &mut content_buf, state);
        }

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

        let diff = highlight_diff_hunk(&comments[0].diff_hunk, state.theme.syntect.clone());

        diff.render(area, buf);
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

    fn content_height(&self, area: Rect, diff_height: u16) -> u16 {
        const GAP: usize = 1;

        let Ok((_, comments)) = self.ctx.threads.current_thread() else {
            return area.height;
        };

        if comments.is_empty() {
            return area.height;
        }

        let gaps = comments.len().saturating_sub(1) * GAP;

        comments
            .iter()
            .fold(gaps as u16, |acc, comment| {
                acc.saturating_add(comment.layout_height(area.width))
            })
            .min(u16::MAX)
            + diff_height
            + 2
    }
}
