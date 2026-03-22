use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Widget};

use crate::action::CompletionItem;
use lajjzy_core::types::GraphData;

pub struct OmnibarWidget<'a> {
    query: &'a str,
    matches: &'a [usize], // graph line indices
    graph: &'a GraphData,
    cursor: usize,
    has_active_revset: bool,
    completions: &'a [CompletionItem],
    completion_cursor: usize,
}

impl<'a> OmnibarWidget<'a> {
    pub fn new(
        query: &'a str,
        matches: &'a [usize],
        graph: &'a GraphData,
        cursor: usize,
        has_active_revset: bool,
        completions: &'a [CompletionItem],
        completion_cursor: usize,
    ) -> Self {
        Self {
            query,
            matches,
            graph,
            cursor,
            has_active_revset,
            completions,
            completion_cursor,
        }
    }
}

/// Shared positional context for rendering rows into the results area.
struct ResultsArea {
    inner: Rect,
    results_y: u16,
    results_height: usize,
}

impl ResultsArea {
    fn highlight_row(&self, y: u16, buf: &mut Buffer) {
        let highlight = Style::default().add_modifier(Modifier::REVERSED);
        for x in self.inner.x..self.inner.x + self.inner.width {
            buf[(x, y)].set_style(highlight);
        }
    }

    fn scroll_for(&self, cursor: usize) -> usize {
        if self.results_height == 0 || cursor < self.results_height {
            0
        } else {
            cursor - self.results_height + 1
        }
    }
}

fn render_completions(
    completions: &[CompletionItem],
    completion_cursor: usize,
    ra: &ResultsArea,
    buf: &mut Buffer,
) {
    let scroll = ra.scroll_for(completion_cursor);
    for (row, idx) in (scroll..scroll + ra.results_height).enumerate() {
        if idx >= completions.len() {
            break;
        }
        let item = &completions[idx];
        let text_style = if item.insert_text.ends_with('(') {
            Style::default().fg(Color::Cyan)
        } else {
            Style::default()
        };
        let line = Line::from(Span::styled(&item.display_text, text_style));
        #[expect(clippy::cast_possible_truncation)]
        let y = ra.results_y + row as u16;
        buf.set_line(ra.inner.x, y, &line, ra.inner.width);
        if idx == completion_cursor {
            ra.highlight_row(y, buf);
        }
    }
}

fn render_fuzzy_matches(
    matches: &[usize],
    graph: &GraphData,
    cursor: usize,
    query: &str,
    ra: &ResultsArea,
    buf: &mut Buffer,
) {
    if matches.is_empty() {
        if ra.results_height > 0 {
            let msg = if query.is_empty() {
                "(no changes)"
            } else {
                "(no matches)"
            };
            let line = Line::styled(msg, Style::default().fg(Color::DarkGray));
            buf.set_line(ra.inner.x, ra.results_y, &line, ra.inner.width);
        }
        return;
    }
    let scroll = ra.scroll_for(cursor);
    for (row, idx) in (scroll..scroll + ra.results_height).enumerate() {
        if idx >= matches.len() {
            break;
        }
        let line_idx = matches[idx];
        let cid = graph.lines[line_idx].change_id.as_deref().unwrap_or("???");
        let detail = graph.details.get(cid);
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
        #[expect(clippy::cast_possible_truncation)]
        let y = ra.results_y + row as u16;
        buf.set_line(ra.inner.x, y, &line, ra.inner.width);
        if idx == cursor {
            ra.highlight_row(y, buf);
        }
    }
}

