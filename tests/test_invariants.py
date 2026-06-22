import pytest

from lajjzy.invariants import InvariantError, invariant


def test_invariant_passes_silently_when_true():
    assert invariant(True, "should not raise") is None


def test_invariant_raises_when_false():
    with pytest.raises(InvariantError, match="boom"):
        invariant(False, "boom")
