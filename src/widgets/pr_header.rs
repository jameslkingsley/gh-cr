use chrono_humanize::Humanize;
use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Style, Stylize},
    text::{Line, Span, Text},
    widgets::{Paragraph, StatefulWidgetRef, Widget},
};

use crate::{context::Context, states::AppState};

#[allow(dead_code)]
#[derive(Debug)]
pub struct PullRequestHeader<'ctx> {
    pub ctx: &'ctx Context,
}

impl StatefulWidgetRef for PullRequestHeader<'_> {
    type State = AppState;

    fn render_ref(&self, area: Rect, buf: &mut Buffer, _state: &mut Self::State) {
        let Some(pr) = self.ctx.pr.as_ref() else {
            return;
        };

        let mut lines: Vec<Line> = Vec::new();

        let title = pr.title.as_deref().unwrap_or("Pull Request");
        lines.push(Line::from_iter([
            // TODO:
            // - Don't like the look of this
            // - Open/closed isn't informative enough
            // pr.state
            //     .as_ref()
            //     .map(|state| match state {
            //         IssueState::Open => Span::styled(" open ", Style::new().black().on_green()),
            //         IssueState::Closed => {
            //             Span::styled(" closed ", Style::new().black().on_magenta())
            //         }
            //         _ => Span::default(),
            //     })
            //     .unwrap_or(Span::default()),
            // Span::raw(" "),
            Span::styled(title, Style::default().white().bold()),
            Span::raw(" "),
            Span::styled(format!("(#{})", pr.number), Style::default().dark_gray()),
        ]));

        let mut meta: Vec<Span> = Vec::new();

        if let Some(author) = pr.user.as_ref() {
            meta.push(Span::styled("› ", Style::default().dark_gray()));

            meta.push(Span::styled(author.login.as_str(), Style::default().gray()));
        }

        if let Some(updated_at) = pr.updated_at {
            if !meta.is_empty() {
                meta.push(Span::styled(" · ", Style::default().dark_gray()));
            }

            meta.push(Span::styled(updated_at.humanize(), Style::default().gray()));
        }

        if let Some(count) = pr.changed_files {
            if !meta.is_empty() {
                meta.push(Span::styled(" · ", Style::default().dark_gray()));
            }

            let label = format!(
                "{} file{} changed",
                count,
                if count == 1 { "" } else { "s" }
            );
            meta.push(Span::styled(label, Style::default().gray()));
        }

        if !meta.is_empty() {
            lines.push(Line::from(meta));
        }

        if let Some(thread) = self.ctx.threads.current_thread() {
            if let Some(comment) = thread.comments.nodes.first() {
                lines.push(Line::from_iter([
                    Span::styled("› ", Style::default().dark_gray()),
                    Span::styled(
                        format!(
                            "{}/{}",
                            self.ctx.threads.current_thread_index() + 1,
                            self.ctx.threads.thread_len(),
                        ),
                        Style::default().gray(),
                    ),
                    Span::raw(" "),
                    thread
                        .is_resolved
                        .then_some(Span::styled(
                            " resolved ",
                            Style::default().white().on_green(),
                        ))
                        .unwrap_or(Span::styled(
                            " unresolved ",
                            Style::default().black().on_yellow(),
                        )),
                    Span::raw(" "),
                    Span::from(format!(
                        "{}:{}",
                        comment.path,
                        comment.line.unwrap_or(comment.original_line.unwrap_or(1)),
                    ))
                    .style(Style::default().gray()),
                ]));
            }
        }

        lines.push(Line::default());

        Paragraph::new(Text::from(lines)).render(area, buf);
    }
}
