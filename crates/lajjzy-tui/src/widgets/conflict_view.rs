use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::Line;
use ratatui::widgets::Widget;

use lajjzy_core::types::{ConflictRegion, HunkResolution};

use crate::app::ConflictView;

/// A single render line produced by flattening the conflict view data.
#[derive(Debug)]
enum RenderLine<'a> {
    /// Collapsed resolved region: "... N lines ..."
    ResolvedCollapsed { line_count: usize },
    /// Separator line for a conflict block section.
    Separator { text: String, style: Style },
    /// A content line within a conflict block section.
    Content { text: &'a str, style: Style },
    /// Resolution status line at the bottom of a conflict block.
    ResolutionStatus {
        resolution: HunkResolution,
        hunk_idx: usize,
    },
}

pub struct ConflictViewWidget<'a> {
    view: &'a ConflictView,
}

impl<'a> ConflictViewWidget<'a> {
    pub fn new(view: &'a ConflictView) -> Self {
        Self { view }
    }

    /// Build the complete flat render list from conflict view data.
    fn render_lines(&self) -> Vec<RenderLine<'_>> {
        let mut lines = Vec::new();
        let mut hunk_idx: usize = 0;

        for region in &self.view.data.regions {
            match region {
                ConflictRegion::Resolved(text) => {
                    let count = text.lines().count().max(1);
                    lines.push(RenderLine::ResolvedCollapsed { line_count: count });
                }
                ConflictRegion::Conflict { base, left, right } => {
                    let resolution = self
                        .view
                        .resolutions
                        .get(hunk_idx)
                        .copied()
                        .unwrap_or(HunkResolution::Unresolved);
                    let is_current = hunk_idx == self.view.cursor;

                    // base block (always dim)
                    Self::push_side(&mut lines, "base", "", base, is_current, false);

                    // left block — dimmed when right is accepted
                    let left_dimmed = resolution == HunkResolution::AcceptRight;
                    let left_fg = if left_dimmed {
                        Color::DarkGray
                    } else {
                        Color::Blue
                    };
                    Self::push_side_colored(
                        &mut lines,
                        "left (ours)",
                        "[1]",
                        left,
                        is_current,
                        left_dimmed,
                        left_fg,
                    );

                    // right block — dimmed when left is accepted
                    let right_dimmed = resolution == HunkResolution::AcceptLeft;
                    let right_fg = if right_dimmed {
                        Color::DarkGray
                    } else {
                        Color::Green
                    };
                    Self::push_side_colored(
                        &mut lines,
                        "right (theirs)",
                        "[2]",
                        right,
                        is_current,
                        right_dimmed,
                        right_fg,
                    );

                    // resolution status
                    lines.push(RenderLine::ResolutionStatus {
                        resolution,
                        hunk_idx,
                    });

                    hunk_idx += 1;
                }
            }
        }

        lines
    }

    /// Push a base-style side block (always `DarkGray`).
    fn push_side<'b>(
        lines: &mut Vec<RenderLine<'b>>,
        label: &str,
        tag: &str,
        content: &'b str,
        is_current: bool,
        _dimmed: bool,
    ) {
        Self::push_side_colored(
            lines,
            label,
            tag,
            content,
            is_current,
            true,
            Color::DarkGray,
        );
    }

    /// Push a separator + content lines for one side of the conflict.
    fn push_side_colored<'b>(
        lines: &mut Vec<RenderLine<'b>>,
        label: &str,
        tag: &str,
        content: &'b str,
        is_current: bool,
        dimmed: bool,
        fg: Color,
    ) {
        let (text, style) = Self::pad_separator(label, tag, is_current, fg);
        lines.push(RenderLine::Separator { text, style });

        let content_style = if dimmed {
            Style::default().fg(Color::DarkGray)
        } else {
            Style::default()
        };
        if content.is_empty() {
            lines.push(RenderLine::Content {
                text: "  (file deleted)",
                style: content_style.add_modifier(Modifier::ITALIC),
            });
        } else {
            for line in content.lines() {
                lines.push(RenderLine::Content {
                    text: line,
                    style: content_style,
                });
            }
        }
    }

    /// Build a separator line like `"--- label ----------- [tag] ---"`.
    /// Returns (text, style).
    fn pad_separator(label: &str, tag: &str, is_current: bool, fg: Color) -> (String, Style) {
        let text = if tag.is_empty() {
            format!("\u{2500}\u{2500}\u{2500} {label} \u{2500}\u{2500}\u{2500}")
        } else {
            format!(
                "\u{2500}\u{2500}\u{2500} {label} \u{2500}\u{2500}\u{2500} {tag} \u{2500}\u{2500}\u{2500}"
            )
        };
        let mut style = Style::default().fg(fg);
        if is_current {
            style = style.add_modifier(Modifier::BOLD);
        }
        (text, style)
    }

    /// Format the resolution status text.
    fn resolution_text(resolution: HunkResolution) -> &'static str {
        match resolution {
            HunkResolution::Unresolved => "resolved: none",
            HunkResolution::AcceptLeft => "resolved: left (ours)",
            HunkResolution::AcceptRight => "resolved: right (theirs)",
        }
    }
}

