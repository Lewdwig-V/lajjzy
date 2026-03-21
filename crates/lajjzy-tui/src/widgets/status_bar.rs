use std::collections::HashSet;

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Style};
use ratatui::text::Line;
use ratatui::widgets::Widget;

use lajjzy_core::types::ChangeDetail;

use crate::action::BackgroundKind;

pub struct StatusBarWidget<'a> {
    change_id: Option<&'a str>,
    detail: Option<&'a ChangeDetail>,
    error: Option<&'a str>,
    status_message: Option<&'a str>,
    pending_background: &'a HashSet<BackgroundKind>,
}

impl<'a> StatusBarWidget<'a> {
    pub fn new(
        change_id: Option<&'a str>,
        detail: Option<&'a ChangeDetail>,
        error: Option<&'a str>,
        status_message: Option<&'a str>,
        pending_background: &'a HashSet<BackgroundKind>,
    ) -> Self {
        Self {
            change_id,
            detail,
            error,
            status_message,
            pending_background,
        }
    }
}

impl Widget for StatusBarWidget<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if area.height == 0 {
            return;
        }

        // Priority: error (red) > status_message (green) > pending indicator > normal info
        if let Some(err) = self.error {
            let style = Style::default().fg(Color::Red);
            let line = Line::styled(err, style);
            buf.set_line(area.x, area.y, &line, area.width);
            return;
        }

        if let Some(msg) = self.status_message {
            let style = Style::default().fg(Color::Green);
            let line = Line::styled(msg, style);
            buf.set_line(area.x, area.y, &line, area.width);
            return;
        }

        // Build pending operations indicator string
        let mut ops: Vec<&str> = Vec::new();
        if self.pending_background.contains(&BackgroundKind::Push) {
            ops.push("Pushing...");
        }
        if self.pending_background.contains(&BackgroundKind::Fetch) {
            ops.push("Fetching...");
        }
        if !ops.is_empty() {
            let indicator = ops.join("  ");
            let style = Style::default().fg(Color::Cyan);
            let line = Line::styled(indicator, style);
            buf.set_line(area.x, area.y, &line, area.width);
            // Fall through to render change info on line 2 if there's space
            if area.height > 1
                && let Some(detail) = self.detail
            {
                let change_id = self.change_id.unwrap_or("???");
                let line2 = format!(
                    "{} {}  {} <{}>",
                    change_id, detail.commit_id, detail.author, detail.email
                );
                buf.set_line(area.x, area.y + 1, &Line::raw(&line2), area.width);
            }
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

    fn empty_bg() -> HashSet<BackgroundKind> {
        HashSet::new()
    }

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
            files: vec![],
        }
    }

    #[test]
    fn renders_change_detail() {
        let detail = sample_detail();
        let bg = empty_bg();
        let widget = StatusBarWidget::new(Some("abc12"), Some(&detail), None, None, &bg);
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
        let bg = empty_bg();
        let widget = StatusBarWidget::new(None, None, Some("Refresh failed: timeout"), None, &bg);
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
        let bg = empty_bg();
        let widget = StatusBarWidget::new(None, None, None, None, &bg);
        let area = Rect::new(0, 0, 40, 2);
        let mut buf = Buffer::empty(area);
        widget.render(area, &mut buf);
        // Buffer should be empty (all spaces)
        let line0: String = (0..40)
            .map(|x| buf[(x, 0)].symbol().chars().next().unwrap_or(' '))
            .collect();
        assert_eq!(line0.trim(), "");
    }

    #[test]
    fn renders_status_message_in_green() {
        let bg = empty_bg();
        let widget = StatusBarWidget::new(None, None, None, Some("Pushed ok"), &bg);
        let area = Rect::new(0, 0, 40, 1);
        let mut buf = Buffer::empty(area);
        widget.render(area, &mut buf);

        let line0: String = (0..40)
            .map(|x| buf[(x, 0)].symbol().chars().next().unwrap_or(' '))
            .collect();
        assert!(line0.contains("Pushed ok"));
        assert_eq!(buf[(0, 0)].style().fg, Some(Color::Green));
    }

    #[test]
    fn error_takes_priority_over_status_message() {
        let bg = empty_bg();
        let widget = StatusBarWidget::new(None, None, Some("fatal error"), Some("all good"), &bg);
        let area = Rect::new(0, 0, 40, 1);
        let mut buf = Buffer::empty(area);
        widget.render(area, &mut buf);

        let line0: String = (0..40)
            .map(|x| buf[(x, 0)].symbol().chars().next().unwrap_or(' '))
            .collect();
        assert!(line0.contains("fatal error"));
        assert!(!line0.contains("all good"));
        assert_eq!(buf[(0, 0)].style().fg, Some(Color::Red));
    }

    #[test]
    fn renders_pushing_indicator_in_cyan() {
        let mut bg = HashSet::new();
        bg.insert(BackgroundKind::Push);
        let widget = StatusBarWidget::new(None, None, None, None, &bg);
        let area = Rect::new(0, 0, 40, 1);
        let mut buf = Buffer::empty(area);
        widget.render(area, &mut buf);

        let line0: String = (0..40)
            .map(|x| buf[(x, 0)].symbol().chars().next().unwrap_or(' '))
            .collect();
        assert!(line0.contains("Pushing..."));
        assert_eq!(buf[(0, 0)].style().fg, Some(Color::Cyan));
    }

    #[test]
    fn renders_fetching_indicator_in_cyan() {
        let mut bg = HashSet::new();
        bg.insert(BackgroundKind::Fetch);
        let widget = StatusBarWidget::new(None, None, None, None, &bg);
        let area = Rect::new(0, 0, 40, 1);
        let mut buf = Buffer::empty(area);
        widget.render(area, &mut buf);

        let line0: String = (0..40)
            .map(|x| buf[(x, 0)].symbol().chars().next().unwrap_or(' '))
            .collect();
        assert!(line0.contains("Fetching..."));
        assert_eq!(buf[(0, 0)].style().fg, Some(Color::Cyan));
    }

    #[test]
    fn renders_both_push_and_fetch_indicators() {
        let mut bg = HashSet::new();
        bg.insert(BackgroundKind::Push);
        bg.insert(BackgroundKind::Fetch);
        let widget = StatusBarWidget::new(None, None, None, None, &bg);
        let area = Rect::new(0, 0, 80, 1);
        let mut buf = Buffer::empty(area);
        widget.render(area, &mut buf);

        let line0: String = (0..80)
            .map(|x| buf[(x, 0)].symbol().chars().next().unwrap_or(' '))
            .collect();
        assert!(line0.contains("Pushing...") || line0.contains("Fetching..."));
    }

    #[test]
    fn status_message_takes_priority_over_pending_indicator() {
        let mut bg: HashSet<BackgroundKind> = HashSet::new();
        bg.insert(BackgroundKind::Push);
        let widget = StatusBarWidget::new(None, None, None, Some("Pushed ok"), &bg);
        let area = Rect::new(0, 0, 40, 1);
        let mut buf = Buffer::empty(area);
        widget.render(area, &mut buf);

        let line0: String = (0..40)
            .map(|x| buf[(x, 0)].symbol().chars().next().unwrap_or(' '))
            .collect();
        assert!(line0.contains("Pushed ok"));
        assert!(!line0.contains("Pushing..."));
    }
}
