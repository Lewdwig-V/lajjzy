use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::Line;
use ratatui::widgets::Widget;

use lajjzy_core::types::{FileChange, FileStatus};

pub struct FileListWidget<'a> {
    files: &'a [FileChange],
    cursor: usize,
    focused: bool,
}

impl<'a> FileListWidget<'a> {
    pub fn new(files: &'a [FileChange], cursor: usize, focused: bool) -> Self {
        Self {
            files,
            cursor,
            focused,
        }
    }

    fn status_color(status: FileStatus) -> Color {
        match status {
            FileStatus::Added => Color::Green,
            FileStatus::Modified => Color::Yellow,
            FileStatus::Deleted => Color::Red,
            FileStatus::Renamed => Color::Cyan,
        }
    }
}

impl Widget for FileListWidget<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if self.files.is_empty() {
            let msg = Line::styled("(no files changed)", Style::default().fg(Color::DarkGray));
            buf.set_line(area.x, area.y, &msg, area.width);
            return;
        }

        let highlight = if self.focused {
            Style::default().add_modifier(Modifier::REVERSED)
        } else {
            Style::default().add_modifier(Modifier::BOLD)
        };

        for (i, file) in self.files.iter().enumerate() {
            if i >= area.height as usize {
                break;
            }

            #[allow(clippy::cast_possible_truncation)] // i bounded by area.height (u16)
            let y = area.y + i as u16;
            let line_text = format!("  {} {}", file.status, file.path);
            let color = Self::status_color(file.status);

            let style = if i == self.cursor {
                highlight.fg(color)
            } else {
                Style::default().fg(color)
            };

            let line = Line::styled(&line_text, style);
            buf.set_line(area.x, y, &line, area.width);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_files() -> Vec<FileChange> {
        vec![
            FileChange {
                path: "bar.txt".into(),
                status: FileStatus::Added,
            },
            FileChange {
                path: "foo.txt".into(),
                status: FileStatus::Modified,
            },
        ]
    }

    #[test]
    fn renders_file_entries() {
        let files = sample_files();
        let widget = FileListWidget::new(&files, 0, true);
        let area = Rect::new(0, 0, 40, 4);
        let mut buf = Buffer::empty(area);
        widget.render(area, &mut buf);

        let line0: String = (0..40)
            .map(|x| buf[(x, 0)].symbol().chars().next().unwrap_or(' '))
            .collect();
        assert!(line0.contains("A"));
        assert!(line0.contains("bar.txt"));
    }

    #[test]
    fn renders_empty_files() {
        let widget = FileListWidget::new(&[], 0, false);
        let area = Rect::new(0, 0, 40, 2);
        let mut buf = Buffer::empty(area);
        widget.render(area, &mut buf);

        let line0: String = (0..40)
            .map(|x| buf[(x, 0)].symbol().chars().next().unwrap_or(' '))
            .collect();
        assert!(line0.contains("no files"));
    }
}
