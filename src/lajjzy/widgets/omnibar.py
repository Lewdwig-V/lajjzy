from __future__ import annotations

from textual.widgets import Static


class Omnibar(Static):
    """Omnibar overlay — revset search + completion. STUB: phase 1b mounts it;
    phase 2 (feature 2) fills in render + view-local state + key handling."""

    def render(self) -> str:
        return "(omnibar — phase 2)"
