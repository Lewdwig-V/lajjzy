from __future__ import annotations

from dataclasses import replace

from lajjzy.core.commands import Cmd, EditMessage, LoadGraph, RunMutation
from lajjzy.core.messages import (
    Abandon,
    CursorBottom,
    CursorDown,
    CursorTop,
    CursorUp,
    DescribeAborted,
    DescribeReady,
    DescribeRequested,
    EditChange,
    GraphLoaded,
    GraphLoadFailed,
    Msg,
    MutationCompleted,
    MutationFailed,
    NewChange,
    RebaseCancel,
    RebaseConfirm,
    RebaseStart,
    ReloadRequested,
    Squash,
)
from lajjzy.core.model import Model, cursor_after_reload, selected_change_id, step_cursor

_REBASE_PROMPT = "Rebase: pick a destination, Enter to confirm, Esc to cancel"
_REBASE_DESC_PROMPT = "Rebase +desc: pick a destination, Enter to confirm, Esc to cancel"


def update(model: Model, msg: Msg) -> tuple[Model, list[Cmd]]:
    """The pure heart of the application: ``(Model, Msg) -> (Model, [Cmd])``.

    No I/O, no async, no Textual. Given the current state and a message, it
    returns the next state and a list of effects for the backend to run. Every
    state transition in the app flows through here and is unit-testable in
    isolation.
    """
    # --- navigation -------------------------------------------------------
    if isinstance(msg, CursorDown):
        return replace(model, cursor=step_cursor(model, 1)), []
    if isinstance(msg, CursorUp):
        return replace(model, cursor=step_cursor(model, -1)), []
    if isinstance(msg, CursorTop):
        if model.graph and model.graph.node_indices:
            return replace(model, cursor=model.graph.node_indices[0]), []
        return model, []
    if isinstance(msg, CursorBottom):
        if model.graph and model.graph.node_indices:
            return replace(model, cursor=model.graph.node_indices[-1]), []
        return model, []

    # --- graph reload -----------------------------------------------------
    if isinstance(msg, ReloadRequested):
        epoch = model.graph_epoch + 1
        return replace(model, graph_epoch=epoch), [LoadGraph(epoch)]
    if isinstance(msg, GraphLoaded):
        if msg.epoch != model.graph_epoch:
            return model, []  # superseded by a newer load; discard
        return replace(
            model, error=None, graph=msg.graph, cursor=cursor_after_reload(msg.graph)
        ), []
    if isinstance(msg, GraphLoadFailed):
        return replace(model, error=msg.error), []

    # --- mutations --------------------------------------------------------
    if isinstance(msg, NewChange):
        target = selected_change_id(model)
        return _start_mutation(model, "new", None if target is None else (target,))
    if isinstance(msg, Abandon):
        target = selected_change_id(model)
        return _start_mutation(model, "abandon", None if target is None else (target,))
    if isinstance(msg, EditChange):
        target = selected_change_id(model)
        return _start_mutation(model, "edit", None if target is None else (target,))
    if isinstance(msg, Squash):
        target = selected_change_id(model)
        return _start_mutation(model, "squash", None if target is None else (target,))
    if isinstance(msg, MutationFailed):
        return replace(model, error=msg.error, pending_mutation=False), []
    if isinstance(msg, MutationCompleted):
        return _mutation_completed(model, msg), []

    # --- describe (mutation gated behind an editor round-trip) ------------
    if isinstance(msg, DescribeRequested):
        target = selected_change_id(model)
        if target is None or model.graph is None:
            return replace(model, error="No change selected"), []
        detail = model.graph.details.get(target)
        if detail is None:
            return model, []
        return model, [EditMessage(target, detail.description)]
    if isinstance(msg, DescribeReady):
        return _start_mutation(model, "describe", (msg.change_id, msg.text))
    if isinstance(msg, DescribeAborted):
        if msg.error is None:
            return model, []
        return replace(model, error=msg.error), []

    # --- rebase mode ------------------------------------------------------
    if isinstance(msg, RebaseStart):
        src = selected_change_id(model)
        if src is None:
            return replace(
                model,
                rebase_source=None,
                rebase_descendants=msg.descendants,
                error="No change selected",
            ), []
        prompt = _REBASE_DESC_PROMPT if msg.descendants else _REBASE_PROMPT
        return replace(
            model, rebase_source=src, rebase_descendants=msg.descendants, error=prompt
        ), []
    if isinstance(msg, RebaseConfirm):
        if model.rebase_source is None:
            return model, []  # Enter does nothing unless rebase mode is armed
        dest = selected_change_id(model)
        src = model.rebase_source
        descend = model.rebase_descendants
        cleared = replace(model, rebase_source=None)
        if dest is None or dest == src:
            return replace(cleared, error="Rebase cancelled (invalid destination)"), []
        kind = "rebase_descendants" if descend else "rebase"
        return _start_mutation(cleared, kind, (src, dest))
    if isinstance(msg, RebaseCancel):
        if model.rebase_source is not None:
            return replace(model, rebase_source=None, error="Rebase cancelled"), []
        return model, []

    return model, []


def _start_mutation(model: Model, kind: str, args: tuple | None) -> tuple[Model, list[Cmd]]:
    """Gate and launch a write op. ``args is None`` means nothing is selected."""
    if args is None:
        return replace(model, error="No change selected"), []
    if model.pending_mutation:
        return replace(model, error="A mutation is already in progress"), []
    epoch = model.graph_epoch + 1
    return replace(model, pending_mutation=True, graph_epoch=epoch), [
        RunMutation(epoch, kind, args)
    ]


def _mutation_completed(model: Model, msg: MutationCompleted) -> Model:
    # The mutation worker has finished, so the gate always reopens here.
    if msg.load_error is not None:
        return replace(model, error=msg.load_error, pending_mutation=False)
    reported = replace(model, error=msg.message, pending_mutation=False)
    # Keep the success message but discard a superseded / failed reload.
    if msg.graph is None or msg.epoch != model.graph_epoch:
        return reported
    return replace(reported, graph=msg.graph, cursor=cursor_after_reload(msg.graph))
