use ratatui::{
    layout::{Constraint, Layout},
    prelude::{Buffer, Rect},
    style::{Style, Stylize},
    widgets::StatefulWidget,
};
use throbber_widgets_tui::Throbber;

use crate::states::AppState;

/// Spinner widget with a specified column indent.
pub struct Spinner<'a> {
    inner: Throbber<'a>,
    left_margin: u16,
}

impl Spinner<'_> {
    pub fn new() -> Self {
        Self {
            inner: Throbber::default(),
            left_margin: 0,
        }
    }

    pub fn label<S: AsRef<str>>(mut self, label: S) -> Self {
        self.inner = self.inner.label(label.as_ref().to_owned());
        self
    }

    pub fn left_margin(mut self, left_margin: u16) -> Self {
        self.left_margin = left_margin;
        self
    }
}

impl StatefulWidget for Spinner<'_> {
    type State = AppState;

    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        let main = match self.left_margin {
            0 => area,
            n => {
                let [_, right] =
                    Layout::horizontal([Constraint::Length(n), Constraint::Fill(1)]).areas(area);
                right
            }
        };

        self.inner
            .throbber_set(throbber_widgets_tui::OGHAM_C)
            .throbber_style(Style::new().yellow())
            .style(Style::new().dark_gray())
            .render(main, buf, &mut state.throbber);
    }
}
