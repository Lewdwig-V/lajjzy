use std::env;

use anyhow::{Context, Result};
use crossterm::event::{self, Event, KeyEventKind};

use lajjzy_core::backend::RepoBackend;
use lajjzy_core::cli::JjCliBackend;
use lajjzy_tui::app::AppState;
use lajjzy_tui::dispatch::dispatch;
use lajjzy_tui::input::{map_event, map_modal_event};
use lajjzy_tui::render::render;

fn main() -> Result<()> {
    let cwd = env::current_dir().context("Failed to get current directory")?;
    let backend = JjCliBackend::new(&cwd).context("Failed to open jj workspace")?;

    let graph = backend.load_graph().context("Failed to load graph")?;
    drop(backend); // TODO(Task 5): backend moves to effect executor
    let mut state = AppState::new(graph);

    let original_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let _ = ratatui::try_restore();
        original_hook(info);
    }));

    let mut terminal = ratatui::init();

    let result = run_loop(&mut terminal, &mut state);

    ratatui::restore();

    result
}

fn run_loop(terminal: &mut ratatui::DefaultTerminal, state: &mut AppState) -> Result<()> {
    loop {
        terminal.draw(|frame| render(frame, state))?;

        if let Event::Key(key_event) = event::read()? {
            if key_event.kind != KeyEventKind::Press {
                continue;
            }
            if let Some(action) = if let Some(ref modal) = state.modal {
                map_modal_event(key_event, modal)
            } else {
                map_event(key_event, state.focus, state.detail_mode)
            } {
                let effects = dispatch(state, action);
                // TODO(Task 5): wire effects to executor — binary is non-functional until then
                let _ = effects;
            }
        }

        if state.should_quit {
            break;
        }
    }
    Ok(())
}
