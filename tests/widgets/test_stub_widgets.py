from __future__ import annotations

import pytest

from lajjzy.widgets import (
    BookmarkInput,
    BookmarkPicker,
    ConflictView,
    HunkPicker,
    Omnibar,
    OpLog,
)


@pytest.mark.parametrize(
    "widget_cls",
    [Omnibar, BookmarkInput, BookmarkPicker, OpLog, ConflictView, HunkPicker],
)
def test_stub_widget_importable(widget_cls):
    # Each stub must at least be importable and constructible.
    w = widget_cls()
    assert w is not None
