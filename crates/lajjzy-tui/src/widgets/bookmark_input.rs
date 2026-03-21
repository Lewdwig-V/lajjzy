use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Widget};

pub struct BookmarkInputWidget<'a> {
    input: &'a str,
    completions: &'a [String],
}

impl<'a> BookmarkInputWidget<'a> {
    pub fn new(input: &'a str, completions: &'a [String]) -> Self {
        Self { input, completions }
    }
}

impl Widget for BookmarkInputWidget<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Yellow))
            .title(" Set bookmark (Enter confirm | Esc cancel) ");
        let inner = block.inner(area);
        block.render(area, buf);

        if inner.height == 0 {
            return;
        }

        // Input line with cursor indicator
        let input_line = Line::from(vec![
            Span::styled("Bookmark: ", Style::default().fg(Color::Yellow)),
            Span::raw(self.input),
            Span::styled("|", Style::default().fg(Color::DarkGray)),
        ]);
        buf.set_line(inner.x, inner.y, &input_line, inner.width);

        // Show matching completions on second line if space available
        if inner.height < 2 || self.completions.is_empty() {
            return;
        }

        let query = self.input.to_lowercase();
        let matches: Vec<&str> = self
            .completions
            .iter()
            .filter(|c| c.to_lowercase().contains(&query))
            .map(String::as_str)
            .take(inner.width as usize / 10 + 1) // cap to available width
            .collect();

        if matches.is_empty() {
            return;
        }

        let spans: Vec<Span> = matches
            .iter()
            .enumerate()
            .flat_map(|(i, name)| {
                let mut s = vec![Span::styled(*name, Style::default().fg(Color::Magenta))];
                if i + 1 < matches.len() {
                    s.push(Span::styled("  ", Style::default()));
                }
                s
            })
            .collect();
        let completion_line = Line::from(spans);
        buf.set_line(inner.x, inner.y + 1, &completion_line, inner.width);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renders_input_and_cursor() {
        let completions: Vec<String> = vec!["main".into(), "feature".into()];
        let widget = BookmarkInputWidget::new("mai", &completions);
        let area = Rect::new(0, 0, 60, 4);
        let mut buf = Buffer::empty(area);
        widget.render(area, &mut buf);

        // Input line (row 1 inside the border)
        let line1: String = (0..60)
            .map(|x| buf[(x, 1)].symbol().chars().next().unwrap_or(' '))
            .collect();
        assert!(line1.contains("mai"), "should show current input");
        assert!(line1.contains('|'), "should show cursor indicator");
    }

    #[test]
    fn renders_completions_on_second_line() {
        let completions: Vec<String> = vec!["main".into(), "feature".into()];
        let widget = BookmarkInputWidget::new("ma", &completions);
        let area = Rect::new(0, 0, 60, 5);
        let mut buf = Buffer::empty(area);
        widget.render(area, &mut buf);

        let line2: String = (0..60)
            .map(|x| buf[(x, 2)].symbol().chars().next().unwrap_or(' '))
            .collect();
        assert!(line2.contains("main"), "should show matching completion");
    }

    #[test]
    fn empty_input_renders_without_panic() {
        let completions: Vec<String> = vec![];
        let widget = BookmarkInputWidget::new("", &completions);
        let area = Rect::new(0, 0, 40, 3);
        let mut buf = Buffer::empty(area);
        widget.render(area, &mut buf);
        // Just ensure it doesn't panic
    }
}
