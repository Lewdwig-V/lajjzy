from hypothesis import given, strategies as st

from lajjzy.backend.types import ChangeDetail, GraphData, GraphLine

_ids = st.text("abcdef0123456789", min_size=1, max_size=4)


def _detail():
    return ChangeDetail(
        commit_id="c",
        author="a",
        email="e",
        timestamp="1h",
        description="d",
        bookmarks=[],
        is_empty=False,
        has_conflict=False,
        files=[],
        parents=[],
    )


@st.composite
def consistent_graphs(draw):
    """Build a GraphData that satisfies the I2 contract by construction."""
    ids = draw(st.lists(_ids, unique=True, max_size=6))
    lines = []
    for cid in ids:
        lines.append(GraphLine(raw=f"◉ {cid}", change_id=cid, glyph_prefix="◉ "))
        if draw(st.booleans()):
            lines.append(GraphLine(raw="│", change_id=None, glyph_prefix="│"))
    details = {cid: _detail() for cid in ids}
    node_positions = [i for i, ln in enumerate(lines) if ln.change_id is not None]
    wci = draw(st.sampled_from(node_positions)) if node_positions else None
    return GraphData(lines=lines, details=details, working_copy_index=wci, op_id="op")


@given(consistent_graphs())
def test_node_indices_match_lines(g):
    # I2: node_indices is exactly the set of lines carrying a change_id.
    assert g.node_indices == [i for i, ln in enumerate(g.lines) if ln.change_id is not None]
    for i in g.node_indices:
        assert g.lines[i].change_id is not None
    if g.working_copy_index is not None:
        assert g.working_copy_index in g.node_indices


@given(
    consistent_graphs(),
    st.lists(st.sampled_from([-1, 1, "top", "bottom"]), max_size=20),
)
def test_cursor_stays_on_node(g, moves):
    # I3: starting on a node and applying any nav sequence, cursor stays on a node.
    if not g.node_indices:
        return
    cursor = g.working_copy_index if g.working_copy_index is not None else g.node_indices[0]
    nodes = g.node_indices
    for mv in moves:
        if mv == "top":
            cursor = nodes[0]
        elif mv == "bottom":
            cursor = nodes[-1]
        else:
            pos = nodes.index(cursor) if cursor in nodes else 0
            cursor = nodes[max(0, min(len(nodes) - 1, pos + mv))]
        assert cursor in nodes
