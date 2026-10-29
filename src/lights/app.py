"""Interactive TUI for lights (Textual).

Keys:
  up/down arrows : select the monitor above/below (wraps)
  j              : raise the selected monitor's brightness by 10
  k              : lower the selected monitor's brightness by 10
  shift+j        : raise by 15
  shift+k        : lower by 15
  r              : re-detect monitors and refresh now
  q              : quit

Every ddcutil call takes a few hundred milliseconds, so none of them run on
the UI thread. Key presses update the shown value immediately and queue the
new target; one writer per monitor applies the latest target, so rapid
presses collapse into as few writes as possible. All app state is touched on
the UI thread only; worker threads do I/O and hand results back through
``call_from_thread``.
"""
from __future__ import annotations

from concurrent.futures import ThreadPoolExecutor
from typing import ClassVar, Optional

from textual.app import App, ComposeResult
from textual.binding import Binding, BindingType
from textual.widgets import Footer, Header, Static

from . import brightness
from .monitors import Display, detect

STEP = 10
STEP_BIG = 15
REFRESH = 1.5

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
    #content {
        padding: 1;
        align: left middle;
        height: 100%;
    }
    """

    BINDINGS: ClassVar[list[BindingType]] = [
        Binding("up", "select(-1)", "Prev", show=False),
        Binding("down", "select(1)", "Next", show=False),
        Binding("j", f"bump({STEP})", f"+{STEP}"),
        Binding("k", f"bump(-{STEP})", f"-{STEP}"),
        Binding("J,shift+j", f"bump({STEP_BIG})", f"+{STEP_BIG}"),
        Binding("K,shift+k", f"bump(-{STEP_BIG})", f"-{STEP_BIG}"),
        Binding("r", "reload", "Refresh"),
        Binding("q,enter", "quit", "Quit"),
    ]

    def __init__(self) -> None:
        super().__init__()
        self.selected = 0
        self.displays: list[Display] = []
        self.values: Readings = {}
        self.detected = False
        # Targets waiting to be written, and buses whose writer is running.
        self._pending: dict[int, int] = {}
        self._writing: set[int] = set()
        self._polling = False
        # Bumped per completed write so reads that started earlier are dropped.
        self._write_gen: dict[int, int] = {}

    def compose(self) -> ComposeResult:
        yield Header(icon="◉")
        yield Static("detecting monitors…", id="content")
        yield Footer()

    def on_mount(self) -> None:
        self.title = "lights"
        self.action_reload()
        self.set_interval(REFRESH, self._poll)

    # -- drawing ---------------------------------------------------------

    def _shown(self, bus: int) -> Optional[int]:
        """Value to display: the pending target if any, else the last read."""
        return self._pending.get(bus, self.values.get(bus))

    def _redraw(self) -> None:
        content = self.query_one("#content", Static)
        if not self.detected:
            content.update("detecting monitors…")
            return
        if not self.displays:
            content.update("no monitors responding  (r to retry)")
            return
        lines: list[str] = []
        for pos, d in enumerate(self.displays):
            val = self._shown(self._bus(d))
            mark = ">" if pos == self.selected else " "
            pct = f"{val}%  " if val is not None else "  n/a "
            lines.append(f"{mark} {d.label:>2}  {d.model:<30}{pct}")
        lines.append(
            "\n  j +10  |  k -10  |  shift+j/k +/-15  |  "
            "up/down select  |  r refresh  |  q quit"
        )
        content.update("\n".join(lines))

    @staticmethod
    def _bus(d: Display) -> int:
        assert d.bus is not None  # only addressable displays are kept
        return d.bus

    # -- detection and polling -------------------------------------------

    def action_reload(self) -> None:
        self.run_worker(self._load, thread=True, group="load", exclusive=True)

    def _load(self) -> None:
        displays = [d for d in detect() if d.addressable]
        values = _read_all([self._bus(d) for d in displays])
        self.call_from_thread(self._loaded, displays, values)

    def _loaded(self, displays: list[Display], values: Readings) -> None:
        self.displays = displays
        self.detected = True
        if self.selected >= len(displays):
            self.selected = 0
        self._apply_reads(values)

    def _poll(self) -> None:
        if self._polling or not self.displays:
            return
        # Skip buses with a write in flight: the bus is busy and the read
        # would race the write.
        buses = [
            b for b in (self._bus(d) for d in self.displays)
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
        if not self.displays:
            return
        d = self.displays[self.selected]
        bus = self._bus(d)
        base = self._shown(bus)
        if base is None:
            self.notify(f"display {d.label}: brightness unknown", severity="error")
            return
        self._pending[bus] = brightness.clamp(base + delta)
        self._redraw()
        if bus not in self._writing:
            self._writing.add(bus)
            self.run_worker(
                lambda: self._write_loop(bus), thread=True, group=f"write-{bus}"
            )

    # -- writer (worker thread) ------------------------------------------

    def _next_target(self, bus: int) -> Optional[int]:
        target = self._pending.get(bus)
        if target is None:
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
