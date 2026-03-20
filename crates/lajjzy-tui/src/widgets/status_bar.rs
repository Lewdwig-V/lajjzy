use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Style};
use ratatui::text::Line;
use ratatui::widgets::Widget;

use lajjzy_core::types::ChangeDetail;

pub struct StatusBarWidget<'a> {
    change_id: Option<&'a str>,
    detail: Option<&'a ChangeDetail>,
    error: Option<&'a str>,
}

impl<'a> StatusBarWidget<'a> {
    pub fn new(
        change_id: Option<&'a str>,
        detail: Option<&'a ChangeDetail>,
        error: Option<&'a str>,
    ) -> Self {
        Self {
            change_id,
            detail,
            error,
        }
    }
}

impl Widget for StatusBarWidget<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if area.height == 0 {
            return;
        }

        if let Some(err) = self.error {
            let style = Style::default().fg(Color::Red);
            let line = Line::styled(err, style);
            buf.set_line(area.x, area.y, &line, area.width);
            return;
        }

        if let Some(detail) = self.detail {
            let change_id = self.change_id.unwrap_or("???");
            let line1 = format!(
                "{} {}  {} <{}>",
                change_id, detail.commit_id, detail.author, detail.email
            );
            buf.set_line(area.x, area.y, &Line::raw(&line1), area.width);

            if area.height > 1 {
                let bookmarks_str = if detail.bookmarks.is_empty() {
                    String::new()
                } else {
                    format!("  [{}]", detail.bookmarks.join(", "))
                };
                let desc = if detail.description.is_empty() {
                    "(no description)"
                } else {
                    &detail.description
                };
                let line2 = format!("{desc}{bookmarks_str}");
                buf.set_line(area.x, area.y + 1, &Line::raw(&line2), area.width);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use lajjzy_core::types::ChangeDetail;

    fn sample_detail() -> ChangeDetail {
        ChangeDetail {
            commit_id: "aaa11".into(),
            author: "alice".into(),
            email: "alice@example.com".into(),
            timestamp: "2m ago".into(),
            description: "fix: parser bug".into(),
            bookmarks: vec!["main".into()],
            is_empty: false,
            has_conflict: false,
        }
    }

    #[test]
    fn renders_change_detail() {
        let detail = sample_detail();
        let widget = StatusBarWidget::new(Some("abc12"), Some(&detail), None);
        let area = Rect::new(0, 0, 60, 2);
        let mut buf = Buffer::empty(area);
        widget.render(area, &mut buf);

        let line0: String = (0..60)
            .map(|x| buf[(x, 0)].symbol().chars().next().unwrap_or(' '))
            .collect();
        assert!(line0.contains("abc12"));
        assert!(line0.contains("alice"));

        let line1: String = (0..60)
            .map(|x| buf[(x, 1)].symbol().chars().next().unwrap_or(' '))
            .collect();
        assert!(line1.contains("fix: parser bug"));
        assert!(line1.contains("[main]"));
    }

    #[test]
    fn renders_error_in_red() {
        let widget = StatusBarWidget::new(None, None, Some("Refresh failed: timeout"));
        let area = Rect::new(0, 0, 40, 2);
        let mut buf = Buffer::empty(area);
        widget.render(area, &mut buf);

        let line0: String = (0..40)
            .map(|x| buf[(x, 0)].symbol().chars().next().unwrap_or(' '))
            .collect();
        assert!(line0.contains("Refresh failed"));
        assert_eq!(buf[(0, 0)].style().fg, Some(Color::Red));
    }

    #[test]
    fn renders_nothing_when_no_detail_and_no_error() {
        let widget = StatusBarWidget::new(None, None, None);
        let area = Rect::new(0, 0, 40, 2);
        let mut buf = Buffer::empty(area);
        widget.render(area, &mut buf);
        // Buffer should be empty (all spaces)
        let line0: String = (0..40)
            .map(|x| buf[(x, 0)].symbol().chars().next().unwrap_or(' '))
            .collect();
        assert_eq!(line0.trim(), "");
    }
}
