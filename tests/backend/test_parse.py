import pytest

from lajjzy.backend.parse import (
    RECORD_SEP,
    UNIT_SEP,
    parse_file_diffs,
    parse_file_line,
    parse_graph_output,
)
from lajjzy.backend.types import FileChange, FileStatus


def test_value_types_are_frozen():
    fc = FileChange(path="x", status=FileStatus.MODIFIED)
    with pytest.raises(Exception):  # FrozenInstanceError
        fc.path = "y"


def _node(display: str, fields: list[str]) -> str:
    return display + UNIT_SEP + RECORD_SEP.join(fields)


def test_parse_two_nodes_with_working_copy_and_files():
    fields_a = ["abc", "commitA", "Alice", "a@x", "1h", "first", "main", "false", "false", "@", ""]
    fields_b = ["def", "commitB", "Bob", "b@x", "2h", "second", "", "false", "false", "", "abc"]
    output = (
        "\n".join(
            [
                _node("◉ abc Alice 1h", fields_a),
                "M a.txt",
                "│",
                _node("◉ def Bob 2h", fields_b),
                "A b.txt",
            ]
        )
        + "\n"
    )

    g = parse_graph_output(output, op_id="op1")

    assert g.op_id == "op1"
    assert g.working_copy_index == 0
    assert g.node_indices == [0, 2]  # file lines attach to change; connector at index 1; node at 2
    assert g.lines[0].change_id == "abc"
    assert g.lines[0].glyph_prefix == "◉ "
    assert g.details["abc"].author == "Alice"
    assert g.details["abc"].bookmarks == ["main"]
    assert g.details["abc"].files[0].status == FileStatus.MODIFIED
    assert g.details["def"].parents == ["abc"]


def test_parse_trailing_newline_no_phantom_line():
    """Regression test: trailing newline should not create a phantom GraphLine.

    jj output always ends with a newline. When split("\n"), this creates
    a final empty-string element. This test ensures that empty line is skipped,
    not appended as a phantom GraphLine(raw="", change_id=None, glyph_prefix="").
    """
    fields = ["abc", "commitA", "Alice", "a@x", "1h", "first", "", "false", "false", "@", ""]
    output = _node("◉ abc Alice 1h", fields) + "\n"

    g = parse_graph_output(output, op_id="op1")

    # Verify no phantom line: every GraphLine has non-empty raw
    assert all(line.raw for line in g.lines), f"Found phantom line with empty raw: {g.lines}"

    # Should have exactly one line: the node itself
    assert len(g.lines) == 1, (
        f"Expected 1 line, got {len(g.lines)}: {[line.raw for line in g.lines]}"
    )

    assert g.lines[0].change_id == "abc"
    assert g.lines[0].raw == "◉ abc Alice 1h"


def test_parse_git_diff_one_file_one_hunk():
    diff = (
        "diff --git a/a.txt b/a.txt\n"
        "index 111..222 100644\n"
        "--- a/a.txt\n"
        "+++ b/a.txt\n"
        "@@ -1,2 +1,2 @@\n"
        " context\n"
        "-old\n"
        "+new\n"
    )
    files = parse_file_diffs(diff)
    assert len(files) == 1
    assert files[0].path == "a.txt"
    assert len(files[0].hunks) == 1
    kinds = [ln.kind for ln in files[0].hunks[0].lines]
    assert kinds == ["context", "remove", "add"]


def test_parse_file_line_strips_graph_prefix():
    fc = parse_file_line("│  M a.txt")
    assert fc is not None
    assert fc.status.value == "M"
    assert fc.path == "a.txt"


def test_parse_file_line_connector_only_returns_none():
    assert parse_file_line("│") is None
    assert parse_file_line("~") is None
    assert parse_file_line("(elided revisions)") is None


def test_parse_file_line_plain_no_prefix_still_works():
    fc = parse_file_line("M a.txt")
    assert fc is not None and fc.path == "a.txt"
