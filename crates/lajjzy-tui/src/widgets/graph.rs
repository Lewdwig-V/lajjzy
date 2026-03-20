use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::Line;
use ratatui::widgets::Widget;

use lajjzy_core::types::GraphData;

pub struct GraphWidget<'a> {
    graph: &'a GraphData,
    cursor: usize,
    scrolloff: usize,
}

impl<'a> GraphWidget<'a> {
    pub fn new(graph: &'a GraphData, cursor: usize) -> Self {
        Self {
            graph,
            cursor,
            scrolloff: 3,
        }
    }

    fn block_end(&self) -> usize {
        self.graph.lines[self.cursor + 1..]
            .iter()
            .position(|l| l.change_id.is_some())
            .map_or(self.graph.lines.len() - 1, |p| self.cursor + p)
    }

    fn scroll_offset(&self, height: usize) -> usize {
        if self.graph.lines.is_empty() || height == 0 {
            return 0;
        }

        let block_start = self.cursor;
        let block_end = self.block_end();

        let total = self.graph.lines.len();
        let desired_top = block_start.saturating_sub(self.scrolloff);
        let desired_bottom = (block_end + self.scrolloff + 1).min(total);

        if desired_bottom - desired_top <= height {
            desired_top.min(total.saturating_sub(height))
        } else {
            block_start.min(total.saturating_sub(height))
        }
    }
}

impl Widget for GraphWidget<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if self.graph.lines.is_empty() {
            return;
        }
        let height = area.height as usize;
        let offset = self.scroll_offset(height);

        let block_start = self.cursor;
        let block_end = self.block_end();

        let highlight = Style::default().add_modifier(Modifier::REVERSED);

        for (row, line_idx) in (offset..offset + height).enumerate() {
            if line_idx >= self.graph.lines.len() {
                break;
            }

            let line = &self.graph.lines[line_idx];
            let style = if line_idx >= block_start && line_idx <= block_end {
                highlight
            } else {
                Style::default()
            };

            let display = Line::raw(&line.raw);
            #[allow(clippy::cast_possible_truncation)] // row bounded by area.height (u16)
            let y = area.y + row as u16;
            buf.set_line(area.x, y, &display, area.width);

            if style != Style::default() {
                for x in area.x..area.x + area.width {
                    buf[(x, y)].set_style(style);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use lajjzy_core::types::{GraphData, GraphLine};
    use std::collections::HashMap;

    fn simple_graph() -> GraphData {
        GraphData::new(
            vec![
                GraphLine {
                    raw: "◉  abc first".into(),
                    change_id: Some("abc".into()),
                },
                GraphLine {
                    raw: "│  description one".into(),
                    change_id: None,
                },
                GraphLine {
                    raw: "◉  def second".into(),
                    change_id: Some("def".into()),
                },
                GraphLine {
                    raw: "│  description two".into(),
                    change_id: None,
                },
            ],
            HashMap::new(),
            Some(0),
        )
    }

    #[test]
    fn renders_lines_in_buffer() {
        let graph = simple_graph();
        let widget = GraphWidget::new(&graph, 0);
        let area = Rect::new(0, 0, 30, 4);
        let mut buf = Buffer::empty(area);
        widget.render(area, &mut buf);

        let line0: String = (0..30)
            .map(|x| buf[(x, 0)].symbol().chars().next().unwrap_or(' '))
            .collect();
        assert!(line0.contains("abc first"));
    }

    #[test]
    fn highlighted_lines_have_reversed_style() {
        let graph = simple_graph();
        let widget = GraphWidget::new(&graph, 0);
        let area = Rect::new(0, 0, 30, 4);
        let mut buf = Buffer::empty(area);
        widget.render(area, &mut buf);

        assert!(
            buf[(0, 0)]
                .style()
                .add_modifier
                .contains(Modifier::REVERSED)
        );
        assert!(
            buf[(0, 1)]
                .style()
                .add_modifier
                .contains(Modifier::REVERSED)
        );
        assert!(
            !buf[(0, 2)]
                .style()
                .add_modifier
                .contains(Modifier::REVERSED)
        );
    }
}
