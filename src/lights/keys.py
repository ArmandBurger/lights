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
"""Vim-style keys shared by the TUI and the GUI.

Both UIs translate their key events into names (a printable character such
as ``"k"``, ``"K"``, ``"5"``, ``"%"``, or ``"left"``/``"right"``/``"up"``/
``"down"``/``"escape"``) and feed them to ``VimKeys``, which returns what to
do. A count prefix repeats a motion, as in vim: ``3k`` is three steps up.
"""
from __future__ import annotations

from dataclasses import dataclass
from typing import Literal, Optional

STEP = 5  # k / j, and a mouse click
STEP_BIG = 10  # K / J

ActionKind = Literal[
    "pending",  # key consumed, waiting for more (count or g)
    "select",  # amount: monitors to move by
    "step",  # amount: steps (signed); size: step size
    "set",  # amount: brightness percent
    "lock",
    "reload",
    "help",
    "quit",
]


@dataclass(frozen=True)
class Action:
    kind: ActionKind
    amount: int = 0
    size: int = 0


HELP: list[tuple[str, str]] = [
    ("h / l  (← / →)", "select previous / next monitor"),
    ("k / j  (↑ / ↓)", f"brighter / dimmer by {STEP}%"),
    ("K / J", f"brighter / dimmer by {STEP_BIG}%"),
    ("<count>", "repeat, e.g. 3k"),
    ("<n>%", "set brightness to n%, e.g. 70%"),
    ("gg / G", "100% / 0%"),
    ("s", "lock: sync and adjust all monitors together"),
    ("r", "re-detect monitors"),
    ("?", "show keys"),
    ("q", "quit"),
    ("Esc", "cancel a pending count"),
]

HELP_LINE = (
    "h/l select · k/j ±5 · K/J ±10 · 3k repeat · 70% set · "
    "gg/G max/min · s lock · r refresh · ? keys · q quit"
)

_MOVES: dict[str, Action] = {
    "h": Action("select", -1),
    "left": Action("select", -1),
    "l": Action("select", 1),
    "right": Action("select", 1),
    "k": Action("step", 1, STEP),
    "up": Action("step", 1, STEP),
    "j": Action("step", -1, STEP),
    "down": Action("step", -1, STEP),
    "K": Action("step", 1, STEP_BIG),
    "J": Action("step", -1, STEP_BIG),
}

_COMMANDS: dict[str, Action] = {
    "s": Action("lock"),
    "r": Action("reload"),
    "?": Action("help"),
    "q": Action("quit"),
}


class VimKeys:
    def __init__(self) -> None:
        self._count = ""
        self._g = False

    @property
    def pending(self) -> str:
        """Keys typed so far for an unfinished command (vim's showcmd)."""
        return self._count + ("g" if self._g else "")

    def reset(self) -> None:
        self._count = ""
        self._g = False

    def feed(self, key: str) -> Optional[Action]:
        """Handle one key; ``None`` means the key is not ours."""
        count = int(self._count) if self._count else None
        if key.isdigit() and len(key) == 1 and not self._g:
            if key == "0" and not self._count:
                self.reset()
                return None
            if len(self._count) < 3:
                self._count += key
            return Action("pending")
        if key == "g":
            if self._g:
                self.reset()
                return Action("set", 100 if count is None else min(count, 100))
            self._g = True
            return Action("pending")
        had_pending = bool(self.pending)
        self.reset()
        if key == "escape":
            return Action("pending") if had_pending else None
        if key == "G":
            return Action("set", 0 if count is None else min(count, 100))
        if key == "%":
            return Action("set", min(count, 100)) if count is not None else None
        if key in _MOVES:
            move = _MOVES[key]
            return Action(move.kind, move.amount * (count or 1), move.size)
        return _COMMANDS.get(key)
