use std::env;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use anyhow::{Context, Result, bail};
use clap::Parser;
use crossterm::event::{self, Event, KeyEventKind};

use lajjzy_core::backend::RepoBackend;
use lajjzy_core::cli::JjCliBackend;
use lajjzy_tui::action::{Action, MutationKind};
use lajjzy_tui::app::AppState;
use lajjzy_tui::dispatch::dispatch;
use lajjzy_tui::effect::Effect;
use lajjzy_tui::input::{map_event, map_modal_event, map_picking_event};
use lajjzy_tui::render::render;

struct EffectExecutor {
    backend: Arc<JjCliBackend>,
    tx: mpsc::Sender<Action>,
    /// Monotonic counter for graph snapshot versioning.
    /// Incremented before each `load_graph()` call so later loads get higher generations.
    graph_generation: AtomicU64,
    /// The active revset filter, synced from dispatch state after each action.
    /// Read by mutation threads at completion time (not snapshot time) so the
    /// refreshed graph reflects the filter the user sees when the mutation finishes.
    active_revset: Arc<Mutex<Option<String>>>,
}

impl EffectExecutor {
    /// Spawn a background thread to execute the effect.
    ///
    /// `let _ = tx.send(...)` is intentional: if the receiver is dropped (event loop
    /// exited or panicked), the send fails harmlessly. The spawned thread has no other
    /// work to do and will exit. This is the expected shutdown race, not a silent failure.
    #[expect(clippy::too_many_lines)]
    fn execute(&self, effect: Effect) {
        let backend = Arc::clone(&self.backend);
        let tx = self.tx.clone();
        // Assign generation BEFORE spawning thread — ordering reflects intent, not completion.
        let generation = self.next_graph_generation(&effect);
        // Clone the Arc so mutation threads read the current revset at completion
        // time, not at spawn time — prevents stale filter after user changes it.
        let active_revset = Arc::clone(&self.active_revset);
        thread::spawn(move || match effect {
            // Read-only effects
            Effect::LoadGraph { revset } => {
                let result = backend
                    .load_graph(revset.as_deref())
                    .map_err(|e| e.to_string());
                let _ = tx.send(Action::GraphLoaded { generation, result });
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
                run_mutation(
                    &backend,
                    &tx,
                    MutationKind::Describe,
                    generation,
                    &active_revset,
                    || backend.describe(&change_id, &text),
                );
            }
            Effect::New { after } => {
                run_mutation(
                    &backend,
                    &tx,
                    MutationKind::New,
                    generation,
                    &active_revset,
                    || backend.new_change(&after),
                );
            }
            Effect::Edit { change_id } => {
                run_mutation(
                    &backend,
                    &tx,
                    MutationKind::Edit,
                    generation,
                    &active_revset,
                    || backend.edit_change(&change_id),
                );
            }
            Effect::Abandon { change_id } => {
                run_mutation(
                    &backend,
                    &tx,
                    MutationKind::Abandon,
                    generation,
                    &active_revset,
                    || backend.abandon(&change_id),
                );
            }
            Effect::LoadChangeDiff {
                change_id,
                operation,
            } => {
                let result = backend.change_diff(&change_id).map_err(|e| e.to_string());
                let _ = tx.send(Action::ChangeDiffLoaded { operation, result });
            }
            Effect::Split {
                change_id,
                selections,
            } => {
                run_mutation(
                    &backend,
                    &tx,
                    MutationKind::Split,
                    generation,
                    &active_revset,
                    || backend.split(&change_id, &selections),
                );
            }
            Effect::SquashPartial {
                change_id,
                selections,
            } => {
                run_mutation(
                    &backend,
                    &tx,
                    MutationKind::SquashPartial,
                    generation,
                    &active_revset,
                    || backend.squash_partial(&change_id, &selections),
                );
            }
            Effect::Undo => {
                run_mutation(
                    &backend,
                    &tx,
                    MutationKind::Undo,
                    generation,
                    &active_revset,
                    || backend.undo(),
                );
            }
            Effect::Redo => {
                run_mutation(
                    &backend,
                    &tx,
                    MutationKind::Redo,
                    generation,
                    &active_revset,
                    || backend.redo(),
                );
            }
            Effect::BookmarkSet { change_id, name } => {
                run_mutation(
                    &backend,
                    &tx,
                    MutationKind::BookmarkSet,
                    generation,
                    &active_revset,
                    || backend.bookmark_set(&change_id, &name),
                );
            }
            Effect::BookmarkDelete { name } => {
                run_mutation(
                    &backend,
                    &tx,
                    MutationKind::BookmarkDelete,
                    generation,
                    &active_revset,
                    || backend.bookmark_delete(&name),
                );
            }
            Effect::GitPush { bookmark } => {
                run_mutation(
                    &backend,
                    &tx,
                    MutationKind::GitPush,
                    generation,
                    &active_revset,
                    || backend.git_push(&bookmark),
                );
            }
            Effect::GitFetch => {
                run_mutation(
                    &backend,
                    &tx,
                    MutationKind::GitFetch,
                    generation,
                    &active_revset,
                    || backend.git_fetch(),
                );
            }

            // EvalRevset: test the query then report back as RevsetLoaded
            // TODO: wired up fully in Task 5
            Effect::EvalRevset { query } => {
                let result = backend.load_graph(Some(&query)).map_err(|e| e.to_string());
                let _ = tx.send(Action::RevsetLoaded {
                    query,
                    generation,
                    result,
                });
            }

            // Rebase effects
            Effect::RebaseSingle {
                source,
                destination,
            } => {
                run_mutation(
                    &backend,
                    &tx,
                    MutationKind::RebaseSingle,
                    generation,
                    &active_revset,
                    || backend.rebase_single(&source, &destination),
                );
            }
            Effect::RebaseWithDescendants {
                source,
                destination,
            } => {
                run_mutation(
                    &backend,
                    &tx,
                    MutationKind::RebaseWithDescendants,
                    generation,
                    &active_revset,
                    || backend.rebase_with_descendants(&source, &destination),
                );
            }

            // Conflict handling
            Effect::LoadConflictData { change_id, path } => {
                let result = backend
                    .conflict_sides(&change_id, &path)
                    .map_err(|e| e.to_string());
                let _ = tx.send(Action::ConflictDataLoaded {
                    change_id,
                    path,
                    result,
                });
            }
            Effect::ResolveFile {
                change_id,
                path,
                content,
            } => {
                // `content` (Vec<u8>) must be moved into the closure since
                // resolve_file takes it by value.  We rebind backend so the
                // move closure doesn't steal the outer Arc that run_mutation
                // also borrows.
                let be = &*backend;
                run_mutation(
                    be,
                    &tx,
                    MutationKind::ResolveConflict,
                    generation,
                    &active_revset,
                    || be.resolve_file(&change_id, &path, content),
                );
            }

            // LaunchMergeTool and SuspendForEditor are intercepted before reaching the executor
            Effect::LaunchMergeTool { .. } => {
                unreachable!("LaunchMergeTool must be intercepted by execute_effects")
            }
            Effect::SuspendForEditor { .. } => {
                unreachable!("SuspendForEditor must be intercepted by execute_effects")
            }
        });
    }