impl Widget for OmnibarWidget<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let showing_completions = !self.completions.is_empty();
        let title = if showing_completions {
            " / Completing... "
        } else if self.query.is_empty() && !self.has_active_revset {
            " / Search or Revset "
        } else if self.has_active_revset {
            " / Revset (active) "
        } else {
            " / Search (Enter to filter as revset) "
        };
        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Blue))
            .title(title);
        let inner = block.inner(area);
        block.render(area, buf);

        if inner.height == 0 {
            return;
        }

        let input_line = Line::from(vec![
            Span::styled("/ ", Style::default().fg(Color::Blue)),
            Span::raw(self.query),
            Span::styled("|", Style::default().fg(Color::DarkGray)),
        ]);
        buf.set_line(inner.x, inner.y, &input_line, inner.width);

        let ra = ResultsArea {
            inner,
            results_y: inner.y + 1,
            results_height: inner.height.saturating_sub(1) as usize,
        };

        if showing_completions {
            render_completions(self.completions, self.completion_cursor, &ra, buf);
        } else {
            render_fuzzy_matches(self.matches, self.graph, self.cursor, self.query, &ra, buf);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::action::CompletionItem;
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
                        parents: vec![],
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
                        parents: vec![],
                    },
                ),
            ]),
            Some(0),
            String::new(),
        )
    }

    #[test]
    fn renders_query_and_results() {
        let graph = test_graph();
        let matches = vec![0, 1];
        let widget = OmnibarWidget::new("ali", &matches, &graph, 0, false, &[], 0);
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
        let widget = OmnibarWidget::new("zzz", &[], &graph, 0, false, &[], 0);
        let area = Rect::new(0, 0, 40, 6);
        let mut buf = Buffer::empty(area);
        widget.render(area, &mut buf);

        let line2: String = (0..40)
            .map(|x| buf[(x, 2)].symbol().chars().next().unwrap_or(' '))
            .collect();
        assert!(line2.contains("no matches"));
    }

    fn title_from_buf(buf: &Buffer, width: u16) -> String {
        // The block title appears on row 0 of the rendered area
        (0..width)
            .map(|x| buf[(x, 0)].symbol().chars().next().unwrap_or(' '))
            .collect()
    }

    #[test]
    fn omnibar_title_shows_search_hint() {
        // Empty query, no active revset → title contains "Search or Revset"
        let graph = test_graph();
        let widget = OmnibarWidget::new("", &[], &graph, 0, false, &[], 0);
        let area = Rect::new(0, 0, 60, 6);
        let mut buf = Buffer::empty(area);
        widget.render(area, &mut buf);
        let title = title_from_buf(&buf, 60);
        assert!(
            title.contains("Search or Revset"),
            "expected 'Search or Revset' in title row, got: {title:?}"
        );
    }

    #[test]
    fn omnibar_title_shows_active_hint() {
        // has_active_revset = true → title contains "Revset (active)"
        let graph = test_graph();
        let widget = OmnibarWidget::new("mine()", &[], &graph, 0, true, &[], 0);
        let area = Rect::new(0, 0, 60, 6);
        let mut buf = Buffer::empty(area);
        widget.render(area, &mut buf);
        let title = title_from_buf(&buf, 60);
        assert!(
            title.contains("Revset (active)"),
            "expected 'Revset (active)' in title row, got: {title:?}"
        );
    }

    #[test]
    fn omnibar_title_shows_enter_hint_when_typing() {
        // Non-empty query, no active revset → title contains "Enter to filter as revset"
        let graph = test_graph();
        let widget = OmnibarWidget::new("foo", &[], &graph, 0, false, &[], 0);
        let area = Rect::new(0, 0, 80, 6);
        let mut buf = Buffer::empty(area);
        widget.render(area, &mut buf);
        let title = title_from_buf(&buf, 80);
        assert!(
            title.contains("Enter to filter as revset"),
            "expected 'Enter to filter as revset' in title row, got: {title:?}"
        );
    }

    fn make_completion(insert_text: &str, display_text: &str) -> CompletionItem {
        CompletionItem {
            insert_text: insert_text.to_string(),
            display_text: display_text.to_string(),
        }
    }

    #[test]
    fn omnibar_renders_completions_when_present() {
        let graph = test_graph();
        let completions = vec![
            make_completion("ancestors(", "ancestors( — revset function"),
            make_completion("main", "main — bookmark"),
        ];
        let widget = OmnibarWidget::new("an", &[], &graph, 0, false, &completions, 0);
        let area = Rect::new(0, 0, 60, 8);
        let mut buf = Buffer::empty(area);
        widget.render(area, &mut buf);

        // Row 2 (y=2) is the first results row (y=0 is border, y=1 is input line)
        let row2: String = (0..60)
            .map(|x| buf[(x, 2)].symbol().chars().next().unwrap_or(' '))
            .collect();
        assert!(
            row2.contains("ancestors"),
            "expected 'ancestors' in first completion row, got: {row2:?}"
        );

        let row3: String = (0..60)
            .map(|x| buf[(x, 3)].symbol().chars().next().unwrap_or(' '))
            .collect();
        assert!(
            row3.contains("main"),
            "expected 'main' in second completion row, got: {row3:?}"
        );
    }

    #[test]
    fn omnibar_title_shows_completing() {
        let graph = test_graph();
        let completions = vec![make_completion("mine()", "mine() — working copy owner")];
        let widget = OmnibarWidget::new("mi", &[], &graph, 0, false, &completions, 0);
        let area = Rect::new(0, 0, 60, 6);
        let mut buf = Buffer::empty(area);
        widget.render(area, &mut buf);

        let title = title_from_buf(&buf, 60);
        assert!(
            title.contains("Completing"),
            "expected 'Completing' in title row when completions present, got: {title:?}"
        );
    }

    #[test]
    fn omnibar_renders_fuzzy_when_no_completions() {
        // Regression: with empty completions, fuzzy match results are shown as normal
        let graph = test_graph();
        let matches = vec![0, 1];
        let widget = OmnibarWidget::new("ali", &matches, &graph, 0, false, &[], 0);
        let area = Rect::new(0, 0, 60, 8);
        let mut buf = Buffer::empty(area);
        widget.render(area, &mut buf);

        // Title should NOT say "Completing"
        let title = title_from_buf(&buf, 60);
        assert!(
            !title.contains("Completing"),
            "title must not say 'Completing' when no completions, got: {title:?}"
        );

        // First result row should contain the change id "abc"
        let row2: String = (0..60)
            .map(|x| buf[(x, 2)].symbol().chars().next().unwrap_or(' '))
            .collect();
        assert!(
            row2.contains("abc"),
            "expected fuzzy match 'abc' in results row, got: {row2:?}"
        );
    }
}
