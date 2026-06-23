from __future__ import annotations

from lajjzy.backend.types import (
    Bookmark,
    CompletionItem,
    ConflictData,
    ConflictRegion,
    HunkResolution,
    OpLogEntry,
)


def test_op_log_entry_fields():
    e = OpLogEntry(op_id="abc", timestamp="1h ago", description="commit")
    assert e.op_id == "abc"
    assert e.timestamp == "1h ago"
    assert e.description == "commit"


def test_bookmark_fields():
    b = Bookmark(name="main", change_id="ksqxwpml", change_description="head")
    assert b.name == "main"
    assert b.change_id == "ksqxwpml"
    assert b.change_description == "head"


def test_conflict_region_resolved():
    r = ConflictRegion.resolved("context line")
    assert r.kind == "resolved"
    assert r.text == "context line"


def test_conflict_region_conflict():
    r = ConflictRegion.conflict(base="b", left="l", right="r")
    assert r.kind == "conflict"
    assert r.base == "b"
    assert r.left == "l"
    assert r.right == "r"


def test_conflict_data():
    c = ConflictData(regions=[ConflictRegion.resolved("x")])
    assert len(c.regions) == 1


def test_hunk_resolution_values():
    assert HunkResolution.NONE is not None
    assert HunkResolution.ACCEPT_LEFT is not None
    assert HunkResolution.ACCEPT_RIGHT is not None
    assert HunkResolution.NONE is not HunkResolution.ACCEPT_LEFT


def test_completion_item_fields():
    c = CompletionItem(insert_text="all(", display_text="all() — all visible changes")
    assert c.insert_text == "all("
    assert c.display_text.startswith("all()")