    /// Increment the generation counter for effects that will load a graph.
    /// Returns 0 for effects that don't load graphs (the value is ignored).
    fn next_graph_generation(&self, effect: &Effect) -> u64 {
        match effect {
            Effect::LoadGraph { .. }
            | Effect::Describe { .. }
            | Effect::New { .. }
            | Effect::Edit { .. }
            | Effect::Abandon { .. }
            | Effect::Split { .. }
            | Effect::SquashPartial { .. }
            | Effect::Undo
            | Effect::Redo
            | Effect::BookmarkSet { .. }
            | Effect::BookmarkDelete { .. }
            | Effect::GitPush { .. }
            | Effect::GitFetch
            | Effect::EvalRevset { .. }
            | Effect::RebaseSingle { .. }
            | Effect::RebaseWithDescendants { .. }
            | Effect::ResolveFile { .. } => {
                self.graph_generation.fetch_add(1, Ordering::SeqCst) + 1
            }
            Effect::LoadOpLog
            | Effect::LoadFileDiff { .. }
            | Effect::LoadChangeDiff { .. }
            | Effect::LoadConflictData { .. }
            | Effect::LaunchMergeTool { .. }
            | Effect::SuspendForEditor { .. } => 0,
        }
    }
}

fn run_mutation(
    backend: &JjCliBackend,
    tx: &mpsc::Sender<Action>,
    op: MutationKind,
    generation: u64,
    active_revset: &Mutex<Option<String>>,
    f: impl FnOnce() -> anyhow::Result<String>,
) {
    match f() {
        Ok(message) => {
            // Read the active revset NOW (at mutation completion time), not at
            // spawn time — so the refreshed graph reflects the filter the user
            // currently sees, even if they changed it while the mutation ran.
            let revset = active_revset
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .clone();
            let graph = Some((
                generation,
                backend
                    .load_graph(revset.as_deref())
                    .map_err(|e| e.to_string()),
            ));
            let _ = tx.send(Action::RepoOpSuccess { op, message, graph });
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
            Effect::LaunchMergeTool { change_id: _, path } => {
                ratatui::restore();
                let status = std::process::Command::new("jj")
                    .args(["resolve", &path, "-r", "@"])
                    .current_dir(executor.backend.workspace_root())
                    .status();
                *terminal = ratatui::init();
                match status {
                    Ok(s) if s.success() => {
                        let generation =
                            executor.graph_generation.fetch_add(1, Ordering::SeqCst) + 1;
                        let revset = executor
                            .active_revset
                            .lock()
                            .unwrap_or_else(std::sync::PoisonError::into_inner)
                            .clone();
                        let graph_result = executor
                            .backend
                            .load_graph(revset.as_deref())
                            .map_err(|e| e.to_string());
                        let new_effects = dispatch(
                            state,
                            Action::MergeToolComplete {
                                path,
                                graph: Some((generation, graph_result)),
                            },
                        );
                        execute_effects(terminal, state, executor, new_effects);
                    }
                    Ok(s) => {
                        let code = s.code().unwrap_or(-1);
                        let new_effects = dispatch(
                            state,
                            Action::MergeToolFailed {
                                path,
                                error: format!("Merge tool exited with status {code}"),
                            },
                        );
                        execute_effects(terminal, state, executor, new_effects);
                    }
                    Err(e) => {
                        let new_effects = dispatch(
                            state,
                            Action::MergeToolFailed {
                                path,
                                error: format!("Failed to launch merge tool: {e}"),
                            },
                        );
                        execute_effects(terminal, state, executor, new_effects);
                    }
                }
            }
            other => executor.execute(other),
        }
    }
}

