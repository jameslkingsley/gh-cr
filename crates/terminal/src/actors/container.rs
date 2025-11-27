use std::sync::Arc;

use ratatui::{
    buffer::Buffer,
    crossterm::terminal::size,
    layout::{Rect, Size},
    widgets::{StatefulWidget, StatefulWidgetRef},
};
use tui_scrollview::{ScrollView, ScrollViewState};

use crate::{actors::Actor, app::Context};

#[derive(Debug)]
pub struct Container<'a> {
    pub actors: &'a [Box<dyn Actor>],
}

impl StatefulWidgetRef for Container<'_> {
    type State = (ScrollViewState, Arc<Context>);

    // use ratatui::{Frame, layout::Rect, widgets::Widget};
    // pub fn render_widgets<W: Widget>(
    //     f: &mut Frame,
    //     area: Rect,
    //     widgets: &[W],
    //     heights: impl Fn(&W) -> u16,
    // ) {
    //     let mut y = area.y;

    //     for w in widgets {
    //         let h = heights(w);
    //         if y + h > area.y + area.height {
    //             break; // stop when out of screen
    //         }

    //         let rect = Rect {
    //             x: area.x,
    //             y,
    //             width: area.width,
    //             height: h,
    //         };

    //         f.render_widget(w, rect);
    //         y += h;
    //     }
    // }

    fn render_ref(&self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        let size = size().expect("terminal size error");
        let content_size = Size::new(size.0.min(80), size.1);
        let mut scroll_view = ScrollView::new(content_size)
            .scrollbars_visibility(tui_scrollview::ScrollbarVisibility::Always);

        for actor in self.actors {
            actor.render_ref(scroll_view.area(), scroll_view.buf_mut(), &mut state.1);
        }

        scroll_view.render(area, buf, &mut state.0);
    }
}
