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
"""lights CLI: detect, info, get, set, up, down."""
from __future__ import annotations

import argparse
import json
import sys
from collections.abc import Callable
from concurrent.futures import ThreadPoolExecutor
from typing import Union

from . import brightness, legal, monitors
from . import __version__
from .monitors import Display

# A value read/written, or the error that prevented it.
Result = Union[int, brightness.DdcError]


def _resolve_target(displays: list[Display], target: str) -> list[Display]:
    """Return the addressable displays matching the requested target."""
    usable = [d for d in displays if d.addressable]
    if target in ("all", "*"):
        return usable
    # numeric index (check before model keyword, which may contain digits)
    if target.lstrip("-").isdigit():
        idx = int(target)
        hit = [d for d in usable if d.index == idx]
        if hit:
            return hit
        raise ValueError(f"no display with index {idx}")
    # model or manufacturer keyword
    low = target.lower()
    matches = [d for d in usable if low in d.model.lower() or low in d.mfg.lower()]
    if matches:
        return matches
    raise ValueError(f"unknown target {target!r}")


def _bus(d: Display) -> int:
    assert d.bus is not None  # guaranteed by _resolve_target (addressable)
    return d.bus


def _run_all(targets: list[Display], func: Callable[[int], int]) -> list[Result]:
    """Run ``func(bus)`` for every target, in parallel (one bus each)."""

    def one(d: Display) -> Result:
        try:
            return func(_bus(d))
        except brightness.DdcError as e:
            return e

    with ThreadPoolExecutor(max_workers=max(1, len(targets))) as pool:
        return list(pool.map(one, targets))


def _print_display(d: Display) -> None:
    status = "ok" if d.addressable else "DDC down"
    print(
        f"  [{d.label}] {d.model or d.i2c_bus or 'unknown'} "
        f"({d.connector}) i2c:{d.i2c_bus} [{status}]"
    )


def cmd_detect() -> int:
    print(monitors.detect_raw())
    return 0


def cmd_info() -> int:
    for d in monitors.detect():
        _print_display(d)
    return 0


def cmd_get(target: str, json_: bool) -> int:
    targets = _resolve_target(monitors.detect(), target)
    results = _run_all(targets, brightness.get_value)
    rc = 0
    for d, r in zip(targets, results):
        if isinstance(r, brightness.DdcError):
            print(f"error reading display {d.label}: {r}", file=sys.stderr)
            rc = 1
    if json_:
        out: list[dict[str, Union[int, str, None]]] = [
            {"display": d.index, "model": d.model, "brightness": r}
            for d, r in zip(targets, results)
            if not isinstance(r, brightness.DdcError)
        ]
        print(json.dumps(out, indent=2))
        return rc
    for d, r in zip(targets, results):
        if not isinstance(r, brightness.DdcError):
            print(f"display {d.label} ({d.model}): {r}%")
    return rc


def _apply(target: str, func: Callable[[int], int]) -> int:
    targets = _resolve_target(monitors.detect(), target)
    rc = 0
    for d, r in zip(targets, _run_all(targets, func)):
        if isinstance(r, brightness.DdcError):
            print(f"error on display {d.label} ({d.model}): {r}", file=sys.stderr)
            rc = 1
            continue
        print(f"display {d.label} ({d.model}): {r}%")
    return rc


def cmd_set(target: str, percent: int) -> int:
    return _apply(target, lambda bus: brightness.set_value(bus, percent))


def cmd_up(target: str, step: int) -> int:
    return _apply(target, lambda bus: brightness.bump_value(bus, step))


def cmd_down(target: str, step: int) -> int:
    return _apply(target, lambda bus: brightness.bump_value(bus, -step))


def cmd_show(what: str) -> int:
    texts = {"w": legal.WARRANTY, "c": legal.license_text(), "n": legal.notices_text()}
    print(texts[what], end="")
    return 0


def cmd_app() -> int:
    from .app import main

    main()
    return 0


def main() -> int:
    p = argparse.ArgumentParser(
        prog="lights",
        description="Brightness controller for monitors via ddcutil",
        epilog=legal.notice("type `lights show w'", "type `lights show c'"),
        formatter_class=argparse.RawDescriptionHelpFormatter,
    )
    p.add_argument(
        "--version",
        action="version",
        version=f"lights {__version__}\n{legal.VERSION_TEXT}",
    )
    sub = p.add_subparsers(dest="cmd", required=True)

    sub.add_parser("detect", help="raw ddcutil monitor detection")
    sub.add_parser("info", help="list detected displays")

    g = sub.add_parser("get", help="show current brightness")
    g.add_argument("target", nargs="?", default="all")
    g.add_argument("--json", action="store_true", help="print JSON")

    s = sub.add_parser("set", help="set brightness percent (0-100)")
    s.add_argument("target", nargs="?", default="all")
    s.add_argument("percent", type=int)

    u = sub.add_parser("up", help="increase brightness")
    u.add_argument("target", nargs="?", default="all")
    u.add_argument("step", nargs="?", type=int, default=brightness.DEFAULT_STEP)

    d = sub.add_parser("down", help="decrease brightness")
    d.add_argument("target", nargs="?", default="all")
    d.add_argument("step", nargs="?", type=int, default=brightness.DEFAULT_STEP)

    sub.add_parser("app", help="launch the interactive TUI")

    sh = sub.add_parser(
        "show",
        help="show the warranty (w), licence conditions (c) or third-party notices (n)",
    )
    sh.add_argument("what", choices=["w", "c", "n"])

    args = p.parse_args()
    cmd: str = args.cmd
    try:
        if cmd == "detect":
            return cmd_detect()
        if cmd == "info":
            return cmd_info()
        if cmd == "get":
            return cmd_get(args.target, args.json)
        if cmd == "set":
            return cmd_set(args.target, args.percent)
        if cmd == "up":
            return cmd_up(args.target, args.step)
        if cmd == "app":
            return cmd_app()
        if cmd == "show":
            return cmd_show(args.what)
        return cmd_down(args.target, args.step)
    except ValueError as exc:
        print(f"error: {exc}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
