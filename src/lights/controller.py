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
"""Brightness state shared by the TUI and the GUI.

Every ddcutil call takes a few hundred milliseconds, so all of them run on
worker threads. Input updates the shown value immediately and queues the new
target; the target is only written once input on that monitor has been quiet
for ``DEBOUNCE`` seconds, so a burst of input becomes a single write. One
writer thread per monitor applies the latest target. A polling thread reads
values every ``REFRESH`` seconds, skipping monitors with a write in flight.

The lock syncs every monitor to the selected one's brightness; while locked,
every change applies the same value to all monitors.

State is guarded by one lock. ``notify`` (state changed) and ``report``
(error message) may be called from any thread and are never called while
holding the lock; the UI marshals them onto its own thread.
"""
from __future__ import annotations

import math
import threading
from collections.abc import Callable
from concurrent.futures import ThreadPoolExecutor
from dataclasses import dataclass
from typing import Optional

from . import brightness
from .monitors import Display, detect

REFRESH = 1.5
DEBOUNCE = 0.5  # seconds of quiet input before a target is written
MAX_COLUMNS = 4
MAX_MONITORS = MAX_COLUMNS * MAX_COLUMNS  # shown as at most a 4x4 grid

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


def grid_shape(count: int) -> tuple[int, int]:
    """(columns, rows) of the most square grid holding ``count`` monitors.

    1 -> 1x1, 2 -> 2x1, 3-4 -> 2x2, 5-6 -> 3x2, 7-9 -> 3x3, 10-12 -> 4x3,
    13-16 -> 4x4.
    """
    count = max(1, min(count, MAX_MONITORS))
    columns = math.ceil(math.sqrt(count))
    return columns, math.ceil(count / columns)


def _bus(d: Display) -> int:
    assert d.bus is not None  # only addressable displays are kept
    return d.bus


def _spawn(target: Callable[[], None]) -> None:
    threading.Thread(target=target, daemon=True).start()


@dataclass(frozen=True)
class MonitorView:
    """What a UI needs to draw one monitor."""

    display: Display
    bus: int
    value: Optional[int]  # pending target if any, else the last read
    busy: bool  # a change is waiting to be written or being written
    selected: bool


@dataclass(frozen=True)
class View:
    detected: bool
    locked: bool
    layout: int  # changes whenever the set of monitors changes
    monitors: list[MonitorView]


