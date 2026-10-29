"""Brightness control (VCP 0x10) over ddcutil.

Displays are addressed by I2C bus number (see ``Display.bus``) rather than
by ``--display N``, which would make ddcutil re-probe every bus per call.
"""
from __future__ import annotations

import re
import subprocess
import time

VCP_CODE = "0x10"  # Brightness
DEFAULT_STEP = 10


class DdcError(RuntimeError):
    pass


def _run(args: list[str], bus: int) -> str:
    cmd = ["ddcutil", "--bus", str(bus)] + args
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


def get_value(bus: int) -> int:
    line = _get_line(bus)
    m = _CURRENT.search(line)
    if m:
        return int(m.group(1))
    raise DdcError(f"cannot read brightness on bus {bus}: {line!r}")


def set_value(bus: int, value: int) -> int:
    """Set brightness and return the value written.

    ddcutil verifies the write itself and reports a mismatch as an error,
    so there is no separate read-back.
    """
    value = clamp(value)
    text = _run(["setvcp", VCP_CODE, str(value)], bus)
    if not _set_ok(text):
        raise DdcError(f"cannot set brightness on bus {bus}: {text!r}")
    return value


def bump_value(bus: int, delta: int) -> int:
    return set_value(bus, get_value(bus) + delta)
