use std::collections::HashMap;

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Widget};

use lajjzy_core::types::ChangeDetail;

pub struct BookmarkPickerWidget<'a> {
    bookmarks: &'a [(String, String)], // (name, change_id)
    descriptions: &'a HashMap<String, ChangeDetail>,
    cursor: usize,
}

impl<'a> BookmarkPickerWidget<'a> {
    pub fn new(
        bookmarks: &'a [(String, String)],
        descriptions: &'a HashMap<String, ChangeDetail>,
        cursor: usize,
    ) -> Self {
        Self {
            bookmarks,
            descriptions,
            cursor,
        }
    }
}

impl Widget for BookmarkPickerWidget<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Blue))
            .title("Bookmarks");
        let inner = block.inner(area);
        block.render(area, buf);

        if self.bookmarks.is_empty() {
            let msg = Line::styled("(no bookmarks)", Style::default().fg(Color::DarkGray));
            buf.set_line(inner.x, inner.y, &msg, inner.width);
            return;
        }

        let highlight = Style::default().add_modifier(Modifier::REVERSED);
        let height = inner.height as usize;

        // Auto-follow: ensure cursor is visible in viewport
        let scroll = if height == 0 {
            0
        } else if self.cursor >= height {
            self.cursor - height + 1
        } else {
            0
        };

        for (row, idx) in (scroll..scroll + height).enumerate() {
            if idx >= self.bookmarks.len() {
                break;
            }
            let (name, cid) = &self.bookmarks[idx];
            let desc = self
                .descriptions
                .get(cid)
                .map_or("", |d| d.description.as_str());
            let spans = vec![
                Span::styled(name, Style::default().fg(Color::Magenta)),
                Span::raw("  "),
                Span::styled(desc, Style::default().fg(Color::DarkGray)),
            ];
            let line = Line::from(spans);
            #[expect(clippy::cast_possible_truncation)]
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
    use lajjzy_core::types::ChangeDetail;

    #[test]
    fn renders_bookmarks() {
        let bookmarks = vec![
            ("main".to_string(), "abc".to_string()),
            ("feature".to_string(), "def".to_string()),
        ];
        let mut details = HashMap::new();
        details.insert(
            "abc".to_string(),
            ChangeDetail {
                commit_id: "a1".into(),
                author: "a".into(),
                email: "a@b".into(),
                timestamp: "1m".into(),
                description: "first change".into(),
                bookmarks: vec!["main".into()],
                is_empty: false,
                has_conflict: false,
                files: vec![],
            },
        );
        details.insert(
            "def".to_string(),
            ChangeDetail {
                commit_id: "d1".into(),
                author: "b".into(),
                email: "b@c".into(),
                timestamp: "2m".into(),
                description: "second change".into(),
                bookmarks: vec!["feature".into()],
                is_empty: false,
                has_conflict: false,
                files: vec![],
            },
        );

        let widget = BookmarkPickerWidget::new(&bookmarks, &details, 0);
        let area = Rect::new(0, 0, 60, 6);
        let mut buf = Buffer::empty(area);
        widget.render(area, &mut buf);

        let line1: String = (0..60)
            .map(|x| buf[(x, 1)].symbol().chars().next().unwrap_or(' '))
            .collect();
        assert!(line1.contains("main"));
        assert!(line1.contains("first change"));
    }

    #[test]
    fn renders_empty_bookmarks() {
        let details = HashMap::new();
        let widget = BookmarkPickerWidget::new(&[], &details, 0);
        let area = Rect::new(0, 0, 40, 4);
        let mut buf = Buffer::empty(area);
        widget.render(area, &mut buf);

        let line1: String = (0..40)
            .map(|x| buf[(x, 1)].symbol().chars().next().unwrap_or(' '))
            .collect();
        assert!(line1.contains("no bookmarks"));
    }
}
