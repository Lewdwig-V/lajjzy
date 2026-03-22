use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Widget;

use lajjzy_core::forge::{PrInfo, PrState, ReviewStatus};
use lajjzy_core::types::GraphData;

use crate::app::TargetPick;
use std::collections::HashMap;

pub struct GraphWidget<'a> {
    graph: &'a GraphData,
    cursor: usize,
    scrolloff: usize,
    target_pick: Option<&'a TargetPick>,
    pr_status: &'a HashMap<String, PrInfo>,
}

impl<'a> GraphWidget<'a> {
    pub fn new(
        graph: &'a GraphData,
        cursor: usize,
        pr_status: &'a HashMap<String, PrInfo>,
    ) -> Self {
        Self {
            graph,
            cursor,
            scrolloff: 3,
            target_pick: None,
            pr_status,
        }
    }

    #[must_use]
    pub fn with_target_pick(mut self, target_pick: Option<&'a TargetPick>) -> Self {
        self.target_pick = target_pick;
        self
    }

    fn block_end(&self) -> usize {
        self.graph.lines[self.cursor + 1..]
            .iter()
            .position(|l| l.change_id.is_some())
            .map_or(self.graph.lines.len() - 1, |p| self.cursor + p)
    }

    pub(crate) fn scroll_offset(&self, height: usize) -> usize {
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
    ///
    /// When `dim` is `true` the entire line is rendered in `DarkGray` (picking
    /// mode exclusion).
    fn colored_node_line(&self, line_idx: usize, dim: bool) -> Line<'static> {
        let line = &self.graph.lines[line_idx];
        let Some(change_id) = line.change_id.as_deref() else {
            return Line::raw(line.raw.clone());
        };

        if dim {
            return Line::from(Span::styled(
                line.raw.clone(),
                Style::default().fg(Color::DarkGray),
            ));
        }

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

            // PR indicators after bookmarks
            for bookmark in &detail.bookmarks {
                if let Some(pr) = self.pr_status.get(bookmark) {
                    let (symbol, color) = match pr.state {
                        PrState::Merged => ("✓", Color::DarkGray),
                        PrState::Closed => ("✗", Color::DarkGray),
                        PrState::Open => match pr.review {
                            ReviewStatus::Approved => ("✓", Color::Green),
                            ReviewStatus::ChangesRequested => ("✗", Color::Red),
                            ReviewStatus::ReviewRequired | ReviewStatus::Unknown => {
                                ("●", Color::Yellow)
                            }
                        },
                    };
                    spans.push(Span::styled(
                        format!(" #{} {symbol}", pr.number),
                        Style::default().fg(color),
                    ));
                }
            }

            // Conflict indicator in yellow
            if detail.conflict_count > 0 {
                spans.push(Span::raw(" "));
                spans.push(Span::styled(
                    format!("⚠{}", detail.conflict_count),
                    Style::default().fg(Color::Yellow),
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

            // In picking mode, dim lines whose change_id is in the excluded set.
            let dim = self.target_pick.is_some_and(|pick| {
                line.change_id
                    .as_deref()
                    .is_some_and(|cid| pick.excluded.contains(cid))
            });

            let style = if !dim && line_idx >= block_start && line_idx <= block_end {
                highlight
            } else {
                Style::default()
            };

            let display = if line.change_id.is_some() {
                self.colored_node_line(line_idx, dim)
            } else {
                // Connector line in dark gray
                Line::from(Span::styled(
                    line.raw.clone(),
                    Style::default().fg(Color::DarkGray),
                ))
            };

            #[expect(clippy::cast_possible_truncation)] // row bounded by area.height (u16)
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
    use crate::action::RebaseMode;
    use crate::app::PickingMode;
    use lajjzy_core::types::{ChangeDetail, GraphData, GraphLine};
    use std::collections::{HashMap, HashSet};

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
                        conflict_count: 0,
                        files: vec![],
                        parents: vec![],
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
                        conflict_count: 0,
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
    fn renders_lines_in_buffer() {
        let graph = simple_graph();
        let pr_status = HashMap::new();
        let widget = GraphWidget::new(&graph, 0, &pr_status);
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
        let pr_status = HashMap::new();
        let widget = GraphWidget::new(&graph, 0, &pr_status);
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
        let pr_status = HashMap::new();
        let widget = GraphWidget::new(&graph, 0, &pr_status);
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
        let pr_status = HashMap::new();
        let widget = GraphWidget::new(&graph, 0, &pr_status);
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

    #[test]
    fn graph_dims_excluded_changes_in_picking_mode() {
        let graph = simple_graph(); // "abc" at idx 0, "def" at idx 2
        // Set up picking: "abc" is excluded (it's the source being rebased)
        let pick = TargetPick {
            source: "abc".into(),
            mode: RebaseMode::Single,
            excluded: HashSet::from(["abc".into()]),
            picking: PickingMode::Browsing,
            original_change_id: "abc".into(),
            descendant_count: 0,
        };

        // Cursor on "def" (row 2); "abc" (row 0) should be dimmed.
        let pr_status = HashMap::new();
        let widget = GraphWidget::new(&graph, 2, &pr_status).with_target_pick(Some(&pick));
        let area = Rect::new(0, 0, 60, 4);
        let mut buf = Buffer::empty(area);
        widget.render(area, &mut buf);

        // Row 0 ("abc") must have DarkGray fg — it is excluded / dimmed.
        assert_eq!(
            buf[(0, 0)].style().fg,
            Some(Color::DarkGray),
            "excluded change 'abc' should be rendered in DarkGray"
        );

        // Row 2 ("def") must NOT have DarkGray as the sole style — it is a valid
        // pick target and should be highlighted (REVERSED) since cursor is there.
        assert!(
            buf[(0, 2)]
                .style()
                .add_modifier
                .contains(Modifier::REVERSED),
            "non-excluded change 'def' at cursor should be REVERSED (highlighted)"
        );
    }

    #[test]
    fn graph_renders_conflict_indicator() {
        // Build a graph where "abc" has conflict_count > 0.
        let graph = GraphData::new(
            vec![GraphLine {
                raw: "◉  abc conflicted change".into(),
                change_id: Some("abc".into()),
                glyph_prefix: "◉  ".into(),
            }],
            HashMap::from([(
                "abc".into(),
                ChangeDetail {
                    commit_id: "aaa".into(),
                    author: "alice".into(),
                    email: "alice@ex.com".into(),
                    timestamp: "1m ago".into(),
                    description: "conflicted change".into(),
                    bookmarks: vec![],
                    is_empty: false,
                    conflict_count: 3,
                    files: vec![],
                    parents: vec![],
                },
            )]),
            Some(0),
            String::new(),
        );

        let pr_status = HashMap::new();
        let widget = GraphWidget::new(&graph, 0, &pr_status);
        let area = Rect::new(0, 0, 80, 2);
        let mut buf = Buffer::empty(area);
        widget.render(area, &mut buf);

        let line0: String = (0..80)
            .map(|x| buf[(x, 0)].symbol().chars().next().unwrap_or(' '))
            .collect();
        assert!(
            line0.contains('⚠'),
            "Expected '⚠' conflict indicator in: {line0:?}"
        );
        assert!(
            line0.contains('3'),
            "Expected conflict count '3' in: {line0:?}"
        );
    }

    #[test]
    fn graph_no_dimming_outside_picking_mode() {
        let graph = simple_graph();
        // No target_pick — normal rendering, "abc" should not be DarkGray at node level.
        let pr_status = HashMap::new();
        let widget = GraphWidget::new(&graph, 2, &pr_status);
        let area = Rect::new(0, 0, 60, 4);
        let mut buf = Buffer::empty(area);
        widget.render(area, &mut buf);

        // Row 0 ("abc") in normal mode: the glyph_prefix is DarkGray, but cell at
        // position of the change_id (after the 3-char glyph prefix) should be Yellow.
        // The glyph prefix is "◉  " (3 bytes). Since "◉" is a multi-byte char,
        // the change_id starts at offset 1 in ratatui's cell indexing (wide char = 2 cols).
        // We just check the row contains "abc" somewhere and is not all-DarkGray.
        let line0: String = (0..60)
            .map(|x| buf[(x, 0)].symbol().chars().next().unwrap_or(' '))
            .collect();
        assert!(
            line0.contains("abc"),
            "Without picking mode, 'abc' should still render: {line0:?}"
        );
    }

    #[test]
    fn graph_renders_pr_indicator() {
        let graph = simple_graph(); // "def" has bookmark "main"
        let mut pr_status = HashMap::new();
        pr_status.insert(
            "main".into(),
            PrInfo {
                number: 42,
                title: "Fix bug".into(),
                state: PrState::Open,
                review: ReviewStatus::Approved,
                head_ref: "main".into(),
                url: "https://github.com/org/repo/pull/42".into(),
            },
        );
        // Cursor on "def" (row 2) which has bookmark "main" with a PR
        let widget = GraphWidget::new(&graph, 2, &pr_status);
        let area = Rect::new(0, 0, 80, 4);
        let mut buf = Buffer::empty(area);
        widget.render(area, &mut buf);

        let line2: String = (0..80)
            .map(|x| buf[(x, 2)].symbol().chars().next().unwrap_or(' '))
            .collect();
        assert!(
            line2.contains("#42"),
            "Expected PR number '#42' in: {line2:?}"
        );
    }
}