class Controller:
    def __init__(
        self, notify: Callable[[], None], report: Callable[[str], None]
    ) -> None:
        self._notify = notify
        self._report = report
        self._lock = threading.RLock()
        self._stop = threading.Event()
        self._displays: list[Display] = []
        self._detected = False
        self._layout = 0
        self._selected = 0
        self._locked = False
        self._values: Readings = {}
        # Targets waiting to be written, and buses whose writer is running.
        self._pending: dict[int, int] = {}
        self._writing: set[int] = set()
        # Per-bus debounce timers. A timer's token must still match when it
        # fires, so a timer replaced just as it fired does nothing.
        self._timers: dict[int, threading.Timer] = {}
        self._tokens: dict[int, int] = {}
        # Bumped per completed write so reads that started earlier are dropped.
        self._write_gen: dict[int, int] = {}

    # -- lifecycle -------------------------------------------------------

    def start(self) -> None:
        self.reload()
        _spawn(self._poll_loop)

    def reload(self) -> None:
        """Re-detect monitors and read their brightness."""
        _spawn(self._load)

    def shutdown(self) -> None:
        """Stop polling and write any still-debouncing targets (blocks)."""
        self._stop.set()
        with self._lock:
            for timer in self._timers.values():
                timer.cancel()
            self._timers.clear()
            pending = dict(self._pending)
        for bus, value in pending.items():
            try:
                brightness.set_value(bus, value)
            except brightness.DdcError:
                pass

    # -- reading state ---------------------------------------------------

    def _shown(self, bus: int) -> Optional[int]:
        return self._pending.get(bus, self._values.get(bus))

    def view(self) -> View:
        with self._lock:
            monitors = [
                MonitorView(
                    display=d,
                    bus=_bus(d),
                    value=self._shown(_bus(d)),
                    busy=_bus(d) in self._pending or _bus(d) in self._writing,
                    selected=self._locked or pos == self._selected,
                )
                for pos, d in enumerate(self._displays)
            ]
            return View(self._detected, self._locked, self._layout, monitors)

    # -- input -----------------------------------------------------------

    def select(self, delta: int) -> None:
        with self._lock:
            if not self._displays:
                return
            self._selected = (self._selected + delta) % len(self._displays)
        self._notify()

    def select_bus(self, bus: int) -> None:
        with self._lock:
            buses = [_bus(d) for d in self._displays]
            if bus not in buses:
                return
            self._selected = buses.index(bus)
        self._notify()

    def step(self, steps: int, size: int, bus: Optional[int] = None) -> None:
        """Move by ``steps`` steps of ``size``, snapping to multiples of it.

        Applies to ``bus`` (default: the selected monitor), or to every
        monitor while locked.
        """
        if steps == 0:
            return
        delta = size if steps > 0 else -size

        def compute(value: int) -> int:
            for _ in range(abs(steps)):
                value = brightness.snap_step(value, delta)
            return value

        self._change(compute, bus)

    def set_level(self, value: int, bus: Optional[int] = None) -> None:
        self._change(lambda _: value, bus)

    def _change(self, compute: Callable[[int], int], bus: Optional[int]) -> None:
        error: Optional[str] = None
        with self._lock:
            if not self._displays:
                return
            if bus is None:
                bus = _bus(self._displays[self._selected])
            base = self._shown(bus)
            if base is None:
                error = f"display {self._label(bus)}: brightness unknown"
            else:
                target = brightness.clamp(compute(base))
                buses = [_bus(d) for d in self._displays] if self._locked else [bus]
                for b in buses:
                    self._queue(b, target)
        if error:
            self._report(error)
        self._notify()

    def toggle_lock(self) -> None:
        """Lock (syncing all monitors to the selected one) or unlock."""
        message: Optional[str] = None
        with self._lock:
            if self._locked:
                self._locked = False
            elif self._displays:
                base = self._shown(_bus(self._displays[self._selected]))
                if base is None:
                    message = "cannot lock: brightness unknown"
                else:
                    self._locked = True
                    for d in self._displays:
                        if self._shown(_bus(d)) != base:
                            self._queue(_bus(d), base)
                    message = f"locked: all monitors at {base}%"
        if message:
            self._report(message)
        self._notify()

    def _label(self, bus: int) -> str:
        return next((d.label for d in self._displays if d.bus == bus), "?")

    # -- debounced writes ------------------------------------------------

    def _queue(self, bus: int, value: int) -> None:
        """Queue ``value`` for ``bus``; restart its debounce. Holds the lock."""
        self._pending[bus] = value
        old = self._timers.get(bus)
        if old is not None:
            old.cancel()
        token = self._tokens.get(bus, 0) + 1
        self._tokens[bus] = token
        timer = threading.Timer(DEBOUNCE, self._debounce_elapsed, (bus, token))
        timer.daemon = True
        self._timers[bus] = timer
        timer.start()

    def _debounce_elapsed(self, bus: int, token: int) -> None:
        with self._lock:
            if self._tokens.get(bus) != token:
                return  # replaced by newer input
            self._timers.pop(bus, None)
            if bus in self._writing:
                return  # the running writer picks the target up
            # Input that netted out (e.g. + then -) needs no write.
            if self._pending.get(bus) == self._values.get(bus):
                self._pending.pop(bus, None)
                start = False
            else:
                start = bus in self._pending
                if start:
                    self._writing.add(bus)
        if start:
            _spawn(lambda: self._write_loop(bus))
        else:
            self._notify()

    def _write_loop(self, bus: int) -> None:
        while True:
            with self._lock:
                target = self._pending.get(bus)
                # New input arrived during the last write: let its debounce
                # finish; _debounce_elapsed starts a fresh writer.
                if target is None or bus in self._timers:
                    self._writing.discard(bus)
                    break
            try:
                brightness.set_value(bus, target)
            except brightness.DdcError:
                with self._lock:
                    self._pending.pop(bus, None)
                    self._writing.discard(bus)
                    label = self._label(bus)
                self._report(f"display {label}: cannot adjust brightness")
                break
            with self._lock:
                self._values[bus] = target
                self._write_gen[bus] = self._write_gen.get(bus, 0) + 1
                # Drop the target only if no newer one arrived during the write.
                if self._pending.get(bus) == target:
                    del self._pending[bus]
            self._notify()
        self._notify()

    # -- detection and polling -------------------------------------------

    def _load(self) -> None:
        displays = [d for d in detect() if d.addressable]
        if len(displays) > MAX_MONITORS:
            self._report(f"showing {MAX_MONITORS} of {len(displays)} monitors")
            displays = displays[:MAX_MONITORS]
        values = _read_all([_bus(d) for d in displays])

        def key(ds: list[Display]) -> list[tuple[Optional[int], str]]:
            return [(d.bus, d.model) for d in ds]

        with self._lock:
            if key(displays) != key(self._displays):
                self._layout += 1
            self._displays = displays
            self._detected = True
            if self._selected >= len(displays):
                self._selected = 0
            self._apply_reads(values)
        self._notify()

    def _poll_loop(self) -> None:
        while not self._stop.wait(REFRESH):
            with self._lock:
                gens = {
                    b: self._write_gen.get(b, 0)
                    for b in (_bus(d) for d in self._displays)
                    if b not in self._writing and b not in self._pending
                }
            if not gens:
                continue
            values = _read_all(list(gens))
            with self._lock:
                # A write that finished while reading makes that reading stale.
                self._apply_reads(
                    {b: v for b, v in values.items() if self._write_gen.get(b, 0) == gens[b]}
                )
            self._notify()

    def _apply_reads(self, values: Readings) -> None:
        """Holds the lock."""
        for bus, val in values.items():
            if bus not in self._pending and bus not in self._writing:
                self._values[bus] = val
