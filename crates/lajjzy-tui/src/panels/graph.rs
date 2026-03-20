use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Color, Style};
use ratatui::widgets::{Block, Borders};

use crate::app::{AppState, PanelFocus};
use crate::widgets::graph::GraphWidget;

pub fn render(frame: &mut Frame, state: &AppState, area: Rect) {
    let focused = state.focus == PanelFocus::Graph;
    let border_style = if focused {
        Style::default().fg(Color::Blue)
    } else {
        Style::default().fg(Color::DarkGray)
    };

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(border_style)
        .title("Changes");

    let inner = block.inner(area);
    frame.render_widget(block, area);

    let graph_widget = GraphWidget::new(&state.graph, state.cursor());
    frame.render_widget(graph_widget, inner);
}
