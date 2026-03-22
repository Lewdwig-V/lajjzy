use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Color, Style};
use ratatui::widgets::{Block, Borders};

use crate::app::{AppState, DetailMode, PanelFocus};
use crate::widgets::diff_view::DiffViewWidget;
use crate::widgets::file_list::FileListWidget;
use crate::widgets::hunk_picker::HunkPickerWidget;

pub fn render(frame: &mut Frame, state: &AppState, area: Rect) {
    let focused = state.focus == PanelFocus::Detail;
    let border_style = if focused {
        Style::default().fg(Color::Blue)
    } else {
        Style::default().fg(Color::DarkGray)
    };

    let title = match state.detail_mode {
        DetailMode::FileList => {
            if let Some(detail) = state.selected_detail() {
                let desc = if detail.description.is_empty() {
                    "(no description)"
                } else {
                    &detail.description
                };
                format!("Files — {desc}")
            } else {
                "Files".to_string()
            }
        }
        DetailMode::DiffView => {
            let path = state
                .selected_detail()
                .and_then(|d| d.files.get(state.detail_cursor()))
                .map_or("", |f| f.path.as_str());
            if path.is_empty() {
                "Diff".to_string()
            } else {
                format!("Diff — {path}")
            }
        }
        DetailMode::HunkPicker => {
            if let Some(hp) = state.hunk_picker.as_ref() {
                match &hp.operation {
                    crate::action::HunkPickerOp::Split { source } => {
                        format!("Split — {source}")
                    }
                    crate::action::HunkPickerOp::Squash {
                        source,
                        destination,
                    } => {
                        format!("Squash — {source} → {destination}")
                    }
                }
            } else {
                "Hunk Picker".to_string()
            }
        }
        DetailMode::ConflictView => {
            if let Some(cv) = state.conflict_view.as_ref() {
                let hunk_num = cv.cursor + 1;
                let total = cv.resolutions.len();
                format!("Conflict — {} (hunk {}/{})", cv.path, hunk_num, total)
            } else {
                "Conflict".to_string()
            }
        }
    };

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(border_style)
        .title(title);

    let inner = block.inner(area);
    frame.render_widget(block, area);

    match state.detail_mode {
        DetailMode::FileList => {
            let files = state
                .selected_detail()
                .map_or(&[][..], |d| d.files.as_slice());
            let widget = FileListWidget::new(files, state.detail_cursor(), focused);
            frame.render_widget(widget, inner);
        }
        DetailMode::DiffView => {
            let widget = DiffViewWidget::new(&state.diff_data, state.diff_scroll);
            frame.render_widget(widget, inner);
        }
        DetailMode::HunkPicker => {
            if let Some(hp) = state.hunk_picker.as_ref() {
                let widget = HunkPickerWidget::new(hp);
                frame.render_widget(widget, inner);
            }
        }
        DetailMode::ConflictView => {
            // Widget rendering added in Task 7
        }
    }
}
