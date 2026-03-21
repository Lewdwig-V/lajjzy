use std::env;
use std::sync::Arc;
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

use anyhow::{Context, Result, bail};
use crossterm::event::{self, Event, KeyEventKind};

use lajjzy_core::backend::RepoBackend;
use lajjzy_core::cli::JjCliBackend;
use lajjzy_tui::action::{Action, MutationKind};
use lajjzy_tui::app::AppState;
use lajjzy_tui::dispatch::dispatch;
use lajjzy_tui::effect::Effect;
use lajjzy_tui::input::{map_event, map_modal_event};
use lajjzy_tui::render::render;

struct EffectExecutor {
    backend: Arc<JjCliBackend>,
    tx: mpsc::Sender<Action>,
}

impl EffectExecutor {
    fn execute(&self, effect: Effect) {
        let backend = Arc::clone(&self.backend);
        let tx = self.tx.clone();
        thread::spawn(move || match effect {
            // Read-only effects
            Effect::LoadGraph { revset: _ } => {
                let result = backend.load_graph().map_err(|e| e.to_string());
                let _ = tx.send(Action::GraphLoaded(result));
            }
            Effect::LoadOpLog => {
                let result = backend.op_log().map_err(|e| e.to_string());
                let _ = tx.send(Action::OpLogLoaded(result));
            }
            Effect::LoadFileDiff { change_id, path } => {
                let result = backend
                    .file_diff(&change_id, &path)
                    .map_err(|e| e.to_string());
                let _ = tx.send(Action::FileDiffLoaded(result));
            }

            // Mutation effects
            Effect::Describe { change_id, text } => {
                run_mutation(&backend, &tx, MutationKind::Describe, || {
                    backend.describe(&change_id, &text)
                });
            }
            Effect::New { after } => {
                run_mutation(&backend, &tx, MutationKind::New, || {
                    backend.new_change(&after)
                });
            }
            Effect::Edit { change_id } => {
                run_mutation(&backend, &tx, MutationKind::Edit, || {
                    backend.edit_change(&change_id)
                });
            }
            Effect::Abandon { change_id } => {
                run_mutation(&backend, &tx, MutationKind::Abandon, || {
                    backend.abandon(&change_id)
                });
            }
            Effect::Squash { change_id } => {
                run_mutation(&backend, &tx, MutationKind::Squash, || {
                    backend.squash(&change_id)
                });
            }
            Effect::Undo => {
                run_mutation(&backend, &tx, MutationKind::Undo, || backend.undo());
            }
            Effect::Redo => {
                run_mutation(&backend, &tx, MutationKind::Redo, || backend.redo());
            }
            Effect::BookmarkSet { change_id, name } => {
                run_mutation(&backend, &tx, MutationKind::BookmarkSet, || {
                    backend.bookmark_set(&change_id, &name)
                });
            }
            Effect::BookmarkDelete { name } => {
                run_mutation(&backend, &tx, MutationKind::BookmarkDelete, || {
                    backend.bookmark_delete(&name)
                });
            }
            Effect::GitPush { bookmark } => {
                run_mutation(&backend, &tx, MutationKind::GitPush, || {
                    backend.git_push(&bookmark)
                });
            }
            Effect::GitFetch => {
                run_mutation(&backend, &tx, MutationKind::GitFetch, || {
                    backend.git_fetch()
                });
            }

            // SuspendForEditor is intercepted before reaching the executor
            Effect::SuspendForEditor { .. } => {
                unreachable!("SuspendForEditor must be intercepted by execute_effects")
            }
        });
    }
}

fn run_mutation(
    backend: &JjCliBackend,
    tx: &mpsc::Sender<Action>,
    op: MutationKind,
    f: impl FnOnce() -> anyhow::Result<String>,
) {
    match f() {
        Ok(message) => {
            let _ = tx.send(Action::RepoOpSuccess { op, message });
            let graph = backend.load_graph().map_err(|e| e.to_string());
            let _ = tx.send(Action::GraphLoaded(graph));
        }
        Err(e) => {
            let _ = tx.send(Action::RepoOpFailed {
                op,
                error: e.to_string(),
            });
        }
    }
}

fn execute_effects(
    terminal: &mut ratatui::DefaultTerminal,
    state: &mut AppState,
    executor: &EffectExecutor,
    effects: Vec<Effect>,
) {
    for effect in effects {
        match effect {
            Effect::SuspendForEditor {
                change_id,
                initial_text,
            } => {
                ratatui::restore();
                let result = run_editor(&initial_text);
                *terminal = ratatui::init();
                match result {
                    Ok(text) => {
                        let new_effects =
                            dispatch(state, Action::EditorComplete { change_id, text });
                        execute_effects(terminal, state, executor, new_effects);
                    }
                    Err(e) => {
                        state.error = Some(format!("Editor failed: {e}"));
                    }
                }
            }
            other => executor.execute(other),
        }
    }
}

