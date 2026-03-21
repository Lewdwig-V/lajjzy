use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::widgets::Clear;

use crate::app::{AppState, Modal};
use crate::panels;
use crate::widgets::status_bar::StatusBarWidget;

const STATUS_BAR_HEIGHT: u16 = 2;

pub fn render(frame: &mut Frame, state: &AppState) {
    let outer = Layout::vertical([Constraint::Min(1), Constraint::Length(STATUS_BAR_HEIGHT)])
        .split(frame.area());

    let main =
        Layout::horizontal([Constraint::Ratio(1, 3), Constraint::Ratio(2, 3)]).split(outer[0]);

    panels::graph::render(frame, state, main[0]);
    panels::detail::render(frame, state, main[1]);

    let change_id = state.selected_change_id();
    let detail = state.selected_detail();
    let error = state.error.as_deref();
    let status_message = state.status_message.as_deref();
    let status_widget = StatusBarWidget::new(
        change_id,
        detail,
        error,
        status_message,
        &state.pending_background,
    );
    frame.render_widget(status_widget, outer[1]);

    // Modal overlay
    if state.modal.is_some() {
        // Describe modal renders over the detail pane (right side)
        if matches!(state.modal, Some(Modal::Describe { .. })) {
            render_modal(frame, state, main[1]);
        } else if matches!(state.modal, Some(Modal::BookmarkInput { .. })) {
            // Lightweight input bar — no background dim, renders at bottom of screen
            render_modal(frame, state, frame.area());
        } else {
            let dim = Style::default().add_modifier(Modifier::DIM);
            let area = outer[0];
            for y in area.y..area.y + area.height {
                for x in area.x..area.x + area.width {
                    frame.buffer_mut()[(x, y)].set_style(dim);
                }
            }
            render_modal(frame, state, area);
        }
    }
}

fn render_modal(frame: &mut Frame, state: &AppState, area: Rect) {
    let Some(modal) = state.modal.as_ref() else {
        return;
    };
    match modal {
        Modal::OpLog {
            entries,
            cursor,
            scroll,
        } => {
            frame.render_widget(Clear, area);
            let widget = crate::widgets::op_log::OpLogWidget::new(entries, *cursor, *scroll);
            frame.render_widget(widget, area);
        }
        Modal::BookmarkPicker { bookmarks, cursor } => {
            let modal_area = centered_rect(60, 80, area);
            frame.render_widget(Clear, modal_area);
            let widget = crate::widgets::bookmark_picker::BookmarkPickerWidget::new(
                bookmarks,
                &state.graph.details,
                *cursor,
            );
            frame.render_widget(widget, modal_area);
        }
        Modal::FuzzyFind {
            query,
            matches,
            cursor,
        } => {
            let modal_area = centered_rect(60, 80, area);
            frame.render_widget(Clear, modal_area);
            let widget = crate::widgets::fuzzy_find::FuzzyFindWidget::new(
                query,
                matches,
                &state.graph,
                *cursor,
            );
            frame.render_widget(widget, modal_area);
        }
        Modal::Help { context, scroll } => {
            let modal_area = centered_rect(50, 60, area);
            frame.render_widget(Clear, modal_area);
            let widget = crate::widgets::help::HelpWidget::new(*context, *scroll);
            frame.render_widget(widget, modal_area);
        }
        Modal::Describe { editor, .. } => {
            frame.render_widget(Clear, area);
            let widget = crate::widgets::describe::DescribeWidget::new(editor);
            frame.render_widget(widget, area);
        }
        Modal::BookmarkInput {
            input, completions, ..
        } => {
            // Render as a bottom bar: 4 rows tall (border + input + completions + border)
            let bar_height: u16 = 4;
            let bar_y = area.y + area.height.saturating_sub(bar_height);
            let bar_area = Rect::new(area.x, bar_y, area.width, bar_height.min(area.height));
            frame.render_widget(Clear, bar_area);
            let widget =
                crate::widgets::bookmark_input::BookmarkInputWidget::new(input, completions);
            frame.render_widget(widget, bar_area);
        }
    }
}

fn centered_rect(percent_x: u16, percent_y: u16, area: Rect) -> Rect {
    let popup_layout = Layout::vertical([
        Constraint::Percentage((100 - percent_y) / 2),
        Constraint::Percentage(percent_y),
        Constraint::Percentage((100 - percent_y) / 2),
    ])
    .split(area);
    Layout::horizontal([
        Constraint::Percentage((100 - percent_x) / 2),
        Constraint::Percentage(percent_x),
        Constraint::Percentage((100 - percent_x) / 2),
    ])
    .split(popup_layout[1])[1]
}
