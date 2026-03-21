use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Color, Style};
use ratatui::widgets::{Block, Borders};

use crate::app::{AppState, DetailMode, PanelFocus};
use crate::widgets::diff_view::DiffViewWidget;
use crate::widgets::file_list::FileListWidget;

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
        // HunkPicker rendered by its own widget in Task 7
        DetailMode::HunkPicker => "Hunk Picker".to_string(),
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
        // HunkPicker rendered by its own widget in Task 7
        DetailMode::HunkPicker => {}
    }
}
