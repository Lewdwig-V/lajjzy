use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Widget};

use lajjzy_core::types::GraphData;

pub struct FuzzyFindWidget<'a> {
    query: &'a str,
    matches: &'a [usize], // graph line indices
    graph: &'a GraphData,
    cursor: usize,
}

impl<'a> FuzzyFindWidget<'a> {
    pub fn new(query: &'a str, matches: &'a [usize], graph: &'a GraphData, cursor: usize) -> Self {
        Self {
            query,
            matches,
            graph,
            cursor,
        }
    }
}

impl Widget for FuzzyFindWidget<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Blue))
            .title("Find Change");
        let inner = block.inner(area);
        block.render(area, buf);

        if inner.height == 0 {
            return;
        }

        // Input line
        let input_line = Line::from(vec![
            Span::styled("/ ", Style::default().fg(Color::Blue)),
            Span::raw(self.query),
            Span::styled("|", Style::default().fg(Color::DarkGray)),
        ]);
        buf.set_line(inner.x, inner.y, &input_line, inner.width);

        // Results
        let results_y = inner.y + 1;
        let results_height = inner.height.saturating_sub(1) as usize;
        let highlight = Style::default().add_modifier(Modifier::REVERSED);

        if self.matches.is_empty() {
            if results_height > 0 {
                let msg = if self.query.is_empty() {
                    "(no changes)"
                } else {
                    "(no matches)"
                };
                let line = Line::styled(msg, Style::default().fg(Color::DarkGray));
                buf.set_line(inner.x, results_y, &line, inner.width);
            }
            return;
        }

        for (row, idx) in (0..results_height).enumerate() {
            if idx >= self.matches.len() {
                break;
            }
            let line_idx = self.matches[idx];
            let cid = self.graph.lines[line_idx]
                .change_id
                .as_deref()
                .unwrap_or("???");
            let detail = self.graph.details.get(cid);
            let author = detail.map_or("", |d| d.author.as_str());
            let desc = detail.map_or("", |d| d.description.as_str());

            let spans = vec![
                Span::styled(cid, Style::default().fg(Color::Yellow)),
                Span::raw("  "),
                Span::styled(author, Style::default().fg(Color::Blue)),
                Span::raw("  "),
                Span::raw(desc),
            ];
            let line = Line::from(spans);
            #[allow(clippy::cast_possible_truncation)]
            let y = results_y + row as u16;
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
    use lajjzy_core::types::{ChangeDetail, GraphLine};
    use std::collections::HashMap;

    fn test_graph() -> GraphData {
        GraphData::new(
            vec![
                GraphLine {
                    raw: "@ abc".into(),
                    glyph_prefix: "@ ".into(),
                    change_id: Some("abc".into()),
                },
                GraphLine {
                    raw: "o def".into(),
                    glyph_prefix: "o ".into(),
                    change_id: Some("def".into()),
                },
            ],
            HashMap::from([
                (
                    "abc".into(),
                    ChangeDetail {
                        commit_id: "a1".into(),
                        author: "alice".into(),
                        email: "a@b".into(),
                        timestamp: "1m".into(),
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
                        commit_id: "d1".into(),
                        author: "bob".into(),
                        email: "b@c".into(),
                        timestamp: "2m".into(),
                        description: "second".into(),
                        bookmarks: vec![],
                        is_empty: false,
                        has_conflict: false,
                        files: vec![],
                    },
                ),
            ]),
            Some(0),
        )
    }

    #[test]
    fn renders_query_and_results() {
        let graph = test_graph();
        let matches = vec![0, 1];
        let widget = FuzzyFindWidget::new("ali", &matches, &graph, 0);
        let area = Rect::new(0, 0, 60, 8);
        let mut buf = Buffer::empty(area);
        widget.render(area, &mut buf);

        // Input line should show the query
        let line1: String = (0..60)
            .map(|x| buf[(x, 1)].symbol().chars().next().unwrap_or(' '))
            .collect();
        assert!(line1.contains("ali"));
    }

    #[test]
    fn renders_no_matches() {
        let graph = test_graph();
        let widget = FuzzyFindWidget::new("zzz", &[], &graph, 0);
        let area = Rect::new(0, 0, 40, 6);
        let mut buf = Buffer::empty(area);
        widget.render(area, &mut buf);

        let line2: String = (0..40)
            .map(|x| buf[(x, 2)].symbol().chars().next().unwrap_or(' '))
            .collect();
        assert!(line2.contains("no matches"));
    }
}
