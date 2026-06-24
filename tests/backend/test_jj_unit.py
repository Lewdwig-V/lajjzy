"""Pure unit tests for _build_resolved_content (no jj subprocess needed)."""

from __future__ import annotations

import pytest

from lajjzy.backend.jj import _build_resolved_content
from lajjzy.backend.types import ConflictData, ConflictRegion, HunkResolution, JjError


def test_accept_right_selects_right():
    """ACCEPT_RIGHT picks region.right."""
    data = ConflictData(
        regions=[ConflictRegion.conflict(base="b\n", left="left\n", right="right\n")]
    )
    result = _build_resolved_content(data, [HunkResolution.ACCEPT_RIGHT])
    assert result == "right\n"


def test_accept_left_selects_left():
    """ACCEPT_LEFT picks region.left."""
    data = ConflictData(
        regions=[ConflictRegion.conflict(base="b\n", left="left\n", right="right\n")]
    )
    result = _build_resolved_content(data, [HunkResolution.ACCEPT_LEFT])
    assert result == "left\n"


def test_none_defaults_to_left():
    """NONE falls back to left (defensive default — widget must not allow NONE at apply)."""
    data = ConflictData(
        regions=[ConflictRegion.conflict(base="b\n", left="left\n", right="right\n")]
    )
    result = _build_resolved_content(data, [HunkResolution.NONE])
    assert result == "left\n"


def test_mixed_resolutions_interleaved_with_resolved():
    """Mixed [ACCEPT_LEFT, ACCEPT_RIGHT] across two conflict regions with resolved text between them.
    Verifies conflict_idx advances only on conflict regions — an off-by-one corrupts files.
    """
    data = ConflictData(
        regions=[
            ConflictRegion.resolved("header\n"),
            ConflictRegion.conflict(base="b1\n", left="left1\n", right="right1\n"),
            ConflictRegion.resolved("middle\n"),
            ConflictRegion.conflict(base="b2\n", left="left2\n", right="right2\n"),
            ConflictRegion.resolved("footer\n"),
        ]
    )
    result = _build_resolved_content(
        data, [HunkResolution.ACCEPT_LEFT, HunkResolution.ACCEPT_RIGHT]
    )
    assert result == "header\nleft1\nmiddle\nright2\nfooter\n"


def test_too_few_resolutions_raises():
    """Fewer resolutions than conflict regions raises JjError naming the counts."""
    data = ConflictData(
        regions=[
            ConflictRegion.conflict(base="b1\n", left="l1\n", right="r1\n"),
            ConflictRegion.conflict(base="b2\n", left="l2\n", right="r2\n"),
        ]
    )
    with pytest.raises(JjError, match="2"):
        _build_resolved_content(data, [HunkResolution.ACCEPT_LEFT])


def test_zero_conflict_regions_no_resolutions():
    """Zero conflict regions with empty resolutions list: returns concatenated resolved text, no raise."""
    data = ConflictData(
        regions=[
            ConflictRegion.resolved("line1\n"),
            ConflictRegion.resolved("line2\n"),
        ]
    )
    result = _build_resolved_content(data, [])
    assert result == "line1\nline2\n"
