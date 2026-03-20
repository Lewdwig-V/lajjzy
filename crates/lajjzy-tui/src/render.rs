use ratatui::Frame;
use ratatui::layout::{Constraint, Layout};

use crate::app::AppState;
use crate::widgets::graph::GraphWidget;
use crate::widgets::status_bar::StatusBarWidget;

const STATUS_BAR_HEIGHT: u16 = 2;

pub fn render(frame: &mut Frame, state: &AppState) {
    let chunks = Layout::vertical([Constraint::Min(1), Constraint::Length(STATUS_BAR_HEIGHT)])
        .split(frame.area());

    let graph_widget = GraphWidget::new(&state.graph, state.cursor());
    frame.render_widget(graph_widget, chunks[0]);

    let change_id = state.selected_change_id();
    let detail = state.selected_detail();
    let error = state.error.as_deref();
    let status_widget = StatusBarWidget::new(change_id, detail, error);
    frame.render_widget(status_widget, chunks[1]);
}
