use std::collections::HashSet;

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Style};
use ratatui::text::Line;
use ratatui::widgets::Widget;

use lajjzy_core::types::{ChangeDetail, HunkResolution};

use crate::action::{BackgroundKind, HunkPickerOp, RebaseMode};
use crate::app::{ConflictView, HunkPicker, PickingMode, TargetPick};

pub struct StatusBarWidget<'a> {
    change_id: Option<&'a str>,
    detail: Option<&'a ChangeDetail>,
    error: Option<&'a str>,
    status_message: Option<&'a str>,
    active_revset: Option<&'a str>,
    pending_background: &'a HashSet<BackgroundKind>,
    target_pick: Option<&'a TargetPick>,
    hunk_picker: Option<&'a HunkPicker>,
    conflict_view: Option<&'a ConflictView>,
}

impl<'a> StatusBarWidget<'a> {
    #[expect(clippy::too_many_arguments)]
    pub fn new(
        change_id: Option<&'a str>,
        detail: Option<&'a ChangeDetail>,
        error: Option<&'a str>,
        status_message: Option<&'a str>,
        active_revset: Option<&'a str>,
        pending_background: &'a HashSet<BackgroundKind>,
        target_pick: Option<&'a TargetPick>,
        hunk_picker: Option<&'a HunkPicker>,
        conflict_view: Option<&'a ConflictView>,
    ) -> Self {
        Self {
            change_id,
            detail,
            error,
            status_message,
            active_revset,
            pending_background,
            target_pick,
            hunk_picker,
            conflict_view,
        }
    }
}

