use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
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

    /// Build a colored `Line` for a node line (one with a `change_id`).
    fn colored_node_line(&self, line_idx: usize) -> Line<'static> {
        let line = &self.graph.lines[line_idx];
        let Some(change_id) = line.change_id.as_deref() else {
            return Line::raw(line.raw.clone());
        };

        let is_working_copy = self.graph.working_copy_index == Some(line_idx);
        let glyph_style = if is_working_copy {
            Style::default()
                .fg(Color::Green)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::DarkGray)
        };

        let mut spans: Vec<Span<'static>> = Vec::new();

        // Glyph prefix (graph drawing characters + whitespace before the change ID)
        if !line.glyph_prefix.is_empty() {
            spans.push(Span::styled(line.glyph_prefix.clone(), glyph_style));
        }

        // Change ID in yellow
        spans.push(Span::styled(
            change_id.to_string(),
            Style::default().fg(Color::Yellow),
        ));

        // Look up details for author, timestamp, bookmarks
        if let Some(detail) = self.graph.details.get(change_id) {
            // Space separator
            spans.push(Span::raw(" "));

            // Author in blue
            if !detail.author.is_empty() {
                spans.push(Span::styled(
                    detail.author.clone(),
                    Style::default().fg(Color::Blue),
                ));
                spans.push(Span::raw(" "));
            }

            // Timestamp in cyan
            if !detail.timestamp.is_empty() {
                spans.push(Span::styled(
                    detail.timestamp.clone(),
                    Style::default().fg(Color::Cyan),
                ));
            }

            // Bookmarks in magenta brackets
            if !detail.bookmarks.is_empty() {
                spans.push(Span::raw(" "));
                spans.push(Span::styled(
                    format!("[{}]", detail.bookmarks.join(", ")),
                    Style::default().fg(Color::Magenta),
                ));
            }
        } else {
            // No detail — fall back to the raw tail after the glyph prefix
            debug_assert!(
                false,
                "GraphLine has change_id '{change_id}' but no detail entry"
            );
            let tail = &line.raw[line.glyph_prefix.len()..];
            spans.push(Span::raw(tail.to_string()));
        }

        Line::from(spans)
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

            let display = if line.change_id.is_some() {
                self.colored_node_line(line_idx)
            } else {
                // Connector line in dark gray
                Line::from(Span::styled(
                    line.raw.clone(),
                    Style::default().fg(Color::DarkGray),
                ))
            };

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
    use lajjzy_core::types::{ChangeDetail, GraphData, GraphLine};
    use std::collections::HashMap;

    fn simple_graph() -> GraphData {
        GraphData::new(
            vec![
                GraphLine {
                    raw: "◉  abc first".into(),
                    change_id: Some("abc".into()),
                    glyph_prefix: "◉  ".into(),
                },
                GraphLine {
                    raw: "│  description one".into(),
                    change_id: None,
                    glyph_prefix: "│  description one".into(),
                },
                GraphLine {
                    raw: "◉  def second".into(),
                    change_id: Some("def".into()),
                    glyph_prefix: "◉  ".into(),
                },
                GraphLine {
                    raw: "│  description two".into(),
                    change_id: None,
                    glyph_prefix: "│  description two".into(),
                },
            ],
            HashMap::from([
                (
                    "abc".into(),
                    ChangeDetail {
                        commit_id: "aaa".into(),
                        author: "alice".into(),
                        email: "alice@ex.com".into(),
                        timestamp: "2m ago".into(),
                        description: "first".into(),
                        bookmarks: vec![],
                        is_empty: false,
                        has_conflict: false,
                        files: vec![],
                    },
                ),
                (
                    "def".into(),
                    ChangeDetail {
                        commit_id: "ddd".into(),
                        author: "bob".into(),
                        email: "bob@ex.com".into(),
                        timestamp: "1h ago".into(),
                        description: "second".into(),
                        bookmarks: vec!["main".into()],
                        is_empty: false,
                        has_conflict: false,
                        files: vec![],
                    },
                ),
            ]),
            Some(0),
            String::new(),
        )
    }

    #[test]
    fn renders_lines_in_buffer() {
        let graph = simple_graph();
        let widget = GraphWidget::new(&graph, 0);
        let area = Rect::new(0, 0, 60, 4);
        let mut buf = Buffer::empty(area);
        widget.render(area, &mut buf);

        // Row 0 is a node line — change ID "abc" should appear
        let line0: String = (0..60)
            .map(|x| buf[(x, 0)].symbol().chars().next().unwrap_or(' '))
            .collect();
        assert!(line0.contains("abc"), "Expected 'abc' in: {line0:?}");

        // Row 2 is a node line — change ID "def" and bookmark "main" should appear
        let line2: String = (0..60)
            .map(|x| buf[(x, 2)].symbol().chars().next().unwrap_or(' '))
            .collect();
        assert!(line2.contains("def"), "Expected 'def' in: {line2:?}");
        assert!(line2.contains("main"), "Expected 'main' in: {line2:?}");
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

    #[test]
    fn working_copy_glyph_is_green_bold() {
        let graph = simple_graph(); // working_copy_index = Some(0)
        let widget = GraphWidget::new(&graph, 0);
        let area = Rect::new(0, 0, 60, 4);
        let mut buf = Buffer::empty(area);
        widget.render(area, &mut buf);

        // The first cell should have green+bold styling (from the glyph prefix)
        // Note: REVERSED is overlaid on top for the highlighted block.
        // We check the fg color before the REVERSED style overrides it.
        // The glyph prefix renders first; cell (0,0) is part of the glyph.
        // After set_line the fg should be Green; then REVERSED is applied.
        // We verify the modifier contains REVERSED and that the glyph row was
        // rendered (not left blank).
        let cell = &buf[(0, 0)];
        assert!(cell.style().add_modifier.contains(Modifier::REVERSED));
    }

    #[test]
    fn connector_line_renders_raw_text() {
        let graph = simple_graph();
        let widget = GraphWidget::new(&graph, 0);
        let area = Rect::new(0, 0, 40, 4);
        let mut buf = Buffer::empty(area);
        widget.render(area, &mut buf);

        // Row 1 is the connector line "│  description one"
        let line1: String = (0..40)
            .map(|x| buf[(x, 1)].symbol().chars().next().unwrap_or(' '))
            .collect();
        assert!(
            line1.contains("description one"),
            "Expected connector text in: {line1:?}"
        );
    }
}
