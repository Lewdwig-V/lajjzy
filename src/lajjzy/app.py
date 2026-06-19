from textual.app import App, ComposeResult
from textual.widgets import Static


class LajjzyApp(App[None]):
    """Root application. Owns cross-cutting reactive state and key bindings."""

    CSS_PATH = "styles.tcss"

    def compose(self) -> ComposeResult:
        yield Static("lajjzy — reboot scaffold")


def main() -> None:
    LajjzyApp().run()