impl Widget for ConflictViewWidget<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if area.width == 0 || area.height == 0 {
            return;
        }

        if self.view.data.regions.is_empty() {
            let msg = Line::styled(
                "(no conflict regions)",
                Style::default().fg(Color::DarkGray),
            );
            buf.set_line(area.x, area.y, &msg, area.width);
            return;
        }

        let lines = self.render_lines();
        let height = area.height as usize;
        let scroll = self.view.scroll;

        for (row, item) in lines.iter().skip(scroll).take(height).enumerate() {
            #[expect(clippy::cast_possible_truncation)]
            let y = area.y + row as u16;

            match item {
                RenderLine::ResolvedCollapsed { line_count } => {
                    let text = format!(
                        "\u{00b7}\u{00b7}\u{00b7} {line_count} lines \u{00b7}\u{00b7}\u{00b7}"
                    );
                    let style = Style::default().fg(Color::DarkGray);
                    let line = Line::styled(text, style);
                    buf.set_line(area.x, y, &line, area.width);
                }
                RenderLine::Separator { text, style } => {
                    // Fill the row with the separator style for a clean look
                    for x in area.x..area.x + area.width {
                        buf[(x, y)].set_style(*style);
                    }
                    let line = Line::styled(text.as_str(), *style);
                    buf.set_line(area.x, y, &line, area.width);
                }
                RenderLine::Content { text, style } => {
                    let padded = format!(" {text}");
                    let line = Line::styled(padded, *style);
                    buf.set_line(area.x, y, &line, area.width);
                }
                RenderLine::ResolutionStatus {
                    resolution,
                    hunk_idx,
                } => {
                    let label = Self::resolution_text(*resolution);
                    let text = format!("\u{2500}\u{2500}\u{2500} {label} \u{2500}\u{2500}\u{2500}");
                    let is_current = *hunk_idx == self.view.cursor;
                    let mut style = Style::default().fg(Color::Yellow);
                    if is_current {
                        style = style.add_modifier(Modifier::BOLD);
                    }
                    for x in area.x..area.x + area.width {
                        buf[(x, y)].set_style(style);
                    }
                    let line = Line::styled(text, style);
                    buf.set_line(area.x, y, &line, area.width);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use lajjzy_core::types::ConflictData;

    fn read_row(buf: &Buffer, y: u16, width: u16) -> String {
        (0..width)
            .map(|x| buf[(x, y)].symbol().chars().next().unwrap_or(' '))
            .collect()
    }

    fn make_view(regions: Vec<ConflictRegion>, resolutions: Vec<HunkResolution>) -> ConflictView {
        ConflictView {
            change_id: "abc".into(),
            path: "src/lib.rs".into(),
            data: ConflictData { regions },
            resolutions,
            cursor: 0,
            scroll: 0,
            viewport_height: 20,
        }
    }

    #[test]
    fn renders_base_left_right_separators() {
        let view = make_view(
            vec![ConflictRegion::Conflict {
                base: "base line".into(),
                left: "left line".into(),
                right: "right line".into(),
            }],
            vec![HunkResolution::Unresolved],
        );
        let widget = ConflictViewWidget::new(&view);
        let area = Rect::new(0, 0, 60, 20);
        let mut buf = Buffer::empty(area);
        widget.render(area, &mut buf);

        let mut found_base = false;
        let mut found_left = false;
        let mut found_right = false;
        for y in 0..20 {
            let row = read_row(&buf, y, 60);
            if row.contains("base") {
                found_base = true;
            }
            if row.contains("left (ours)") {
                found_left = true;
            }
            if row.contains("right (theirs)") {
                found_right = true;
            }
        }
        assert!(found_base, "base separator missing");
        assert!(found_left, "left separator missing");
        assert!(found_right, "right separator missing");
    }

    #[test]
    fn renders_resolved_region_as_collapsed_line() {
        let view = make_view(
            vec![
                ConflictRegion::Resolved("line1\nline2\nline3".into()),
                ConflictRegion::Conflict {
                    base: "b".into(),
                    left: "l".into(),
                    right: "r".into(),
                },
            ],
            vec![HunkResolution::Unresolved],
        );
        let widget = ConflictViewWidget::new(&view);
        let area = Rect::new(0, 0, 60, 20);
        let mut buf = Buffer::empty(area);
        widget.render(area, &mut buf);

        let row0 = read_row(&buf, 0, 60);
        assert!(
            row0.contains("3 lines"),
            "expected collapsed resolved line, got: {row0:?}"
        );
    }

    #[test]
    fn does_not_panic_on_empty_area() {
        let view = make_view(
            vec![ConflictRegion::Conflict {
                base: "b".into(),
                left: "l".into(),
                right: "r".into(),
            }],
            vec![HunkResolution::Unresolved],
        );
        let widget = ConflictViewWidget::new(&view);
        let area = Rect::new(0, 0, 0, 0);
        let mut buf = Buffer::empty(area);
        widget.render(area, &mut buf);
        // No panic = pass
    }

    #[test]
    fn does_not_panic_on_empty_data() {
        let view = make_view(vec![], vec![]);
        let widget = ConflictViewWidget::new(&view);
        let area = Rect::new(0, 0, 40, 5);
        let mut buf = Buffer::empty(area);
        widget.render(area, &mut buf);

        let row0 = read_row(&buf, 0, 40);
        assert!(
            row0.contains("no conflict regions"),
            "expected empty message: {row0:?}"
        );
    }

    #[test]
    fn selected_left_dims_right_side() {
        let view = make_view(
            vec![ConflictRegion::Conflict {
                base: "base".into(),
                left: "left".into(),
                right: "right".into(),
            }],
            vec![HunkResolution::AcceptLeft],
        );
        let widget = ConflictViewWidget::new(&view);
        let area = Rect::new(0, 0, 60, 20);
        let mut buf = Buffer::empty(area);
        widget.render(area, &mut buf);

        // Find the "right (theirs)" separator and check it is dimmed (DarkGray)
        for y in 0..20 {
            let row = read_row(&buf, y, 60);
            if row.contains("right (theirs)") {
                let cell = &buf[(4, y)];
                assert_eq!(
                    cell.fg,
                    Color::DarkGray,
                    "right separator should be dimmed when left is selected"
                );
                return;
            }
        }
        panic!("right separator not found");
    }

    #[test]
    fn selected_right_dims_left_side() {
        let view = make_view(
            vec![ConflictRegion::Conflict {
                base: "base".into(),
                left: "left".into(),
                right: "right".into(),
            }],
            vec![HunkResolution::AcceptRight],
        );
        let widget = ConflictViewWidget::new(&view);
        let area = Rect::new(0, 0, 60, 20);
        let mut buf = Buffer::empty(area);
        widget.render(area, &mut buf);

        // Find the "left (ours)" separator and check it is dimmed (DarkGray)
        for y in 0..20 {
            let row = read_row(&buf, y, 60);
            if row.contains("left (ours)") {
                let cell = &buf[(4, y)];
                assert_eq!(
                    cell.fg,
                    Color::DarkGray,
                    "left separator should be dimmed when right is selected"
                );
                return;
            }
        }
        panic!("left separator not found");
    }

    #[test]
    fn resolution_status_shows_resolved_left() {
        let view = make_view(
            vec![ConflictRegion::Conflict {
                base: "b".into(),
                left: "l".into(),
                right: "r".into(),
            }],
            vec![HunkResolution::AcceptLeft],
        );
        let widget = ConflictViewWidget::new(&view);
        let area = Rect::new(0, 0, 60, 20);
        let mut buf = Buffer::empty(area);
        widget.render(area, &mut buf);

        let mut found = false;
        for y in 0..20 {
            let row = read_row(&buf, y, 60);
            if row.contains("resolved: left (ours)") {
                found = true;
                break;
            }
        }
        assert!(found, "expected resolution status line for AcceptLeft");
    }

    #[test]
    fn current_hunk_separator_is_bold() {
        let view = make_view(
            vec![ConflictRegion::Conflict {
                base: "b".into(),
                left: "l".into(),
                right: "r".into(),
            }],
            vec![HunkResolution::Unresolved],
        );
        let widget = ConflictViewWidget::new(&view);
        let area = Rect::new(0, 0, 60, 20);
        let mut buf = Buffer::empty(area);
        widget.render(area, &mut buf);

        // The first separator (base) at row 0 should be bold (cursor = 0)
        let cell = &buf[(0, 0)];
        assert!(
            cell.modifier.contains(Modifier::BOLD),
            "current hunk separator should be BOLD"
        );
    }

    #[test]
    fn empty_side_renders_file_deleted() {
        let view = make_view(
            vec![ConflictRegion::Conflict {
                base: "".into(),
                left: "left content".into(),
                right: "".into(),
            }],
            vec![HunkResolution::Unresolved],
        );
        let widget = ConflictViewWidget::new(&view);
        let area = Rect::new(0, 0, 60, 20);
        let mut buf = Buffer::empty(area);
        widget.render(area, &mut buf);

        let mut deleted_count = 0;
        for y in 0..20 {
            let row = read_row(&buf, y, 60);
            if row.contains("(file deleted)") {
                deleted_count += 1;
            }
        }
        assert_eq!(
            deleted_count, 2,
            "expected 2 '(file deleted)' lines for empty base and right"
        );
    }
}