fn run_editor(initial_text: &str) -> anyhow::Result<String> {
    let editor_var = std::env::var("EDITOR")
        .or_else(|_| std::env::var("VISUAL"))
        .unwrap_or_else(|_| "vi".to_string());

    // Parse $EDITOR to support "code --wait", "nvim -f", "emacsclient -c", etc.
    let mut parts = editor_var.split_whitespace();
    let program = parts.next().context("$EDITOR is empty")?;
    let editor_args: Vec<&str> = parts.collect();

    let tmp = tempfile::NamedTempFile::new()?;
    std::fs::write(tmp.path(), initial_text)?;
    let status = std::process::Command::new(program)
        .args(&editor_args)
        .arg(tmp.path())
        .status()
        .with_context(|| format!("Failed to launch editor: {editor_var}"))?;
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

/// A keyboard-driven, lazygit-style TUI for Jujutsu (jj)
#[derive(Parser)]
#[command(version, about)]
struct Cli {}

fn main() -> Result<()> {
    let _cli = Cli::parse();

    let cwd = env::current_dir().context("Failed to get current directory")?;
    let backend = Arc::new(JjCliBackend::new(&cwd).context("Failed to open jj workspace")?);

    let graph = backend.load_graph(None).context("Failed to load graph")?;
    let mut state = AppState::new(graph);

    let (tx, rx) = mpsc::channel();
    let executor = EffectExecutor {
        backend,
        tx,
        graph_generation: AtomicU64::new(0),
        active_revset: Arc::new(Mutex::new(None)),
    };

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

        // Update hunk picker viewport height from terminal size so dispatch
        // can adjust scroll. The detail pane is ~2/3 width, full height minus
        // status bar (2) and borders (2). Approximate with terminal height - 4.
        if let Some(ref mut hp) = state.hunk_picker {
            let term_height = terminal.size().map_or(20, |s| s.height as usize);
            hp.viewport_height = term_height.saturating_sub(4);
        }
        if let Some(ref mut cv) = state.conflict_view {
            let term_height = terminal.size().map_or(20, |s| s.height as usize);
            cv.viewport_height = term_height.saturating_sub(4);
        }

        if crossterm::event::poll(Duration::from_millis(50))?
            && let Event::Key(key_event) = event::read()?
        {
            if key_event.kind != KeyEventKind::Press {
                continue;
            }
            state.status_message = None;

            // Picking mode intercepts all keys before modal/normal routing.
            if state.target_pick.is_some() {
                let picking = state.target_pick.as_ref().unwrap().picking.clone();
                if let Some(action) = map_picking_event(key_event, &picking) {
                    let effects = dispatch(state, action);
                    executor
                        .active_revset
                        .lock()
                        .unwrap_or_else(std::sync::PoisonError::into_inner)
                        .clone_from(&state.active_revset);
                    execute_effects(terminal, state, executor, effects);
                }
                continue; // swallow unhandled keys during picking
            }

            let action = if let Some(ref modal) = state.modal {
                map_modal_event(key_event, modal)
            } else {
                map_event(key_event, state.focus, state.detail_mode)
            };
            if let Some(action) = action {
                let effects = dispatch(state, action);
                executor
                    .active_revset
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner)
                    .clone_from(&state.active_revset);
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
            executor
                .active_revset
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .clone_from(&state.active_revset);
            execute_effects(terminal, state, executor, effects);
        }

        if state.should_quit {
            break;
        }
    }
    Ok(())
}
