from lajjzy.backend.parse import RECORD_SEP, UNIT_SEP, parse_graph_output
from lajjzy.backend.types import FileStatus


def _node(display: str, fields: list[str]) -> str:
    return display + UNIT_SEP + RECORD_SEP.join(fields)


def test_parse_two_nodes_with_working_copy_and_files():
    fields_a = ["abc", "commitA", "Alice", "a@x", "1h", "first", "main",
                "false", "false", "@", ""]
    fields_b = ["def", "commitB", "Bob", "b@x", "2h", "second", "",
                "false", "false", "", "abc"]
    output = "\n".join([
        _node("◉ abc Alice 1h", fields_a),
        "M a.txt",
        "│",
        _node("◉ def Bob 2h", fields_b),
        "A b.txt",
    ]) + "\n"

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
