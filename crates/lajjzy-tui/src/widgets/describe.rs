use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::Line;
use ratatui::widgets::{Block, Borders, Widget};
use tui_textarea::TextArea;

pub struct DescribeWidget<'a> {
    editor: &'a TextArea<'static>,
}

impl<'a> DescribeWidget<'a> {
    pub fn new(editor: &'a TextArea<'static>) -> Self {
        Self { editor }
    }
}

impl Widget for DescribeWidget<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Yellow))
            .title(" Describe (Ctrl-S save | Esc cancel | Shift-E editor) ");
        let inner = block.inner(area);
        block.render(area, buf);

        // Render TextArea content manually (tui-textarea 0.7 uses ratatui 0.29,
        // so its Widget impl is incompatible with our ratatui 0.30).
        let lines = self.editor.lines();
        let (cursor_row, cursor_col) = self.editor.cursor();
        let height = inner.height as usize;
        let width = inner.width as usize;

        for (row, line_idx) in (0..height).zip(0..lines.len()) {
            let text = &lines[line_idx];
            let line = Line::raw(text);
            #[allow(clippy::cast_possible_truncation)]
            let y = inner.y + row as u16;
            buf.set_line(inner.x, y, &line, inner.width);

            // Highlight the cursor position
            if line_idx == cursor_row {
                #[allow(clippy::cast_possible_truncation)]
                let cx = inner.x + (cursor_col as u16).min(inner.width.saturating_sub(1));
                if (cx as usize) < inner.x as usize + width {
                    buf[(cx, y)].set_style(
                        Style::default()
                            .fg(Color::Black)
                            .bg(Color::White)
                            .add_modifier(Modifier::BOLD),
                    );
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renders_describe_widget() {
        let editor = TextArea::new(vec!["hello world".to_string()]);
        let widget = DescribeWidget::new(&editor);
        let area = Rect::new(0, 0, 60, 6);
        let mut buf = Buffer::empty(area);
        widget.render(area, &mut buf);

        // Check the border title is present
        let top_line: String = (0..60)
            .map(|x| buf[(x, 0)].symbol().chars().next().unwrap_or(' '))
            .collect();
        assert!(top_line.contains("Describe"));
        assert!(top_line.contains("Ctrl-S"));
    }

    #[test]
    fn renders_multiline_content() {
        let editor = TextArea::new(vec!["line one".to_string(), "line two".to_string()]);
        let widget = DescribeWidget::new(&editor);
        let area = Rect::new(0, 0, 40, 6);
        let mut buf = Buffer::empty(area);
        widget.render(area, &mut buf);

        // Content should be inside the border (row 1)
        let line1: String = (0..40)
            .map(|x| buf[(x, 1)].symbol().chars().next().unwrap_or(' '))
            .collect();
        assert!(line1.contains("line one"));
    }
}
