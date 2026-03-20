use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Widget};

use lajjzy_core::types::OpLogEntry;

pub struct OpLogWidget<'a> {
    entries: &'a [OpLogEntry],
    cursor: usize,
    scroll: usize,
}

impl<'a> OpLogWidget<'a> {
    pub fn new(entries: &'a [OpLogEntry], cursor: usize, scroll: usize) -> Self {
        Self {
            entries,
            cursor,
            scroll,
        }
    }
}

impl Widget for OpLogWidget<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Blue))
            .title("Operation Log");
        let inner = block.inner(area);
        block.render(area, buf);

        if self.entries.is_empty() {
            let msg = Line::styled("(no operations)", Style::default().fg(Color::DarkGray));
            buf.set_line(inner.x, inner.y, &msg, inner.width);
            return;
        }

        let highlight = Style::default().add_modifier(Modifier::REVERSED);
        let height = inner.height as usize;

        for (row, idx) in (self.scroll..self.scroll + height).enumerate() {
            if idx >= self.entries.len() {
                break;
            }
            let entry = &self.entries[idx];
            let spans = vec![
                Span::styled(&entry.id, Style::default().fg(Color::Yellow)),
                Span::raw("  "),
                Span::styled(&entry.timestamp, Style::default().fg(Color::Cyan)),
                Span::raw("  "),
                Span::raw(&entry.description),
            ];
            let line = Line::from(spans);
            #[allow(clippy::cast_possible_truncation)]
            let y = inner.y + row as u16;
            buf.set_line(inner.x, y, &line, inner.width);

            if idx == self.cursor {
                for x in inner.x..inner.x + inner.width {
                    buf[(x, y)].set_style(highlight);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_entries() -> Vec<OpLogEntry> {
        vec![
            OpLogEntry {
                id: "abc123".into(),
                description: "create bookmark".into(),
                timestamp: "2h ago".into(),
            },
            OpLogEntry {
                id: "def456".into(),
                description: "snapshot".into(),
                timestamp: "3h ago".into(),
            },
        ]
    }

    #[test]
    fn renders_entries() {
        let entries = sample_entries();
        let widget = OpLogWidget::new(&entries, 0, 0);
        let area = Rect::new(0, 0, 60, 6);
        let mut buf = Buffer::empty(area);
        widget.render(area, &mut buf);

        let line1: String = (0..60)
            .map(|x| buf[(x, 1)].symbol().chars().next().unwrap_or(' '))
            .collect();
        assert!(line1.contains("abc123"));
        assert!(line1.contains("create bookmark"));
    }

    #[test]
    fn renders_empty() {
        let widget = OpLogWidget::new(&[], 0, 0);
        let area = Rect::new(0, 0, 40, 4);
        let mut buf = Buffer::empty(area);
        widget.render(area, &mut buf);

        let line1: String = (0..40)
            .map(|x| buf[(x, 1)].symbol().chars().next().unwrap_or(' '))
            .collect();
        assert!(line1.contains("no operations"));
    }
}
