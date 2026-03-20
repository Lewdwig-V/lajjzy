use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::Line;
use ratatui::widgets::Widget;

use lajjzy_core::types::{DiffHunk, DiffLineKind};

pub struct DiffViewWidget<'a> {
    hunks: &'a [DiffHunk],
    scroll: usize,
}

impl<'a> DiffViewWidget<'a> {
    pub fn new(hunks: &'a [DiffHunk], scroll: usize) -> Self {
        Self { hunks, scroll }
    }

    fn flat_lines(&self) -> Vec<(DiffLineKind, &str)> {
        let mut lines = Vec::new();
        for hunk in self.hunks {
            lines.push((DiffLineKind::Header, hunk.header.as_str()));
            for dl in &hunk.lines {
                lines.push((dl.kind, &dl.content));
            }
        }
        lines
    }
}

impl Widget for DiffViewWidget<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if self.hunks.is_empty() {
            let msg = Line::styled("(empty diff)", Style::default().fg(Color::DarkGray));
            buf.set_line(area.x, area.y, &msg, area.width);
            return;
        }

        let flat = self.flat_lines();
        let height = area.height as usize;

        for (row, idx) in (self.scroll..self.scroll + height).enumerate() {
            if idx >= flat.len() {
                break;
            }

            let (kind, content) = flat[idx];
            let prefix = match kind {
                DiffLineKind::Added => "+",
                DiffLineKind::Removed => "-",
                DiffLineKind::Context => " ",
                DiffLineKind::Header => "",
            };
            let text = if prefix.is_empty() {
                content.to_string()
            } else {
                format!("{prefix}{content}")
            };

            let style = match kind {
                DiffLineKind::Added => Style::default().fg(Color::Green),
                DiffLineKind::Removed => Style::default().fg(Color::Red),
                DiffLineKind::Context => Style::default(),
                DiffLineKind::Header => Style::default()
                    .fg(Color::DarkGray)
                    .add_modifier(Modifier::BOLD),
            };

            #[allow(clippy::cast_possible_truncation)] // row bounded by area.height (u16)
            let y = area.y + row as u16;
            let line = Line::styled(&text, style);
            buf.set_line(area.x, y, &line, area.width);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use lajjzy_core::types::DiffLine;

    fn sample_hunks() -> Vec<DiffHunk> {
        vec![DiffHunk {
            header: "@@ -1,1 +1,1 @@".into(),
            lines: vec![
                DiffLine {
                    kind: DiffLineKind::Removed,
                    content: "hello".into(),
                },
                DiffLine {
                    kind: DiffLineKind::Added,
                    content: "world".into(),
                },
            ],
        }]
    }

    #[test]
    fn renders_diff_lines() {
        let hunks = sample_hunks();
        let widget = DiffViewWidget::new(&hunks, 0);
        let area = Rect::new(0, 0, 40, 5);
        let mut buf = Buffer::empty(area);
        widget.render(area, &mut buf);

        let line0: String = (0..40)
            .map(|x| buf[(x, 0)].symbol().chars().next().unwrap_or(' '))
            .collect();
        assert!(line0.contains("@@"));
    }

    #[test]
    fn renders_empty_diff() {
        let widget = DiffViewWidget::new(&[], 0);
        let area = Rect::new(0, 0, 40, 2);
        let mut buf = Buffer::empty(area);
        widget.render(area, &mut buf);

        let line0: String = (0..40)
            .map(|x| buf[(x, 0)].symbol().chars().next().unwrap_or(' '))
            .collect();
        assert!(line0.contains("empty diff"));
    }
}
