use ratatui::{
    buffer::Buffer,
    layout::{Constraint, Layout, Rect},
    widgets::{StatefulWidget, StatefulWidgetRef},
};

use crate::{states::AppState, widgets::spinner::Spinner};

#[derive(Debug)]
pub struct LoadingView;

impl StatefulWidgetRef for LoadingView {
    type State = AppState;

    fn render_ref(&self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        let [_, main, _] = Layout::vertical([
            Constraint::Length(1),
            Constraint::Length(2),
            Constraint::Length(1),
        ])
        .areas(area);

        Spinner::new()
            .left_margin(2)
            .label("Fetching pull request")
            .render(main, buf, state);
    }
}
