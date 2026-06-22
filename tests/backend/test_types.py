import pytest

from lajjzy.backend.types import (
    ChangeDetail,
    FileChange,
    FileStatus,
    GraphData,
    GraphLine,
)


def _make_detail() -> ChangeDetail:
    return ChangeDetail(
        commit_id="c1",
        author="a",
        email="e",
        timestamp="1h",
        description="d",
        bookmarks=[],
        is_empty=False,
        has_conflict=False,
        files=[FileChange(path="x.py", status=FileStatus.MODIFIED)],
        parents=[],
    )


def _make_lines() -> list[GraphLine]:
    return [
        GraphLine(raw="◉ abc author 1h", change_id="abc", glyph_prefix="◉ "),
        GraphLine(raw="│", change_id=None, glyph_prefix="│"),
        GraphLine(raw="◉ def author 2h", change_id="def", glyph_prefix="◉ "),
    ]


def test_graphdata_node_indices_and_lookup():
    lines = _make_lines()
    detail = _make_detail()
    g = GraphData(
        lines=lines, details={"abc": detail, "def": detail}, working_copy_index=0, op_id="op1"
    )

    assert g.node_indices == [0, 2]
    assert g.change_id_at(2) == "def"
    assert g.change_id_at(1) is None


def test_graphdata_derives_node_indices_ignoring_supplied():
    """node_indices is a cached_property derived from lines; no constructor arg accepted."""
    lines = _make_lines()
    detail = _make_detail()
    g = GraphData(
        lines=lines,
        details={"abc": detail, "def": detail},
        working_copy_index=0,
        op_id="op1",
    )
    assert g.node_indices == [0, 2], f"Expected [0, 2] (derived from lines), got {g.node_indices!r}"


def test_graphdata_rejects_out_of_range_working_copy_index():
    """working_copy_index out of bounds for lines must raise ValueError."""
    lines = _make_lines()
    detail = _make_detail()
    with pytest.raises(ValueError, match="working_copy_index 5 is not a valid node line"):
        GraphData(
            lines=lines, details={"abc": detail, "def": detail}, working_copy_index=5, op_id="op1"
        )


def test_graphdata_rejects_working_copy_index_on_connector_line():
    """working_copy_index pointing at a connector line (change_id=None) must raise ValueError."""
    lines = _make_lines()
    detail = _make_detail()
    # lines[1] is a connector with change_id=None
    with pytest.raises(ValueError, match="working_copy_index 1 is not a valid node line"):
        GraphData(
            lines=lines, details={"abc": detail, "def": detail}, working_copy_index=1, op_id="op1"
        )


# ---------------------------------------------------------------------------
# Task-3 tests: frozen + self-validating GraphData, node_indices as cached_property
# ---------------------------------------------------------------------------


def _detail(commit: str = "c") -> ChangeDetail:
    return ChangeDetail(
        commit_id=commit,
        author="a",
        email="e",
        timestamp="1h",
        description="d",
        bookmarks=[],
        is_empty=False,
        has_conflict=False,
        files=[FileChange(path="x", status=FileStatus.MODIFIED)],
        parents=[],
    )


def test_node_indices_is_derived_not_stored():
    lines = [
        GraphLine(raw="◉ abc", change_id="abc", glyph_prefix="◉ "),
        GraphLine(raw="│", change_id=None, glyph_prefix="│"),
        GraphLine(raw="◉ def", change_id="def", glyph_prefix="◉ "),
    ]
    g = GraphData(
        lines=lines,
        details={"abc": _detail(), "def": _detail()},
        working_copy_index=0,
        op_id="op1",
    )
    assert g.node_indices == [0, 2]


def test_graphdata_is_frozen():
    g = GraphData(lines=[], details={}, working_copy_index=None, op_id="op1")
    with pytest.raises(Exception):  # FrozenInstanceError
        g.op_id = "mutated"  # type: ignore[misc]


def test_rejects_out_of_range_working_copy_index():
    lines = [GraphLine(raw="◉ abc", change_id="abc", glyph_prefix="◉ ")]
    with pytest.raises(ValueError):
        GraphData(lines=lines, details={"abc": _detail()}, working_copy_index=5, op_id="op1")


def test_rejects_working_copy_index_on_connector():
    lines = [
        GraphLine(raw="◉ abc", change_id="abc", glyph_prefix="◉ "),
        GraphLine(raw="│", change_id=None, glyph_prefix="│"),
    ]
    with pytest.raises(ValueError):
        GraphData(lines=lines, details={"abc": _detail()}, working_copy_index=1, op_id="op1")


def test_rejects_referential_mismatch():
    lines = [GraphLine(raw="◉ abc", change_id="abc", glyph_prefix="◉ ")]
    with pytest.raises(ValueError):
        # 'abc' in lines but absent from details
        GraphData(lines=lines, details={}, working_copy_index=None, op_id="op1")
