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
"""Brightness control (VCP 0x10) over ddcutil.

Displays are addressed by I2C bus number (see ``Display.bus``) rather than
by ``--display N``, which would make ddcutil re-probe every bus per call.
"""
from __future__ import annotations

import re
import subprocess
import threading
import time

VCP_CODE = "0x10"  # Brightness
DEFAULT_STEP = 10


class DdcError(RuntimeError):
    pass


# One ddcutil call per bus at a time: overlapping calls on the same bus
# (e.g. a background read during a write) make the monitor drop commands.
_locks_guard = threading.Lock()
_bus_locks: dict[int, threading.Lock] = {}


def _bus_lock(bus: int) -> threading.Lock:
    with _locks_guard:
        return _bus_locks.setdefault(bus, threading.Lock())


def _run(args: list[str], bus: int) -> str:
    cmd = ["ddcutil", "--bus", str(bus)] + args
    with _bus_lock(bus):
        p = subprocess.run(
            cmd, capture_output=True, text=True, encoding="utf-8", errors="replace"
        )
    return (p.stdout + "\n" + p.stderr).strip()


ERROR_MARKERS = ("failed", "not found", "unrecognized", "unknown option", "invalid")


def _set_ok(text: str) -> bool:
    if not text:
        return True
    return not any(m in text.lower() for m in ERROR_MARKERS)


def _get_line(bus: int, retries: int = 3, delay: float = 0.25) -> str:
    line = _run(["getvcp", VCP_CODE], bus)
    for _ in range(retries):
        if "current value =" in line:
            break
        time.sleep(delay)
        line = _run(["getvcp", VCP_CODE], bus)
    return line


_CURRENT = re.compile(r"current value\s*=\s*(\d+),\s*max value\s*=\s*(\d+)")


def clamp(value: int) -> int:
    return max(0, min(100, value))


def snap_step(value: int, delta: int) -> int:
    """Step by ``delta``, landing on a multiple of ``|delta|``.

    An off-grid value settles on the next grid point first, so with a step
    of 5: 74 +5 -> 75, 75 +5 -> 80, 74 -5 -> 70, 70 -5 -> 65.
    """
    step = abs(delta)
    if delta > 0:
        return clamp((value // step + 1) * step)
    return clamp((-(-value // step) - 1) * step)


def get_value(bus: int) -> int:
    line = _get_line(bus)
    m = _CURRENT.search(line)
    if m:
        return int(m.group(1))
    raise DdcError(f"cannot read brightness on bus {bus}: {line!r}")


def set_value(bus: int, value: int, retries: int = 2, delay: float = 0.25) -> int:
    """Set brightness and return the value written.

    ddcutil verifies the write itself and reports a mismatch as an error,
    so there is no separate read-back. Monitors occasionally drop a write,
    so failures are retried like reads.
    """
    value = clamp(value)
    text = ""
    for attempt in range(retries + 1):
        if attempt:
            time.sleep(delay)
        text = _run(["--verify", "setvcp", VCP_CODE, str(value)], bus)
        if _set_ok(text):
            return value
    raise DdcError(f"cannot set brightness on bus {bus}: {text!r}")


def bump_value(bus: int, delta: int) -> int:
    return set_value(bus, get_value(bus) + delta)