impl Widget for StatusBarWidget<'_> {
    #[expect(clippy::too_many_lines)]
    fn render(self, area: Rect, buf: &mut Buffer) {
        if area.height == 0 {
            return;
        }

        // Priority: hunk picker (magenta) > picking mode (yellow) > error (red) > status_message (green) > active_revset (cyan) > pending indicator > normal info
        if let Some(hp) = self.hunk_picker {
            let total: usize = hp.files.iter().map(|f| f.hunks.len()).sum();
            let selected: usize = hp
                .files
                .iter()
                .flat_map(|f| &f.hunks)
                .filter(|h| h.selected)
                .count();
            let text = match &hp.operation {
                HunkPickerOp::Split { source } => {
                    format!(
                        "Split: {selected}/{total} hunks selected → new change after {source}  (Space toggle, a/A all/none, Enter confirm, Esc cancel)"
                    )
                }
                HunkPickerOp::Squash {
                    source,
                    destination,
                } => {
                    format!(
                        "Squash: {selected}/{total} hunks from {source} → into {destination}  (Space toggle, a/A all/none, Enter confirm, Esc cancel)"
                    )
                }
            };
            let style = Style::default().fg(Color::Magenta);
            let line = Line::styled(text, style);
            buf.set_line(area.x, area.y, &line, area.width);
            return;
        }

        if let Some(pick) = self.target_pick {
            let mode_str = match pick.mode {
                RebaseMode::Single => format!("Rebase {} onto →", pick.source),
                RebaseMode::WithDescendants => format!(
                    "Rebase {} + {} descendant{} onto →",
                    pick.source,
                    pick.descendant_count,
                    if pick.descendant_count == 1 { "" } else { "s" },
                ),
            };
            let text = match &pick.picking {
                PickingMode::Browsing => {
                    format!("{mode_str}  (j/k navigate, Enter confirm, Esc cancel)")
                }
                PickingMode::Filtering { query } => {
                    format!("{mode_str}  filter: {query}  (Enter confirm, Esc cancel)")
                }
            };
            let style = Style::default().fg(Color::Yellow);
            let line = Line::styled(text, style);
            buf.set_line(area.x, area.y, &line, area.width);
            return;
        }

        if let Some(err) = self.error {
            let style = Style::default().fg(Color::Red);
            let line = Line::styled(err, style);
            buf.set_line(area.x, area.y, &line, area.width);
            return;
        }

        if let Some(msg) = self.status_message {
            let style = Style::default().fg(Color::Green);
            let line = Line::styled(msg, style);
            buf.set_line(area.x, area.y, &line, area.width);
            return;
        }

        if let Some(cv) = self.conflict_view {
            let total = cv.resolutions.len();
            let current = cv.cursor + 1;
            let state_str = match cv.resolutions.get(cv.cursor) {
                Some(HunkResolution::Unresolved) | None => "unresolved",
                Some(HunkResolution::AcceptLeft) => "left (ours)",
                Some(HunkResolution::AcceptRight) => "right (theirs)",
            };
            let text = format!(
                "Hunk {current}/{total}: {state_str} | 1: left | 2: right | n: next | m: merge tool"
            );
            let style = Style::default().fg(Color::Yellow);
            let line = Line::styled(text, style);
            buf.set_line(area.x, area.y, &line, area.width);
            return;
        }

        if let Some(revset) = self.active_revset {
            let text = format!("revset: {revset}");
            let style = Style::default().fg(Color::Cyan);
            let line = Line::styled(text, style);
            buf.set_line(area.x, area.y, &line, area.width);
            return;
        }

        // Build pending operations indicator string
        let mut ops: Vec<&str> = Vec::new();
        if self.pending_background.contains(&BackgroundKind::Push) {
            ops.push("Pushing...");
        }
        if self.pending_background.contains(&BackgroundKind::Fetch) {
            ops.push("Fetching...");
        }
        if !ops.is_empty() {
            let indicator = ops.join("  ");
            let style = Style::default().fg(Color::Cyan);
            let line = Line::styled(indicator, style);
            buf.set_line(area.x, area.y, &line, area.width);
            // Fall through to render change info on line 2 if there's space
            if area.height > 1
                && let Some(detail) = self.detail
            {
                let change_id = self.change_id.unwrap_or("???");
                let line2 = format!(
                    "{} {}  {} <{}>",
                    change_id, detail.commit_id, detail.author, detail.email
                );
                buf.set_line(area.x, area.y + 1, &Line::raw(&line2), area.width);
            }
            return;
        }

        if let Some(detail) = self.detail {
            let change_id = self.change_id.unwrap_or("???");
            let line1 = format!(
                "{} {}  {} <{}>",
                change_id, detail.commit_id, detail.author, detail.email
            );
            buf.set_line(area.x, area.y, &Line::raw(&line1), area.width);

            if area.height > 1 {
                let bookmarks_str = if detail.bookmarks.is_empty() {
                    String::new()
                } else {
                    format!("  [{}]", detail.bookmarks.join(", "))
                };
                let desc = if detail.description.is_empty() {
                    "(no description)"
                } else {
                    &detail.description
                };
                let line2 = format!("{desc}{bookmarks_str}");
                buf.set_line(area.x, area.y + 1, &Line::raw(&line2), area.width);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use lajjzy_core::types::ChangeDetail;

    fn empty_bg() -> HashSet<BackgroundKind> {
        HashSet::new()
    }

    fn sample_detail() -> ChangeDetail {
        ChangeDetail {
            commit_id: "aaa11".into(),
            author: "alice".into(),
            email: "alice@example.com".into(),
            timestamp: "2m ago".into(),
            description: "fix: parser bug".into(),
            bookmarks: vec!["main".into()],
            is_empty: false,
            conflict_count: 0,
            files: vec![],
            parents: vec![],
        }
    }

    #[test]
    fn renders_change_detail() {
        let detail = sample_detail();
        let bg = empty_bg();
        let widget = StatusBarWidget::new(
            Some("abc12"),
            Some(&detail),
            None,
            None,
            None,
            &bg,
            None,
            None,
            None,
        );
        let area = Rect::new(0, 0, 60, 2);
        let mut buf = Buffer::empty(area);
        widget.render(area, &mut buf);

        let line0: String = (0..60)
            .map(|x| buf[(x, 0)].symbol().chars().next().unwrap_or(' '))
            .collect();
        assert!(line0.contains("abc12"));
        assert!(line0.contains("alice"));

        let line1: String = (0..60)
            .map(|x| buf[(x, 1)].symbol().chars().next().unwrap_or(' '))
            .collect();
        assert!(line1.contains("fix: parser bug"));
        assert!(line1.contains("[main]"));
    }

    #[test]
    fn renders_error_in_red() {
        let bg = empty_bg();
        let widget = StatusBarWidget::new(
            None,
            None,
            Some("Refresh failed: timeout"),
            None,
            None,
            &bg,
            None,
            None,
            None,
        );
        let area = Rect::new(0, 0, 40, 2);
        let mut buf = Buffer::empty(area);
        widget.render(area, &mut buf);

        let line0: String = (0..40)
            .map(|x| buf[(x, 0)].symbol().chars().next().unwrap_or(' '))
            .collect();
        assert!(line0.contains("Refresh failed"));
        assert_eq!(buf[(0, 0)].style().fg, Some(Color::Red));
    }

    #[test]
    fn renders_nothing_when_no_detail_and_no_error() {
        let bg = empty_bg();
        let widget = StatusBarWidget::new(None, None, None, None, None, &bg, None, None, None);
        let area = Rect::new(0, 0, 40, 2);
        let mut buf = Buffer::empty(area);
        widget.render(area, &mut buf);
        // Buffer should be empty (all spaces)
        let line0: String = (0..40)
            .map(|x| buf[(x, 0)].symbol().chars().next().unwrap_or(' '))
            .collect();
        assert_eq!(line0.trim(), "");
    }

    #[test]
    fn renders_status_message_in_green() {
        let bg = empty_bg();
        let widget = StatusBarWidget::new(
            None,
            None,
            None,
            Some("Pushed ok"),
            None,
            &bg,
            None,
            None,
            None,
        );
        let area = Rect::new(0, 0, 40, 1);
        let mut buf = Buffer::empty(area);
        widget.render(area, &mut buf);

        let line0: String = (0..40)
            .map(|x| buf[(x, 0)].symbol().chars().next().unwrap_or(' '))
            .collect();
        assert!(line0.contains("Pushed ok"));
        assert_eq!(buf[(0, 0)].style().fg, Some(Color::Green));
    }

    #[test]
    fn error_takes_priority_over_status_message() {
        let bg = empty_bg();
        let widget = StatusBarWidget::new(
            None,
            None,
            Some("fatal error"),
            Some("all good"),
            None,
            &bg,
            None,
            None,
            None,
        );
        let area = Rect::new(0, 0, 40, 1);
        let mut buf = Buffer::empty(area);
        widget.render(area, &mut buf);

        let line0: String = (0..40)
            .map(|x| buf[(x, 0)].symbol().chars().next().unwrap_or(' '))
            .collect();
        assert!(line0.contains("fatal error"));
        assert!(!line0.contains("all good"));
        assert_eq!(buf[(0, 0)].style().fg, Some(Color::Red));
    }

    #[test]
    fn renders_pushing_indicator_in_cyan() {
        let mut bg = HashSet::new();
        bg.insert(BackgroundKind::Push);
        let widget = StatusBarWidget::new(None, None, None, None, None, &bg, None, None, None);
        let area = Rect::new(0, 0, 40, 1);
        let mut buf = Buffer::empty(area);
        widget.render(area, &mut buf);

        let line0: String = (0..40)
            .map(|x| buf[(x, 0)].symbol().chars().next().unwrap_or(' '))
            .collect();
        assert!(line0.contains("Pushing..."));
        assert_eq!(buf[(0, 0)].style().fg, Some(Color::Cyan));
    }

    #[test]
    fn renders_fetching_indicator_in_cyan() {
        let mut bg = HashSet::new();
        bg.insert(BackgroundKind::Fetch);
        let widget = StatusBarWidget::new(None, None, None, None, None, &bg, None, None, None);
        let area = Rect::new(0, 0, 40, 1);
        let mut buf = Buffer::empty(area);
        widget.render(area, &mut buf);

        let line0: String = (0..40)
            .map(|x| buf[(x, 0)].symbol().chars().next().unwrap_or(' '))
            .collect();
        assert!(line0.contains("Fetching..."));
        assert_eq!(buf[(0, 0)].style().fg, Some(Color::Cyan));
    }

    #[test]
    fn renders_both_push_and_fetch_indicators() {
        let mut bg = HashSet::new();
        bg.insert(BackgroundKind::Push);
        bg.insert(BackgroundKind::Fetch);
        let widget = StatusBarWidget::new(None, None, None, None, None, &bg, None, None, None);
        let area = Rect::new(0, 0, 80, 1);
        let mut buf = Buffer::empty(area);
        widget.render(area, &mut buf);

        let line0: String = (0..80)
            .map(|x| buf[(x, 0)].symbol().chars().next().unwrap_or(' '))
            .collect();
        assert!(line0.contains("Pushing...") || line0.contains("Fetching..."));
    }

    #[test]
    fn status_message_takes_priority_over_pending_indicator() {
        let mut bg: HashSet<BackgroundKind> = HashSet::new();
        bg.insert(BackgroundKind::Push);
        let widget = StatusBarWidget::new(
            None,
            None,
            None,
            Some("Pushed ok"),
            None,
            &bg,
            None,
            None,
            None,
        );
        let area = Rect::new(0, 0, 40, 1);
        let mut buf = Buffer::empty(area);
        widget.render(area, &mut buf);

        let line0: String = (0..40)
            .map(|x| buf[(x, 0)].symbol().chars().next().unwrap_or(' '))
            .collect();
        assert!(line0.contains("Pushed ok"));
        assert!(!line0.contains("Pushing..."));
    }

    #[test]
    fn status_bar_shows_active_revset() {
        let bg = empty_bg();
        let widget = StatusBarWidget::new(
            None,
            None,
            None,
            None,
            Some("mine() & ~empty()"),
            &bg,
            None,
            None,
            None,
        );
        let area = Rect::new(0, 0, 60, 1);
        let mut buf = Buffer::empty(area);
        widget.render(area, &mut buf);

        let line0: String = (0..60)
            .map(|x| buf[(x, 0)].symbol().chars().next().unwrap_or(' '))
            .collect();
        assert!(
            line0.contains("revset: mine() & ~empty()"),
            "expected revset breadcrumb, got: {line0:?}"
        );
        assert_eq!(buf[(0, 0)].style().fg, Some(Color::Cyan));
    }

    #[test]
    fn active_revset_takes_priority_over_pending_indicator() {
        let mut bg = HashSet::new();
        bg.insert(BackgroundKind::Push);
        let widget = StatusBarWidget::new(
            None,
            None,
            None,
            None,
            Some("mine()"),
            &bg,
            None,
            None,
            None,
        );
        let area = Rect::new(0, 0, 40, 1);
        let mut buf = Buffer::empty(area);
        widget.render(area, &mut buf);

        let line0: String = (0..40)
            .map(|x| buf[(x, 0)].symbol().chars().next().unwrap_or(' '))
            .collect();
        assert!(line0.contains("revset: mine()"));
        assert!(!line0.contains("Pushing..."));
    }

    fn sample_pick_single() -> TargetPick {
        TargetPick {
            source: "abc".into(),
            mode: RebaseMode::Single,
            excluded: std::collections::HashSet::from(["abc".into()]),
            picking: PickingMode::Browsing,
            original_change_id: "abc".into(),
            descendant_count: 0,
        }
    }

    fn sample_pick_with_descendants() -> TargetPick {
        TargetPick {
            source: "def".into(),
            mode: RebaseMode::WithDescendants,
            excluded: std::collections::HashSet::from(["def".into(), "abc".into()]),
            picking: PickingMode::Browsing,
            original_change_id: "def".into(),
            descendant_count: 1,
        }
    }

    #[test]
    fn status_bar_shows_picking_mode_text() {
        let bg = empty_bg();
        let pick = sample_pick_single();
        let widget =
            StatusBarWidget::new(None, None, None, None, None, &bg, Some(&pick), None, None);
        let area = Rect::new(0, 0, 80, 1);
        let mut buf = Buffer::empty(area);
        widget.render(area, &mut buf);

        let line0: String = (0..80)
            .map(|x| buf[(x, 0)].symbol().chars().next().unwrap_or(' '))
            .collect();
        assert!(
            line0.contains("Rebase abc onto →"),
            "expected picking text, got: {line0:?}"
        );
        assert_eq!(
            buf[(0, 0)].style().fg,
            Some(Color::Yellow),
            "picking mode text should be yellow"
        );
    }

    #[test]
    fn status_bar_shows_blast_radius() {
        let bg = empty_bg();
        let pick = sample_pick_with_descendants();
        let widget =
            StatusBarWidget::new(None, None, None, None, None, &bg, Some(&pick), None, None);
        let area = Rect::new(0, 0, 80, 1);
        let mut buf = Buffer::empty(area);
        widget.render(area, &mut buf);

        let line0: String = (0..80)
            .map(|x| buf[(x, 0)].symbol().chars().next().unwrap_or(' '))
            .collect();
        assert!(
            line0.contains("Rebase def + 1 descendant onto →"),
            "expected blast radius text, got: {line0:?}"
        );
        assert_eq!(buf[(0, 0)].style().fg, Some(Color::Yellow));
    }

    #[test]
    fn status_bar_shows_filter_query_in_picking_mode() {
        let bg = empty_bg();
        let mut pick = sample_pick_single();
        pick.picking = PickingMode::Filtering {
            query: "main".into(),
        };
        let widget =
            StatusBarWidget::new(None, None, None, None, None, &bg, Some(&pick), None, None);
        let area = Rect::new(0, 0, 80, 1);
        let mut buf = Buffer::empty(area);
        widget.render(area, &mut buf);

        let line0: String = (0..80)
            .map(|x| buf[(x, 0)].symbol().chars().next().unwrap_or(' '))
            .collect();
        assert!(
            line0.contains("filter: main"),
            "expected filter query in text, got: {line0:?}"
        );
    }

    #[test]
    fn picking_mode_takes_priority_over_error() {
        let bg = empty_bg();
        let pick = sample_pick_single();
        let widget = StatusBarWidget::new(
            None,
            None,
            Some("some error"),
            None,
            None,
            &bg,
            Some(&pick),
            None,
            None,
        );
        let area = Rect::new(0, 0, 80, 1);
        let mut buf = Buffer::empty(area);
        widget.render(area, &mut buf);

        let line0: String = (0..80)
            .map(|x| buf[(x, 0)].symbol().chars().next().unwrap_or(' '))
            .collect();
        assert!(
            line0.contains("Rebase abc onto →"),
            "picking mode must beat error, got: {line0:?}"
        );
        assert!(!line0.contains("some error"));
    }

    fn sample_hunk_picker_split() -> HunkPicker {
        use crate::action::HunkPickerOp;
        use crate::app::{PickerFile, PickerHunk};
        use lajjzy_core::types::{DiffLine, DiffLineKind};

        HunkPicker {
            operation: HunkPickerOp::Split {
                source: "abc12".into(),
            },
            files: vec![PickerFile {
                path: "src/lib.rs".into(),
                hunks: vec![
                    PickerHunk {
                        header: "@@ -1,1 +1,2 @@".into(),
                        lines: vec![DiffLine {
                            kind: DiffLineKind::Added,
                            content: "new line".into(),
                        }],
                        selected: true,
                    },
                    PickerHunk {
                        header: "@@ -5,1 +6,2 @@".into(),
                        lines: vec![DiffLine {
                            kind: DiffLineKind::Added,
                            content: "another".into(),
                        }],
                        selected: false,
                    },
                ],
            }],
            cursor: 0,
            scroll: 0,
            viewport_height: 20,
        }
    }

    fn sample_hunk_picker_squash() -> HunkPicker {
        use crate::action::HunkPickerOp;
        use crate::app::{PickerFile, PickerHunk};
        use lajjzy_core::types::{DiffLine, DiffLineKind};

        HunkPicker {
            operation: HunkPickerOp::Squash {
                source: "abc12".into(),
                destination: "def34".into(),
            },
            files: vec![PickerFile {
                path: "src/lib.rs".into(),
                hunks: vec![PickerHunk {
                    header: "@@ -1,1 +1,2 @@".into(),
                    lines: vec![DiffLine {
                        kind: DiffLineKind::Added,
                        content: "new line".into(),
                    }],
                    selected: true,
                }],
            }],
            cursor: 0,
            scroll: 0,
            viewport_height: 20,
        }
    }

    #[test]
    fn status_bar_shows_hunk_picker_split() {
        let bg = empty_bg();
        let hp = sample_hunk_picker_split();
        let widget = StatusBarWidget::new(None, None, None, None, None, &bg, None, Some(&hp), None);
        let area = Rect::new(0, 0, 100, 1);
        let mut buf = Buffer::empty(area);
        widget.render(area, &mut buf);

        let line0: String = (0..100)
            .map(|x| buf[(x, 0)].symbol().chars().next().unwrap_or(' '))
            .collect();
        assert!(
            line0.contains("Split:"),
            "expected Split prefix, got: {line0:?}"
        );
        assert!(
            line0.contains("1/2"),
            "expected 1/2 selected count, got: {line0:?}"
        );
        assert!(
            line0.contains("abc12"),
            "expected source id, got: {line0:?}"
        );
        assert_eq!(buf[(0, 0)].style().fg, Some(Color::Magenta));
    }

    #[test]
    fn status_bar_shows_hunk_picker_squash() {
        let bg = empty_bg();
        let hp = sample_hunk_picker_squash();
        let widget = StatusBarWidget::new(None, None, None, None, None, &bg, None, Some(&hp), None);
        let area = Rect::new(0, 0, 100, 1);
        let mut buf = Buffer::empty(area);
        widget.render(area, &mut buf);

        let line0: String = (0..100)
            .map(|x| buf[(x, 0)].symbol().chars().next().unwrap_or(' '))
            .collect();
        assert!(
            line0.contains("Squash:"),
            "expected Squash prefix, got: {line0:?}"
        );
        assert!(line0.contains("abc12"), "expected source, got: {line0:?}");
        assert!(
            line0.contains("def34"),
            "expected destination, got: {line0:?}"
        );
        assert_eq!(buf[(0, 0)].style().fg, Some(Color::Magenta));
    }

    #[test]
    fn hunk_picker_takes_priority_over_error() {
        let bg = empty_bg();
        let hp = sample_hunk_picker_split();
        let widget = StatusBarWidget::new(
            None,
            None,
            Some("some error"),
            None,
            None,
            &bg,
            None,
            Some(&hp),
            None,
        );
        let area = Rect::new(0, 0, 100, 1);
        let mut buf = Buffer::empty(area);
        widget.render(area, &mut buf);

        let line0: String = (0..100)
            .map(|x| buf[(x, 0)].symbol().chars().next().unwrap_or(' '))
            .collect();
        assert!(
            line0.contains("Split:"),
            "hunk picker must beat error, got: {line0:?}"
        );
        assert!(!line0.contains("some error"));
    }
}
