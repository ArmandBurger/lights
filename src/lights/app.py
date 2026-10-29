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
(74 -> 75 -> 80). Click a screen or use the arrow keys to select it.

The lock syncs every monitor to the selected one's brightness; while locked,
every step (mouse or keyboard) applies the same value to all monitors.

Keys:
  left/right, up/down : select the previous/next monitor (wraps)
  j              : raise the selected monitor's brightness by 10
  k              : lower the selected monitor's brightness by 10
  shift+j        : raise by 15
  shift+k        : lower by 15
  l              : toggle the lock
  w / c          : show the warranty / the licence conditions (GPL v3)
  r              : re-detect monitors and refresh now
  q              : quit

Every ddcutil call takes a few hundred milliseconds, so none of them run on
the UI thread. Input updates the shown value immediately and queues the
new target; the target is only written once input on that monitor has been
quiet for ``DEBOUNCE`` seconds, so a burst of clicks becomes a single write.
One writer per monitor applies the latest target. All app state is touched on
the UI thread only; worker threads do I/O and hand results back through
``call_from_thread``.
"""
from __future__ import annotations

from collections.abc import Callable
import asyncio
from concurrent.futures import ThreadPoolExecutor
from typing import ClassVar, Optional

from textual import on
from textual.app import App, ComposeResult
from textual.binding import Binding, BindingType
from textual.containers import Horizontal
from textual.timer import Timer
from textual.widgets import Button, Footer, Header, Static

from . import brightness, legal
from .monitors import Display, detect
from .widgets import LegalScreen, MonitorScreen

STEP = 10
STEP_BIG = 15
CLICK_STEP = 5
REFRESH = 1.5
DEBOUNCE = 0.5  # seconds of quiet input before a target is written
Readings = dict[int, Optional[int]]  # bus -> brightness (None: unreadable)


def _safe_get(bus: int) -> Optional[int]:
    try:
        return brightness.get_value(bus)
    except brightness.DdcError:
        return None


def _read_all(buses: list[int]) -> Readings:
    """Read every bus in parallel; each monitor has its own bus."""
    if not buses:
        return {}
    with ThreadPoolExecutor(max_workers=len(buses)) as pool:
        return dict(zip(buses, pool.map(_safe_get, buses)))


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
    """

    BINDINGS: ClassVar[list[BindingType]] = [
        Binding("left,up", "select(-1)", "Prev", show=False),
        Binding("right,down", "select(1)", "Next", show=False),
        Binding("j", f"bump({STEP})", f"+{STEP}"),
        Binding("k", f"bump(-{STEP})", f"-{STEP}"),
        Binding("J,shift+j", f"bump({STEP_BIG})", f"+{STEP_BIG}"),
        Binding("K,shift+k", f"bump(-{STEP_BIG})", f"-{STEP_BIG}"),
        Binding("l", "toggle_lock", "Lock"),
        Binding("r", "reload", "Refresh"),
        Binding("w", "show_warranty", "Warranty", show=False),
        Binding("c", "show_conditions", "License", show=False),
        Binding("q,enter", "quit", "Quit"),
    ]

    def __init__(self) -> None:
        super().__init__()
        self.selected = 0
        self.locked = False
        self.displays: list[Display] = []
        self.values: Readings = {}
        self.detected = False
        # Targets waiting to be written, and buses whose writer is running.
        self._pending: dict[int, int] = {}
        self._writing: set[int] = set()
        # Per-bus debounce timers; a bus with a running timer is not written.
        self._debounce: dict[int, Timer] = {}
        self._polling = False
        # Bumped per completed write so reads that started earlier are dropped.
        self._write_gen: dict[int, int] = {}

    def compose(self) -> ComposeResult:
        yield Header(icon="◉")
        with Horizontal(id="toolbar"):
            yield Button(id="lock")
        yield Static("detecting monitors…", id="status")
        yield Horizontal(id="screens")
        yield Static(legal.notice("press w", "press c"), id="notice", markup=False)
        yield Footer()

    def on_mount(self) -> None:
        self.title = "lights"
        self._redraw()
        self.action_reload()
        self.set_interval(REFRESH, self._poll)

    def action_show_warranty(self) -> None:
        self.push_screen(LegalScreen("Warranty", legal.WARRANTY))

    def action_show_conditions(self) -> None:
        self.push_screen(LegalScreen("GNU General Public License v3", legal.license_text()))

    # -- drawing ---------------------------------------------------------

    def _shown(self, bus: int) -> Optional[int]:
        """Value to display: the pending target if any, else the last read."""
        return self._pending.get(bus, self.values.get(bus))

    def _redraw(self) -> None:
        lock = self.query_one("#lock", Button)
        lock.label = "Locked · synced" if self.locked else "Unlocked"
        lock.variant = "primary" if self.locked else "default"

        status = self.query_one("#status", Static)
        status.display = not self.displays
        if not self.detected:
            status.update("detecting monitors…")
        elif not self.displays:
            status.update("no monitors responding  (r to retry)")

        for pos, screen in enumerate(self.query(MonitorScreen)):
            bus = screen.bus
            screen.show(
                self._shown(bus),
                writing=bus in self._pending or bus in self._writing,
                selected=self.locked or pos == self.selected,
            )

    @staticmethod
    def _bus(d: Display) -> int:
        assert d.bus is not None  # only addressable displays are kept
        return d.bus

    def _buses(self) -> list[int]:
        return [self._bus(d) for d in self.displays]

    # -- detection and polling -------------------------------------------

    def action_reload(self) -> None:
        self.run_worker(self._load, thread=True, group="load", exclusive=True)

    def _load(self) -> None:
        displays = [d for d in detect() if d.addressable]
        values = _read_all([self._bus(d) for d in displays])
        self.call_from_thread(self._loaded, displays, values)

    async def _loaded(self, displays: list[Display], values: Readings) -> None:
        def key(ds: list[Display]) -> list[tuple[Optional[int], str]]:
            return [(d.bus, d.model) for d in ds]

        changed = key(displays) != key(self.displays)
        self.displays = displays
        self.detected = True
        if self.selected >= len(displays):
            self.selected = 0
        if changed:
            screens = self.query_one("#screens", Horizontal)
            await screens.remove_children()
            await screens.mount_all(
                MonitorScreen(d, self._bus(d), CLICK_STEP) for d in displays
            )
        self._apply_reads(values)

    def _poll(self) -> None:
        if self._polling or not self.displays:
            return
        # Skip buses with a write in flight: the bus is busy and the read
        # would race the write.
        buses = [
            b for b in self._buses()
            if b not in self._writing and b not in self._pending
        ]
        if not buses:
            return
        self._polling = True
        gens = {b: self._write_gen.get(b, 0) for b in buses}
        self.run_worker(lambda: self._poll_io(gens), thread=True, group="poll")

    def _poll_io(self, gens: dict[int, int]) -> None:
        values = _read_all(list(gens))
        self.call_from_thread(self._polled, values, gens)

    def _polled(self, values: Readings, gens: dict[int, int]) -> None:
        self._polling = False
        # A write that finished while reading makes that reading stale.
        fresh = {
            b: v for b, v in values.items() if self._write_gen.get(b, 0) == gens[b]
        }
        self._apply_reads(fresh)

    def _apply_reads(self, values: Readings) -> None:
        for bus, val in values.items():
            if bus not in self._pending and bus not in self._writing:
                self.values[bus] = val
        self._redraw()

    # -- actions ---------------------------------------------------------

    def action_select(self, delta: int) -> None:
        if not self.displays:
            return
        self.selected = (self.selected + delta) % len(self.displays)
        self._redraw()

    def action_bump(self, delta: int) -> None:
        if self.displays:
            bus = self._bus(self.displays[self.selected])
            self._step(bus, lambda v: v + delta)

    @on(MonitorScreen.Step)
    def _on_screen_step(self, message: MonitorScreen.Step) -> None:
        self._select_bus(message.bus)
        delta = message.delta
        self._step(message.bus, lambda v: brightness.snap_step(v, delta))

    @on(MonitorScreen.Selected)
    def _on_screen_selected(self, message: MonitorScreen.Selected) -> None:
        self._select_bus(message.bus)
        self._redraw()

    def _select_bus(self, bus: int) -> None:
        self.selected = self._buses().index(bus)

    def _step(self, bus: int, compute: Callable[[int], int]) -> None:
        """Move ``bus`` (or every monitor, when locked) to ``compute(value)``."""
        base = self._shown(bus)
        if base is None:
            label = self.displays[self._buses().index(bus)].label
            self.notify(f"display {label}: brightness unknown", severity="error")
            return
        target = brightness.clamp(compute(base))
        for b in self._buses() if self.locked else [bus]:
            self._set_target(b, target)
        self._redraw()

    @on(Button.Pressed, "#lock")
    def action_toggle_lock(self) -> None:
        if self.locked:
            self.locked = False
        elif self.displays:
            bus = self._bus(self.displays[self.selected])
            base = self._shown(bus)
            if base is None:
                self.notify("cannot lock: brightness unknown", severity="error")
                return
            self.locked = True
            for b in self._buses():
                if self._shown(b) != base:
                    self._set_target(b, base)
            self.notify(f"locked: all monitors at {base}%")
        self._redraw()

    def _set_target(self, bus: int, value: int) -> None:
        """Queue ``value`` for ``bus``; it is written after ``DEBOUNCE``."""
        self._pending[bus] = value
        # Restart the countdown from now. (Timer.reset() would only restart
        # once the current interval has run out, stretching the delay.)
        timer = self._debounce.get(bus)
        if timer is not None:
            timer.stop()
        self._debounce[bus] = self.set_timer(
            DEBOUNCE, lambda: self._debounce_elapsed(bus)
        )

    def _debounce_elapsed(self, bus: int) -> None:
        """Debounce elapsed: start the writer unless one is already running."""
        self._debounce.pop(bus, None)
        # Input that netted out (e.g. + then -) needs no write.
        if bus not in self._writing and self._pending.get(bus) == self.values.get(bus):
            del self._pending[bus]
            self._redraw()
            return
        if bus in self._pending and bus not in self._writing:
            self._writing.add(bus)
            self.run_worker(
                lambda: self._write_loop(bus), thread=True, group=f"write-{bus}"
            )

    async def action_quit(self) -> None:
        """Write any still-debouncing targets before exiting."""
        for timer in self._debounce.values():
            timer.stop()
        self._debounce.clear()
        pending = dict(self._pending)
        if pending:
            def write_all() -> None:
                for bus, value in pending.items():
                    try:
                        brightness.set_value(bus, value)
                    except brightness.DdcError:
                        pass

            await asyncio.to_thread(write_all)
        self.exit()

    # -- writer (worker thread) ------------------------------------------

    def _next_target(self, bus: int) -> Optional[int]:
        target = self._pending.get(bus)
        # New input arrived during the last write: let its debounce finish;
        # _debounce_elapsed starts a fresh writer.
        if target is None or bus in self._debounce:
            self._writing.discard(bus)
        return target

    def _write_loop(self, bus: int) -> None:
        while (target := self.call_from_thread(self._next_target, bus)) is not None:
            try:
                brightness.set_value(bus, target)
            except brightness.DdcError:
                self.call_from_thread(self._write_failed, bus)
                return
            self.call_from_thread(self._written, bus, target)

    def _written(self, bus: int, value: int) -> None:
        self.values[bus] = value
        self._write_gen[bus] = self._write_gen.get(bus, 0) + 1
        # Drop the target only if no newer one arrived during the write.
        if self._pending.get(bus) == value:
            del self._pending[bus]
        self._redraw()

    def _write_failed(self, bus: int) -> None:
        self._pending.pop(bus, None)
        self._writing.discard(bus)
        label = next((d.label for d in self.displays if d.bus == bus), "?")
        self.notify(f"display {label}: cannot adjust brightness", severity="error")
        self._redraw()


def main() -> None:
    LightsApp().run()
