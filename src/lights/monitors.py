"""Display detection via ddcutil.

The ``--display N`` index accepted by ddcutil equals the ``Display N``
label printed by ``ddcutil detect`` for every monitor whose DDC link is
currently up. A monitor whose DDC link is down prints ``Invalid display``
(no number) and is not addressable.

Detection is slow (every bus is probed), so callers detect once and then
address each monitor by its I2C bus (``ddcutil --bus N``), which skips
that probe and is roughly twice as fast per call.
"""
from __future__ import annotations

import re
import subprocess
from dataclasses import dataclass
from typing import Optional

_DISPLAY_RE = re.compile(r"^Display (\d+)")
_INVALID_RE = re.compile(r"^Invalid display")
_BUS_NUM_RE = re.compile(r"i2c-(\d+)$")
I2C_RE = re.compile(r"I2C bus:\s+(\S+)")
CONN_RE = re.compile(r"DRM connector:\s+(\S+)")
MFG_RE = re.compile(r"Mfg id:\s+(.*)$")
MODEL_RE = re.compile(r"Model:\s+(.*)$")


@dataclass
class Display:
    index: Optional[int]
    i2c_bus: str = ""
    connector: str = ""
    mfg: str = ""
    model: str = ""
    ddc_failed: bool = False

    @property
    def bus(self) -> Optional[int]:
        """Bus number for ``ddcutil --bus``, e.g. 8 for ``/dev/i2c-8``."""
        m = _BUS_NUM_RE.search(self.i2c_bus)
        return int(m.group(1)) if m else None

    @property
    def addressable(self) -> bool:
        return self.index is not None and self.bus is not None and not self.ddc_failed

    @property
    def label(self) -> str:
        return str(self.index) if self.index is not None else "?"


def _run(cmd: list[str]) -> str:
    p = subprocess.run(
        cmd, capture_output=True, text=True, encoding="utf-8", errors="replace"
    )
    return p.stdout + p.stderr


def detect_raw() -> str:
    return _run(["ddcutil", "detect"])


def detect() -> list[Display]:
    displays: list[Display] = []
    current: Optional[Display] = None
    for line in detect_raw().splitlines():
        s = line.strip()
        if (m := _DISPLAY_RE.match(s)):
            current = Display(index=int(m.group(1)))
            displays.append(current)
            continue
        if _INVALID_RE.match(s):
            current = Display(index=None, ddc_failed=True)
            displays.append(current)
            continue
        if current is None:
            continue
        if (m := I2C_RE.search(s)):
            current.i2c_bus = m.group(1)
        elif (m := CONN_RE.search(s)):
            current.connector = m.group(1)
        elif (m := MFG_RE.search(s)):
            current.mfg = m.group(1).strip()
        elif (m := MODEL_RE.search(s)):
            current.model = m.group(1).strip()
        elif "DDC communication failed" in s:
            current.ddc_failed = True
    return displays
