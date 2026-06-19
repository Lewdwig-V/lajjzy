from lajjzy.backend.types import (
    ChangeDetail, FileChange, FileStatus, GraphData, GraphLine,
)


def test_graphdata_node_indices_and_lookup():
    lines = [
        GraphLine(raw="◉ abc author 1h", change_id="abc", glyph_prefix="◉ "),
        GraphLine(raw="│", change_id=None, glyph_prefix="│"),
        GraphLine(raw="◉ def author 2h", change_id="def", glyph_prefix="◉ "),
    ]
    detail = ChangeDetail(
        commit_id="c1", author="a", email="e", timestamp="1h",
        description="d", bookmarks=[], is_empty=False, conflict_count=0,
        files=[FileChange(path="x.py", status=FileStatus.MODIFIED)], parents=[],
    )
    g = GraphData(lines=lines, details={"abc": detail, "def": detail},
                  working_copy_index=0, op_id="op1")

    assert g.node_indices == [0, 2]
    assert g.change_id_at(2) == "def"
    assert g.change_id_at(1) is None
