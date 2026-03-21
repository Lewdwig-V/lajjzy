use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Widget};

use crate::app::HelpContext;

pub struct HelpWidget {
    context: HelpContext,
    scroll: usize,
}

impl HelpWidget {
    pub fn new(context: HelpContext, scroll: usize) -> Self {
        Self { context, scroll }
    }

    fn help_lines(context: HelpContext) -> Vec<(&'static str, &'static str)> {
        match context {
            HelpContext::Graph => vec![
                ("j/k", "Move between changes"),
                ("g/G", "Jump to top/bottom"),
                ("@", "Jump to working copy"),
                ("Tab", "Switch to detail pane"),
                ("R", "Refresh"),
                ("/", "Fuzzy-find"),
                ("b", "Bookmarks"),
                ("O", "Operation log"),
                ("?", "This help"),
                ("q", "Quit"),
                ("d", "Abandon selected change"),
                ("n", "New change after selected"),
                ("e", "Edit description (inline)"),
                ("Ctrl-E", "Switch working copy (edit)"),
                ("S", "Squash into parent"),
                ("u", "Undo last operation"),
                ("Ctrl-R", "Redo"),
                ("B", "Set bookmark"),
                ("P", "Git push"),
                ("f", "Git fetch"),
            ],
            HelpContext::DetailFileList => vec![
                ("j/k", "Move between files"),
                ("Enter", "Open diff view"),
                ("Esc", "Return to graph"),
                ("Tab", "Switch to graph pane"),
            ],
            HelpContext::DetailDiffView => vec![
                ("j/k", "Scroll diff"),
                ("n/N", "Next/previous hunk"),
                ("Esc", "Return to file list"),
            ],
        }
    }
}

impl Widget for HelpWidget {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let title = match self.context {
            HelpContext::Graph => "Help — Graph",
            HelpContext::DetailFileList => "Help — File List",
            HelpContext::DetailDiffView => "Help — Diff View",
        };
        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Blue))
            .title(title);
        let inner = block.inner(area);
        block.render(area, buf);

        let lines = Self::help_lines(self.context);
        let height = inner.height as usize;
        let key_style = Style::default()
            .fg(Color::Yellow)
            .add_modifier(Modifier::BOLD);

        for (row, idx) in (self.scroll..self.scroll + height).enumerate() {
            if idx >= lines.len() {
                break;
            }
            let (key, desc) = lines[idx];
            let spans = vec![
                Span::styled(format!("{key:>10}"), key_style),
                Span::raw("  "),
                Span::raw(desc),
            ];
            let line = Line::from(spans);
            #[expect(clippy::cast_possible_truncation)]
            let y = inner.y + row as u16;
            buf.set_line(inner.x, y, &line, inner.width);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn graph_help_contains_quit() {
        let widget = HelpWidget::new(HelpContext::Graph, 0);
        let area = Rect::new(0, 0, 40, 14);
        let mut buf = Buffer::empty(area);
        widget.render(area, &mut buf);

        // Look for "Quit" somewhere in the rendered output
        let mut found = false;
        for y in 0..14 {
            let line: String = (0..40)
                .map(|x| buf[(x, y)].symbol().chars().next().unwrap_or(' '))
                .collect();
            if line.contains("Quit") {
                found = true;
                break;
            }
        }
        assert!(found, "Help should contain 'Quit'");
    }

    #[test]
    fn diff_help_contains_hunk_nav() {
        let widget = HelpWidget::new(HelpContext::DetailDiffView, 0);
        let area = Rect::new(0, 0, 40, 8);
        let mut buf = Buffer::empty(area);
        widget.render(area, &mut buf);

        let mut found = false;
        for y in 0..8 {
            let line: String = (0..40)
                .map(|x| buf[(x, y)].symbol().chars().next().unwrap_or(' '))
                .collect();
            if line.contains("hunk") {
                found = true;
                break;
            }
        }
        assert!(found, "Diff help should mention hunks");
    }
}
