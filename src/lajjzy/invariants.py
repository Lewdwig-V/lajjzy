"""Hard-invariant assertions. A violation means lajjzy's model of reality is
broken — a programmer error, not a user/jj error — so it raises and (per the
crash policy) brings the app down via the top-level handler in app.main()."""

from __future__ import annotations


class InvariantError(Exception):
    """Raised when a hard internal invariant is violated."""


def invariant(condition: bool, message: str) -> None:
    """Assert a hard internal invariant. Explicit raise (survives `python -O`).

    Use for model/state breaches only. Data-shape problems use ValueError;
    user/jj failures use JjError.
    """
    if not condition:
        raise InvariantError(message)
