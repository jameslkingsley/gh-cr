use ratatui::{
    prelude::*,
    widgets::{Block, BorderType, Borders, Widget},
};

/// use ratatui::widgets::Paragraph;
/// let thread_paragraph = Paragraph::new("text");
/// let widget = Bordered::new(thread_paragraph);
/// f.render_widget(widget, area);
pub struct Bordered<W> {
    inner: W,
    border_style: Style,
}

impl<W> Bordered<W> {
    pub fn new(inner: W) -> Self {
        Self {
            inner,
            border_style: Style::default().fg(Color::DarkGray),
        }
    }

    pub fn border_style(mut self, style: Style) -> Self {
        self.border_style = style;
        self
    }
}

impl<W> Widget for Bordered<W>
where
    W: Widget,
{
    fn render(self, area: Rect, buf: &mut Buffer) {
        // if not enough height, just render inner
        if area.height < 2 || area.width == 0 {
            self.inner.render(area, buf);
            return;
        }

        // draw full rounded left+top+bottom
        let block = Block::default()
            .borders(Borders::LEFT | Borders::TOP | Borders::BOTTOM)
            .border_type(BorderType::Rounded)
            .border_style(self.border_style);

        let inner_area = block.inner(area);
        block.render(area, buf);

        // wipe horizontal lines, keep only left corners
        let top_y = area.y;
        let bottom_y = area.y + area.height - 1;

        if area.width > 1 {
            let start_x = area.x + 1;
            let end_x = area.x + area.width - 1;

            for x in start_x..=end_x {
                buf.get_mut(x, top_y).set_symbol(" ");
                buf.get_mut(x, bottom_y).set_symbol(" ");
            }
        }

        // render the wrapped widget in the inner rect
        self.inner.render(inner_area, buf);
    }
}