fn run_editor(initial_text: &str) -> anyhow::Result<String> {
    let editor = std::env::var("EDITOR")
        .or_else(|_| std::env::var("VISUAL"))
        .unwrap_or_else(|_| "vi".to_string());
    let tmp = tempfile::NamedTempFile::new()?;
    std::fs::write(tmp.path(), initial_text)?;
    let status = std::process::Command::new(&editor)
        .arg(tmp.path())
        .status()
        .with_context(|| format!("Failed to launch editor: {editor}"))?;
    if !status.success() {
        bail!("Editor exited with status {status}");
    }
    Ok(std::fs::read_to_string(tmp.path())?)
}

/// Convert a crossterm 0.29 `KeyEvent` into tui-textarea's backend-agnostic `Input`.
/// This bridge is needed because tui-textarea 0.7 depends on crossterm 0.28,
/// whose `KeyEvent` type is distinct from the crossterm 0.29 used by ratatui 0.30.
fn key_event_to_textarea_input(key: crossterm::event::KeyEvent) -> tui_textarea::Input {
    use crossterm::event::{KeyCode, KeyModifiers};
    use tui_textarea::{Input, Key};

    let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
    let alt = key.modifiers.contains(KeyModifiers::ALT);
    let shift = key.modifiers.contains(KeyModifiers::SHIFT);

    let textarea_key = match key.code {
        KeyCode::Char(c) => Key::Char(c),
        KeyCode::Backspace => Key::Backspace,
        KeyCode::Enter => Key::Enter,
        KeyCode::Left => Key::Left,
        KeyCode::Right => Key::Right,
        KeyCode::Up => Key::Up,
        KeyCode::Down => Key::Down,
        KeyCode::Tab => Key::Tab,
        KeyCode::Delete => Key::Delete,
        KeyCode::Home => Key::Home,
        KeyCode::End => Key::End,
        KeyCode::PageUp => Key::PageUp,
        KeyCode::PageDown => Key::PageDown,
        KeyCode::Esc => Key::Esc,
        KeyCode::F(n) => Key::F(n),
        _ => Key::Null,
    };

    Input {
        key: textarea_key,
        ctrl,
        alt,
        shift,
    }
}

fn main() -> Result<()> {
    let cwd = env::current_dir().context("Failed to get current directory")?;
    let backend = Arc::new(JjCliBackend::new(&cwd).context("Failed to open jj workspace")?);

    let graph = backend.load_graph().context("Failed to load graph")?;
    let mut state = AppState::new(graph);

    let (tx, rx) = mpsc::channel();
    let executor = EffectExecutor { backend, tx };

    let original_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let _ = ratatui::try_restore();
        original_hook(info);
    }));

    let mut terminal = ratatui::init();
    let result = run_loop(&mut terminal, &mut state, &executor, &rx);
    ratatui::restore();
    result
}

fn run_loop(
    terminal: &mut ratatui::DefaultTerminal,
    state: &mut AppState,
    executor: &EffectExecutor,
    rx: &mpsc::Receiver<Action>,
) -> Result<()> {
    loop {
        terminal.draw(|frame| render(frame, state))?;

        if crossterm::event::poll(Duration::from_millis(50))?
            && let Event::Key(key_event) = event::read()?
        {
            if key_event.kind != KeyEventKind::Press {
                continue;
            }
            state.status_message = None;
            let action = if let Some(ref modal) = state.modal {
                map_modal_event(key_event, modal)
            } else {
                map_event(key_event, state.focus, state.detail_mode)
            };
            if let Some(action) = action {
                let effects = dispatch(state, action);
                execute_effects(terminal, state, executor, effects);
            } else if let Some(lajjzy_tui::modal::Modal::Describe { ref mut editor, .. }) =
                state.modal
            {
                // Unhandled key in describe modal — forward to tui-textarea.
                // Convert crossterm 0.29 KeyEvent to tui-textarea's Input manually
                // since tui-textarea 0.7 uses crossterm 0.28.
                let input = key_event_to_textarea_input(key_event);
                editor.input(input);
            }
        }

        // Drain all pending results before next render
        while let Ok(action) = rx.try_recv() {
            let effects = dispatch(state, action);
            execute_effects(terminal, state, executor, effects);
        }

        if state.should_quit {
            break;
        }
    }
    Ok(())
}
