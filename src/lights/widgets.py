"""Clickable monitor widgets for the TUI.

Each monitor is drawn as a "screen": a rounded box split by a vertical
divider, a large minus on the left half and a large plus on the right half,
with the display's name and info underneath. Clicking a half posts
``MonitorScreen.Step``; clicking anywhere else on the widget posts
``MonitorScreen.Selected``.
"""
from __future__ import annotations

from typing import Optional

from textual import events
from textual.app import ComposeResult
from textual.containers import Horizontal
from textual.message import Message
from textual.widget import Widget
from textual.widgets import Static

from .monitors import Display

# Terminal cells are about twice as tall as wide, so glyphs are drawn wide.
_PLUS = "\n".join(
    [
        "    ██    ",
        "    ██    ",
        "██████████",
        "    ██    ",
        "    ██    ",
    ]
)
_MINUS = "\n".join(
    [
        "          ",
        "          ",
        "██████████",
        "          ",
        "          ",
    ]
)


class StepZone(Static):
    """One clickable half of a screen."""

    def __init__(self, bus: int, delta: int) -> None:
        super().__init__(_PLUS if delta > 0 else _MINUS)
        self.bus = bus
        self.delta = delta
        self.add_class("plus" if delta > 0 else "minus")

    def on_click(self, event: events.Click) -> None:
        event.stop()
        self.post_message(MonitorScreen.Step(self.bus, self.delta))


class MonitorScreen(Widget):
    """A monitor drawn as a screen with minus/plus halves and a caption."""

    DEFAULT_CSS = """
    MonitorScreen {
        width: 1fr;
        height: 1fr;
        margin: 1 2;
    }
    MonitorScreen > .panel {
        height: 1fr;
        border: round $foreground 60%;
    }
    MonitorScreen.-selected > .panel {
        border: round $accent;
    }
    MonitorScreen StepZone {
        width: 1fr;
        height: 1fr;
        content-align: center middle;
    }
    MonitorScreen StepZone:hover {
        background: $boost;
        color: $accent;
    }
    MonitorScreen .divider {
        width: 1;
        height: 1fr;
        margin: 2 0;
        border-left: solid $foreground 60%;
    }
    MonitorScreen > .caption {
        width: 1fr;
        height: 3;
        content-align: center top;
        text-align: center;
    }
    """

    class Step(Message):
        """A half of a screen was clicked: move brightness by ``delta``."""

        def __init__(self, bus: int, delta: int) -> None:
            super().__init__()
            self.bus = bus
            self.delta = delta

    class Selected(Message):
        def __init__(self, bus: int) -> None:
            super().__init__()
            self.bus = bus

    def __init__(self, monitor: Display, bus: int, step: int) -> None:
        super().__init__()
        self.monitor = monitor
        self.bus = bus
        self.step = step

    def compose(self) -> ComposeResult:
        with Horizontal(classes="panel"):
            yield StepZone(self.bus, -self.step)
            yield Static(classes="divider")
            yield StepZone(self.bus, self.step)
        yield Static(classes="caption")

    def on_click(self, event: events.Click) -> None:
        self.post_message(self.Selected(self.bus))

    def show(self, value: Optional[int], writing: bool, selected: bool) -> None:
        d = self.monitor
        pct = f"{value}%" if value is not None else "n/a"
        busy = " …" if writing else ""
        bus_name = d.i2c_bus.removeprefix("/dev/")
        self.query_one(".caption", Static).update(
            f"[b]{d.model or 'unknown'}[/b]  [b $accent]{pct}{busy}[/]\n"
            f"[dim]display {d.label} · {d.connector} · {bus_name}[/dim]"
        )
        self.set_class(selected, "-selected")
