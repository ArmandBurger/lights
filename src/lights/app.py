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
"""Interactive TUI for lights (Textual).

Each monitor is drawn as a screen: click its left half (minus) or right
half (plus) to step brightness by 5, snapping to a multiple of 5 first
(74 -> 75 -> 80). Click a screen to select it. Keys are vim-style (see
``keys.HELP``); w / c / n show the warranty, the licence and third-party
notices. Brightness state, debouncing and the lock live in ``Controller``.
"""
from __future__ import annotations

import asyncio
import threading
from collections.abc import Callable
from typing import ClassVar

from textual import events, on
from textual.app import App, ComposeResult
from textual.binding import Binding, BindingType
from textual.containers import Horizontal
from textual.widgets import Button, Header, Static

from . import legal
from .controller import Controller
from .keys import HELP, HELP_LINE, STEP, Action, VimKeys
from .widgets import LegalScreen, MonitorScreen


class LightsApp(App[None]):
    CSS = """
    #toolbar {
        height: 3;
        align: center middle;
    }
    #lock {
        width: 24;
    }
    #status {
        height: 1fr;
        content-align: center middle;
    }
    #screens {
        height: 1fr;
    }
    #notice {
        height: auto;
        padding: 0 2;
        color: $text-muted;
    }
    #keys {
        height: 1;
        padding: 0 2;
        background: $panel;
    }
    """

    BINDINGS: ClassVar[list[BindingType]] = [
        Binding("w", "show_warranty", "Warranty", show=False),
        Binding("c", "show_conditions", "License", show=False),
        Binding("n", "show_notices", "Notices", show=False),
    ]

    def __init__(self) -> None:
        super().__init__()
        self._ui_thread = threading.get_ident()
        self.controller = Controller(self._on_change, self._on_report)
        self.vim = VimKeys()
        self._layout = -1

    def compose(self) -> ComposeResult:
        yield Header(icon="◉")
        with Horizontal(id="toolbar"):
            yield Button(id="lock")
        yield Static("detecting monitors…", id="status")
        yield Horizontal(id="screens")
        yield Static(legal.notice("press w", "press c"), id="notice", markup=False)
        yield Static(HELP_LINE, id="keys", markup=False)

    def on_mount(self) -> None:
        self.title = "lights"
        self.controller.start()

    # -- controller callbacks (any thread) -------------------------------

    def _on_ui_thread(self, callback: Callable[[], object]) -> None:
        if threading.get_ident() == self._ui_thread:
            self.call_later(callback)
            return
        try:
            self.call_from_thread(callback)
        except RuntimeError:
            pass  # app already closed

    def _on_change(self) -> None:
        self._on_ui_thread(self._sync)

    def _on_report(self, message: str) -> None:
        self._on_ui_thread(lambda: self.notify(message))

    # -- drawing ---------------------------------------------------------

    async def _sync(self) -> None:
        view = self.controller.view()
        lock = self.query_one("#lock", Button)
        lock.label = "Locked · synced" if view.locked else "Unlocked"
        lock.variant = "primary" if view.locked else "default"

        status = self.query_one("#status", Static)
        status.display = not view.monitors
        if not view.detected:
            status.update("detecting monitors…")
        elif not view.monitors:
            status.update("no monitors responding  (r to retry)")

        if view.layout != self._layout:
            self._layout = view.layout
            screens = self.query_one("#screens", Horizontal)
            await screens.remove_children()
            await screens.mount_all(
                MonitorScreen(m.display, m.bus, STEP) for m in view.monitors
            )
        for screen, m in zip(self.query(MonitorScreen), view.monitors):
            screen.show(m.value, writing=m.busy, selected=m.selected)
        self._show_keys()

    def _show_keys(self) -> None:
        pending = self.vim.pending
        self.query_one("#keys", Static).update(
            f"{HELP_LINE}    {pending}" if pending else HELP_LINE
        )

    # -- input -----------------------------------------------------------

    async def on_key(self, event: events.Key) -> None:
        if isinstance(self.screen, LegalScreen):
            return  # the overlay handles its own keys
        name =event.character if event.is_printable and event.character else event.key
        action = self.vim.feed(name)
        if action is None:
            return
        event.stop()
        event.prevent_default()
        await self._run(action)
        self._show_keys()

    async def _run(self, action: Action) -> None:
        c = self.controller
        if action.kind == "select":
            c.select(action.amount)
        elif action.kind == "step":
            c.step(action.amount, action.size)
        elif action.kind == "set":
            c.set_level(action.amount)
        elif action.kind == "lock":
            c.toggle_lock()
        elif action.kind == "reload":
            c.reload()
        elif action.kind == "help":
            text = "\n".join(f"{keys:<18}{what}" for keys, what in HELP)
            self.push_screen(LegalScreen("Keys", text))
        elif action.kind == "quit":
            await self.action_quit()

    @on(MonitorScreen.Step)
    def _on_screen_step(self, message: MonitorScreen.Step) -> None:
        self.controller.select_bus(message.bus)
        steps = 1 if message.delta > 0 else -1
        self.controller.step(steps, abs(message.delta), message.bus)

    @on(MonitorScreen.Selected)
    def _on_screen_selected(self, message: MonitorScreen.Selected) -> None:
        self.controller.select_bus(message.bus)

    @on(Button.Pressed, "#lock")
    def _on_lock_pressed(self) -> None:
        self.controller.toggle_lock()

    async def action_quit(self) -> None:
        """Write any still-debouncing targets before exiting."""
        await asyncio.to_thread(self.controller.shutdown)
        self.exit()

    def action_show_warranty(self) -> None:
        self.push_screen(LegalScreen("Warranty", legal.WARRANTY))

    def action_show_conditions(self) -> None:
        self.push_screen(LegalScreen("GNU General Public License v3", legal.license_text()))

    def action_show_notices(self) -> None:
        self.push_screen(LegalScreen("Third-party notices", legal.notices_text()))


def main() -> None:
    LightsApp().run()
