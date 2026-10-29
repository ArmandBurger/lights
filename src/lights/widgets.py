# lights - brightness controller for monitors via ddcutil
# Copyright (C) 2026  Armand Burger
#
# This program is free software: you can redistribute it and/or modify
# it under the terms of the GNU General Public License as published by
# the Free Software Foundation, either version 3 of the License, or
# (at your option) any later version.
#
# This program is distributed in the hope that it will be useful,
# but WITHOUT ANY WARRANTY; without even the implied warranty of
# MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
# GNU General Public License for more details.
#
# You should have received a copy of the GNU General Public License
# along with this program.  If not, see <https://www.gnu.org/licenses/>.
"""Clickable monitor widgets for the TUI.

Each monitor is drawn as a "screen": a rounded box split by a vertical
divider, a large minus on the left half and a large plus on the right half,
with the display's name and info underneath. Clicking a half posts
``MonitorScreen.Step``; clicking anywhere else on the widget posts
``MonitorScreen.Selected``.
"""
from __future__ import annotations

from typing import ClassVar, Optional

from textual import events
from textual.app import ComposeResult
from textual.binding import Binding, BindingType
from textual.containers import Horizontal, VerticalScroll
from textual.message import Message
from textual.screen import ModalScreen
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


class LegalScreen(ModalScreen[None]):
    """Scrollable view of the warranty or the licence; Esc/q closes it."""

    DEFAULT_CSS = """
    LegalScreen {
        align: center middle;
    }
    LegalScreen > VerticalScroll {
        width: 84;
        max-width: 100%;
        height: 90%;
        border: round $accent;
        background: $surface;
        padding: 0 1;
    }
    """

    BINDINGS: ClassVar[list[BindingType]] = [
        Binding("escape,q,w,c,n", "dismiss", "Close"),
    ]

    def __init__(self, title: str, text: str) -> None:
        super().__init__()
        self.title_text = title
        self.text = text

    def compose(self) -> ComposeResult:
        with VerticalScroll() as scroll:
            scroll.border_title = self.title_text
            scroll.border_subtitle = "Esc to close"
            yield Static(self.text, markup=False)
