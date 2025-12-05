use ratatui::{
    prelude::{Buffer, Rect},
    style::{Style, Stylize},
    text::{Line, Span},
    widgets::{StatefulWidget, Widget},
};

use crate::states::{AppState, View};

/// Spinner widget with a specified column indent.
#[derive(Debug, Default)]
pub struct ControlHints;

impl StatefulWidget for ControlHints {
    type State = AppState;

    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        let controls = match state.view {
            View::ReviewsAll | View::ReviewsUnresolved => {
                vec![
                    ("←/→", " thread"),
                    ("tab", " view"),
                    ("d", "iff"),
                    ("r", "eply"),
                    ("q", "uit"),
                ]
            }
            View::Convo => vec![
                ("tab", " view"),
                ("d", "escription"),
                ("r", "eply"),
                ("q", "uit"),
            ],
        };

        let mut spans = Vec::with_capacity(controls.len());

        for (key, label) in controls {
            spans.push(Span::raw(key).style(Style::new().white()));
            spans.push(Span::raw(label).style(Style::new().dark_gray()));
            spans.push(Span::raw("  "));
        }

        Line::from(spans).render(area, buf);
    }
}
