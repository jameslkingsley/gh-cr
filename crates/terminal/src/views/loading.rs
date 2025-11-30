use ratatui::{
    buffer::Buffer,
    layout::{Constraint, Layout, Rect},
    text::Line,
    widgets::{StatefulWidgetRef, Widget},
};

use crate::states::AppState;

#[derive(Debug)]
pub struct LoadingView;

impl StatefulWidgetRef for LoadingView {
    type State = AppState;

    fn render_ref(&self, area: Rect, buf: &mut Buffer, _state: &mut Self::State) {
        let [_, main, _] = Layout::vertical([
            Constraint::Length(1),
            Constraint::Length(2),
            Constraint::Length(1),
        ])
        .areas(area);

        let [_, content] =
            Layout::horizontal([Constraint::Length(2), Constraint::Fill(1)]).areas(main);

        Line::raw("Loading...").render(content, buf);
    }
}
