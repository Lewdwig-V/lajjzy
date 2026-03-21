use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::Line;
use ratatui::widgets::Widget;

use lajjzy_core::types::DiffLineKind;

use crate::app::{HunkPicker, PickerFile, PickerHunk};

/// A flat item in the picker's render list.
#[derive(Debug)]
enum PickerItem<'a> {
    FileHeader {
        file: &'a PickerFile,
    },
    Hunk {
        hunk: &'a PickerHunk,
        /// Global flat-list index (used for cursor comparison)
        flat_idx: usize,
    },
    DiffLine {
        kind: DiffLineKind,
        content: &'a str,
        /// Global flat-list index of the *hunk* that owns this line (for bg tinting)
        hunk_flat_idx: usize,
        hunk_selected: bool,
    },
}

pub struct HunkPickerWidget<'a> {
    picker: &'a HunkPicker,
}

impl<'a> HunkPickerWidget<'a> {
    pub fn new(picker: &'a HunkPicker) -> Self {
        Self { picker }
    }

    /// Build the complete flat render list from the picker state.
    fn flat_items(&self) -> Vec<PickerItem<'_>> {
        let mut items = Vec::new();
        let mut flat_idx: usize = 0;

        for file in &self.picker.files {
            items.push(PickerItem::FileHeader { file });

            for hunk in &file.hunks {
                let hunk_flat_idx = flat_idx;
                let selected = hunk.selected;
                items.push(PickerItem::Hunk {
                    hunk,
                    flat_idx: hunk_flat_idx,
                });
                flat_idx += 1;

                for dl in &hunk.lines {
                    items.push(PickerItem::DiffLine {
                        kind: dl.kind,
                        content: &dl.content,
                        hunk_flat_idx,
                        hunk_selected: selected,
                    });
                }
            }
        }

        items
    }
}

