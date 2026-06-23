from __future__ import annotations

from lajjzy.backend.parse import parse_op_log
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
    assert HunkResolution.NONE == "none"
    assert HunkResolution.ACCEPT_LEFT == "accept_left"
    assert HunkResolution.ACCEPT_RIGHT == "accept_right"
    assert HunkResolution.NONE != HunkResolution.ACCEPT_LEFT


def test_completion_item_fields():
    c = CompletionItem(insert_text="all(", display_text="all() — all visible changes")
    assert c.insert_text == "all("
    assert c.display_text.startswith("all()")


def test_parse_op_log_empty():
    assert parse_op_log("") == []


def test_parse_op_log_entries():
    # jj op log --no-graph -T produces one entry per line with our template;
    # fields are separated by \x1f, entries by \n.
    out = "abc123\x1f2 hours ago\x1fcommit xyz\ndef456\x1f1 hour ago\x1fabsorb"
    entries = parse_op_log(out)
    assert len(entries) == 2
    assert entries[0].op_id == "abc123"
    assert entries[0].timestamp == "2 hours ago"
    assert entries[0].description == "commit xyz"
    assert entries[1].op_id == "def456"
    assert entries[1].description == "absorb"


def test_parse_op_log_ignores_blank_trailing_line():
    out = "abc\x1fnow\x1fdesc\n"
    entries = parse_op_log(out)
    assert len(entries) == 1


def test_parse_op_log_skips_malformed_line():
    # The middle line has no field separators — it must be skipped silently.
    out = "abc\x1fnow\x1fdesc\nMALFORMED_NO_SEPARATORS\ndef\x1flater\x1fother\n"
    entries = parse_op_log(out)
    assert len(entries) == 2
    assert entries[0].op_id == "abc"
    assert entries[1].op_id == "def"


def test_parse_bookmarks_empty():
    from lajjzy.backend.parse import parse_bookmarks

    assert parse_bookmarks("") == []


def test_parse_bookmarks_entries():
    from lajjzy.backend.parse import parse_bookmarks

    # name \x1f change_id \x1f change_description \n
    out = "main\x1fksqxwpml\x1fhead commit\nfeature\x1fytoqrzxn\x1fwip"
    bms = parse_bookmarks(out)
    assert len(bms) == 2
    assert bms[0].name == "main"
    assert bms[0].change_id == "ksqxwpml"
    assert bms[0].change_description == "head commit"
    assert bms[1].name == "feature"


def test_parse_bookmarks_ignores_blank_trailing_line():
    from lajjzy.backend.parse import parse_bookmarks

    bms = parse_bookmarks("main\x1fabc\x1fdesc\n")
    assert len(bms) == 1


def test_parse_conflict_data_no_conflicts():
    from lajjzy.backend.parse import parse_conflict_data

    # A file with no conflict markers is one resolved region.
    cd = parse_conflict_data("line1\nline2\n")
    assert len(cd.regions) == 1
    assert cd.regions[0].kind == "resolved"
    assert cd.regions[0].text == "line1\nline2\n"


def test_parse_conflict_data_one_conflict():
    from lajjzy.backend.parse import parse_conflict_data

    out = "before\n<<<<<<<\nours\n|||||||\nbase\n=======\ntheirs\n>>>>>>>\nafter\n"
    cd = parse_conflict_data(out)
    assert len(cd.regions) == 3
    assert cd.regions[0].kind == "resolved"
    assert cd.regions[0].text == "before\n"
    assert cd.regions[1].kind == "conflict"
    assert cd.regions[1].left == "ours\n"
    assert cd.regions[1].base == "base\n"
    assert cd.regions[1].right == "theirs\n"
    assert cd.regions[2].kind == "resolved"
    assert cd.regions[2].text == "after\n"


def test_parse_conflict_data_empty_sides():
    from lajjzy.backend.parse import parse_conflict_data

    # Empty side = that side deleted the region.
    out = "<<<<<<<\n|||||||\nbase\n=======\ntheirs\n>>>>>>>\n"
    cd = parse_conflict_data(out)
    assert len(cd.regions) == 1
    assert cd.regions[0].kind == "conflict"
    assert cd.regions[0].left == ""
    assert cd.regions[0].base == "base\n"
    assert cd.regions[0].right == "theirs\n"
