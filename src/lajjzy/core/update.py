from __future__ import annotations

from dataclasses import replace
from typing import Any

from lajjzy.core.commands import (
    Cmd,
    EditMessage,
    LoadBookmarks,
    LoadConflictData,
    LoadGraph,
    LoadOpLog,
    RunMutation,
)
from lajjzy.core.messages import (
    Abandon,
    ApplyResolutions,
    BookmarkDelete,
    BookmarkInputCancel,
    BookmarkInputConfirm,
    BookmarkMoveConfirm,
    BookmarksLoadFailed,
    BookmarksLoaded,
    ConflictDataLoadFailed,
    ConflictDataLoaded,
    ConflictViewClose,
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
    HunkPickerClose,
    Msg,
    MutationCompleted,
    MutationFailed,
    NewChange,
    OmnibarCancel,
    OmnibarSubmit,
    OpenBookmarkPicker,
    OpenBookmarkSet,
    OpenConflictView,
    OpenOmnibar,
    OpenOpLog,
    OpLogClose,
    OpLogLoadFailed,
    OpLogLoaded,
    OpLogRestore,
    RebaseCancel,
    RebaseConfirm,
    RebaseStart,
    Redo,
    ReloadRequested,
    Split,
    SplitConfirm,
    Squash,
    SquashPartial,
    SquashPartialConfirm,
    Undo,
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
        # A reload while a mutation is pending would race the mutation's own
        # follow-up load: its older graph could land first (matching the bumped
        # epoch) and the mutation's fresh graph would then be discarded as
        # stale. The mutation's follow-up reload brings the correct graph, so
        # drop the user's refresh until the gate reopens.
        if model.pending_mutation:
            return model, []
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

    # --- undo / redo ------------------------------------------------------
    if isinstance(msg, Undo):
        return _start_mutation(model, "undo", ())
    if isinstance(msg, Redo):
        return _start_mutation(model, "redo", ())

    # --- omnibar ----------------------------------------------------------
    if isinstance(msg, OpenOmnibar):
        return replace(model, modal="omnibar"), []
    if isinstance(msg, OmnibarCancel):
        return replace(model, modal=None), []
    if isinstance(msg, OmnibarSubmit):
        revset = msg.revset
        if revset is not None and revset == "":
            # empty query = no-op, just close
            return replace(model, modal=None), []
        if model.pending_mutation:
            # Don't race the mutation's follow-up reload (same guard as
            # ReloadRequested).  Record the revset + close the modal; it takes
            # effect on the next load triggered by MutationCompleted.
            return replace(model, modal=None, revset=revset), []
        epoch = model.graph_epoch + 1
        return replace(model, modal=None, revset=revset, graph_epoch=epoch), [
            LoadGraph(epoch, revset)
        ]

    # OmnibarInput / OmnibarBackspace / OmnibarAcceptCompletion are handled
    # widget-locally (query/cursor/completions are ephemeral); only submit /
    # cancel reach core.

    # --- bookmarks --------------------------------------------------------
    # BookmarkMove is handled widget-locally (the picker flips into destination-pick mode and later dispatches BookmarkMoveConfirm); no core branch.
    if isinstance(msg, OpenBookmarkSet):
        return replace(model, modal="bookmark_input"), []
    if isinstance(msg, OpenBookmarkPicker):
        return replace(model, modal="bookmark_picker"), [LoadBookmarks()]
    if isinstance(msg, BookmarkInputConfirm):
        target = selected_change_id(model)
        if target is None:
            return replace(model, modal=None, error="No change selected"), []
        return _start_mutation(replace(model, modal=None), "bookmark_set", (target, msg.name))
    if isinstance(msg, BookmarkInputCancel):
        return replace(model, modal=None), []
    if isinstance(msg, BookmarkDelete):
        return _start_mutation(model, "bookmark_delete", (msg.name,))
    if isinstance(msg, BookmarkMoveConfirm):
        return _start_mutation(model, "bookmark_move", (msg.name, msg.dest_change_id))
    if isinstance(msg, BookmarksLoaded):
        return replace(model, bookmarks=msg.bookmarks), []
    if isinstance(msg, BookmarksLoadFailed):
        return replace(model, error=msg.error), []

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

    # --- operation log ----------------------------------------------------
    if isinstance(msg, OpenOpLog):
        return replace(model, modal="op_log"), [LoadOpLog()]
    if isinstance(msg, OpLogClose):
        return replace(model, modal=None), []
    if isinstance(msg, OpLogRestore):
        return _start_mutation(replace(model, modal=None), "op_restore", (msg.op_id,))
    if isinstance(msg, OpLogLoaded):
        return replace(model, op_log_entries=msg.entries), []
    if isinstance(msg, OpLogLoadFailed):
        return replace(model, error=msg.error), []

    # --- conflict view ----------------------------------------------------
    if isinstance(msg, OpenConflictView):
        return replace(model, modal="conflict_view", conflict_path=msg.path), [
            LoadConflictData(msg.path)
        ]
    if isinstance(msg, ConflictViewClose):
        return replace(model, modal=None, conflict_path=None, conflict_data=None), []
    if isinstance(msg, ApplyResolutions):
        return _start_mutation(
            replace(model, modal=None, conflict_path=None, conflict_data=None),
            "resolve",
            (msg.path, msg.resolutions),
        )
    if isinstance(msg, ConflictDataLoaded):
        return replace(model, conflict_data=msg.data), []
    if isinstance(msg, ConflictDataLoadFailed):
        return replace(model, error=msg.error), []

    # --- hunk picker (split / partial squash) ----------------------------
    if isinstance(msg, Split):
        return replace(model, modal="hunk_picker"), []
    if isinstance(msg, SquashPartial):
        return replace(model, modal="hunk_picker"), []
    if isinstance(msg, HunkPickerClose):
        return replace(model, modal=None), []
    if isinstance(msg, SplitConfirm):
        return _start_mutation(replace(model, modal=None), "split", (msg.source, msg.hunks))
    if isinstance(msg, SquashPartialConfirm):
        return _start_mutation(
            replace(model, modal=None), "squash_partial", (msg.source, msg.hunks)
        )

    return model, []


def _start_mutation(
    model: Model, kind: str, args: tuple[Any, ...] | None
) -> tuple[Model, list[Cmd]]:
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