impl Widget for HunkPickerWidget<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if self.picker.files.is_empty() {
            let msg = Line::styled("(no hunks to pick)", Style::default().fg(Color::DarkGray));
            buf.set_line(area.x, area.y, &msg, area.width);
            return;
        }

        let items = self.flat_items();
        let height = area.height as usize;
        let scroll = self.picker.scroll;

        for (row, item) in items.iter().skip(scroll).take(height).enumerate() {
            #[expect(clippy::cast_possible_truncation)] // row bounded by area.height (u16)
            let y = area.y + row as u16;

            match item {
                PickerItem::FileHeader { file } => {
                    let total = file.hunks.len();
                    let selected_count = file.hunks.iter().filter(|h| h.selected).count();
                    let text = format!("▸ {}  [{}/{}]", file.path, selected_count, total);
                    let style = Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD);
                    let line = Line::styled(text, style);
                    buf.set_line(area.x, y, &line, area.width);
                }

                PickerItem::Hunk { hunk, flat_idx } => {
                    let checkbox = if hunk.selected { "[✓]" } else { "[ ]" };
                    let text = format!("  {checkbox} {}", hunk.header);

                    let is_cursor = *flat_idx == self.picker.cursor;
                    let bg = if hunk.selected {
                        Color::Rgb(0, 40, 40)
                    } else {
                        Color::Reset
                    };
                    let mut style = Style::default().bg(bg);
                    if is_cursor {
                        style = style.add_modifier(Modifier::REVERSED);
                    }

                    // Fill the whole row with the background before setting the text
                    for x in area.x..area.x + area.width {
                        buf[(x, y)].set_style(style);
                    }

                    let line = Line::styled(text, style);
                    buf.set_line(area.x, y, &line, area.width);
                }

                PickerItem::DiffLine {
                    kind,
                    content,
                    hunk_flat_idx,
                    hunk_selected,
                } => {
                    let prefix = match kind {
                        DiffLineKind::Added => "+",
                        DiffLineKind::Removed => "-",
                        DiffLineKind::Context => " ",
                        DiffLineKind::Header => "",
                    };
                    let text = if prefix.is_empty() {
                        format!("    {content}")
                    } else {
                        format!("    {prefix}{content}")
                    };

                    let is_cursor_hunk = *hunk_flat_idx == self.picker.cursor;
                    let bg = if *hunk_selected {
                        Color::Rgb(0, 40, 40)
                    } else {
                        Color::Reset
                    };

                    let fg = match kind {
                        DiffLineKind::Added => Color::Green,
                        DiffLineKind::Removed => Color::Red,
                        DiffLineKind::Context => Color::Reset,
                        DiffLineKind::Header => Color::DarkGray,
                    };

                    let mut style = Style::default().fg(fg).bg(bg);
                    if is_cursor_hunk {
                        style = style.add_modifier(Modifier::REVERSED);
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
    use lajjzy_core::types::{DiffLine, DiffLineKind};

    use crate::action::HunkPickerOp;
    use crate::app::{HunkPicker, PickerFile, PickerHunk};

    fn read_row(buf: &Buffer, y: u16, width: u16) -> String {
        (0..width)
            .map(|x| buf[(x, y)].symbol().chars().next().unwrap_or(' '))
            .collect()
    }

    fn make_picker(files: Vec<PickerFile>) -> HunkPicker {
        HunkPicker {
            operation: HunkPickerOp::Split {
                source: "abc".into(),
            },
            files,
            cursor: 0,
            scroll: 0,
        }
    }

    fn sample_hunk(header: &str, selected: bool) -> PickerHunk {
        PickerHunk {
            header: header.into(),
            lines: vec![DiffLine {
                kind: DiffLineKind::Added,
                content: "new line".into(),
            }],
            selected,
        }
    }

    #[test]
    fn hunk_picker_renders_file_header() {
        let file = PickerFile {
            path: "src/lib.rs".into(),
            hunks: vec![sample_hunk("@@ -1,1 +1,2 @@", true)],
        };
        let picker = make_picker(vec![file]);
        let widget = HunkPickerWidget::new(&picker);
        let area = Rect::new(0, 0, 60, 5);
        let mut buf = Buffer::empty(area);
        widget.render(area, &mut buf);

        let row0 = read_row(&buf, 0, 60);
        assert!(row0.contains("src/lib.rs"), "file path missing: {row0:?}");
        assert!(row0.contains('▸'), "triangle prefix missing: {row0:?}");
        assert!(row0.contains("[1/1]"), "count missing: {row0:?}");
    }

    #[test]
    fn hunk_picker_renders_selected_and_unselected() {
        let file = PickerFile {
            path: "src/main.rs".into(),
            hunks: vec![
                sample_hunk("@@ -10,3 +10,5 @@", true),
                sample_hunk("@@ -25,2 +27,4 @@", false),
            ],
        };
        let picker = make_picker(vec![file]);
        let widget = HunkPickerWidget::new(&picker);
        let area = Rect::new(0, 0, 60, 10);
        let mut buf = Buffer::empty(area);
        widget.render(area, &mut buf);

        // Row 0: file header
        // Row 1: first hunk (selected) — [✓]
        // Row 2: diff line for hunk 0
        // Row 3: second hunk (unselected) — [ ]
        let row1 = read_row(&buf, 1, 60);
        assert!(row1.contains('✓'), "selected marker missing: {row1:?}");

        let row3 = read_row(&buf, 3, 60);
        // [ ] — the brackets and space should appear
        assert!(
            row3.contains('['),
            "unselected open bracket missing: {row3:?}"
        );
        assert!(
            row3.contains(']'),
            "unselected close bracket missing: {row3:?}"
        );
        // must NOT contain the checkmark
        assert!(
            !row3.contains('✓'),
            "unselected hunk should not have ✓: {row3:?}"
        );
    }

    #[test]
    fn hunk_picker_file_header_shows_count() {
        let file = PickerFile {
            path: "src/config.rs".into(),
            hunks: vec![
                sample_hunk("@@ -1,1 +1,2 @@", true),
                sample_hunk("@@ -5,1 +6,2 @@", true),
                sample_hunk("@@ -10,1 +12,2 @@", false),
            ],
        };
        let picker = make_picker(vec![file]);
        let widget = HunkPickerWidget::new(&picker);
        let area = Rect::new(0, 0, 60, 2);
        let mut buf = Buffer::empty(area);
        widget.render(area, &mut buf);

        let row0 = read_row(&buf, 0, 60);
        assert!(
            row0.contains("[2/3]"),
            "expected [2/3] count, got: {row0:?}"
        );
    }

    #[test]
    fn hunk_picker_renders_empty() {
        let picker = make_picker(vec![]);
        let widget = HunkPickerWidget::new(&picker);
        let area = Rect::new(0, 0, 40, 2);
        let mut buf = Buffer::empty(area);
        widget.render(area, &mut buf);

        let row0 = read_row(&buf, 0, 40);
        assert!(
            row0.contains("no hunks"),
            "expected empty message: {row0:?}"
        );
    }

    #[test]
    fn hunk_picker_cursor_on_second_hunk() {
        let file = PickerFile {
            path: "src/lib.rs".into(),
            hunks: vec![
                sample_hunk("@@ -1,1 +1,2 @@", false),
                sample_hunk("@@ -5,1 +6,2 @@", false),
            ],
        };
        let mut picker = make_picker(vec![file]);
        picker.cursor = 1; // second hunk (flat index 1)
        let widget = HunkPickerWidget::new(&picker);
        let area = Rect::new(0, 0, 60, 10);
        let mut buf = Buffer::empty(area);
        widget.render(area, &mut buf);

        // Row 3 is the second hunk header (row 0 = file header, row 1 = hunk 0, row 2 = diff line, row 3 = hunk 1)
        // The cursor hunk row should have REVERSED modifier
        let cell = &buf[(2, 3)]; // a character inside the second hunk row
        assert!(
            cell.modifier.contains(Modifier::REVERSED),
            "cursor hunk should have REVERSED modifier"
        );
    }
}
