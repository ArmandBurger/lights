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
"""GTK 4 desktop UI for lights.

Same model as the TUI: each monitor is a screen with a minus half and a plus
half (click to step by 5, snapping to multiples of 5), its name and info
underneath. Every action is also on the keyboard with vim-style keys (see
``keys.HELP``), handled before GTK's own focus navigation. The About dialog
carries the copyright, licence and third-party notices.
"""
from __future__ import annotations

import threading
from collections.abc import Callable
from pathlib import Path
from typing import Optional

import gi

gi.require_version("Gdk", "4.0")
gi.require_version("Graphene", "1.0")
gi.require_version("Gtk", "4.0")
gi.require_version("Pango", "1.0")
from gi.repository import Gdk, Gio, GLib, Graphene, Gtk, Pango  # noqa: E402

from . import __version__, legal  # noqa: E402
from .controller import Controller, MonitorView, grid_shape  # noqa: E402
from .keys import HELP, STEP, Action, VimKeys  # noqa: E402

APP_ID = "io.github.armandburger.lights"
# An icon theme root (hicolor/scalable/actions/*-symbolic.svg): GTK only
# recolours symbolic icons to the theme colour when they sit in a theme layout.
ICONS_DIR = Path(__file__).parent / "icons"
ICON_ADD = "ic_fluent_add_24_filled-symbolic"
ICON_SUBTRACT = "ic_fluent_subtract_24_filled-symbolic"
ICON_HIGH = "ic_fluent_brightness_high_24_filled-symbolic"
ICON_LOW = "ic_fluent_brightness_low_24_filled-symbolic"
MESSAGE_SECONDS = 4

# Window sizing: each monitor card is about CARD_WIDTH x CARD_HEIGHT at the
# requested size; the window never asks for more than SCREEN_FRACTION of the
# screen. Cards and icons shrink and grow with the window after that.
CARD_WIDTH = 280
CARD_HEIGHT = 200
GAP = 16
CHROME_HEIGHT = 84  # header bar + status bar
SCREEN_FRACTION = 0.9

CSS = """
.screen {
    border: 2px solid alpha(currentColor, 0.3);
    border-radius: 14px;
}
.screen.selected {
    border-color: @theme_selected_bg_color;
}
.screen button.step {
    background: none;
    border: none;
    box-shadow: none;
    min-height: 0;
    min-width: 0;
    padding: 0;
}
.screen button.step.minus { border-radius: 12px 0 0 12px; }
.screen button.step.plus { border-radius: 0 12px 12px 0; }
.screen button.step:hover { background: alpha(currentColor, 0.07); }
.screen button.step:active { background: alpha(currentColor, 0.14); }
.screen button.step:hover { color: @theme_selected_bg_color; }
.model { font-size: 13pt; font-weight: bold; }
.level { font-size: 13pt; font-weight: bold; color: @theme_selected_bg_color; }
.info { opacity: 0.6; }
.statusbar {
    padding: 4px 12px;
    border-top: 1px solid alpha(currentColor, 0.15);
}
.pending { font-family: monospace; }
"""

_KEY_NAMES = {
    Gdk.KEY_Left: "left",
    Gdk.KEY_Right: "right",
    Gdk.KEY_Up: "up",
    Gdk.KEY_Down: "down",
    Gdk.KEY_Escape: "escape",
}


class ScalableIcon(Gtk.Widget):
    """A symbolic theme icon drawn to fit the space it is given.

    Gtk.Image draws at a fixed pixel size that is also its minimum size, which
    would stop the window from shrinking; this widget scales the icon to
    ``fraction`` of its smaller side instead, in the theme's text colour.
    """

    __gtype_name__ = "LightsScalableIcon"
    LOAD_SIZE = 128  # rasterised once at this size, then scaled down

    def __init__(self, icon_name: str, fraction: float, max_size: int) -> None:
        super().__init__(hexpand=True, vexpand=True)
        self._icon_name = icon_name
        self._fraction = fraction
        self._max_size = max_size
        self._paintable: Optional[Gtk.IconPaintable] = None
        self._scale = 0

    def do_measure(
        self, orientation: object, for_size: int
    ) -> tuple[int, int, int, int]:
        return 16, 48, -1, -1

    def do_snapshot(self, snapshot: Gtk.Snapshot) -> None:
        width, height = self.get_width(), self.get_height()
        size = min(min(width, height) * self._fraction, self._max_size)
        if size < 1:
            return
        scale = self.get_scale_factor()
        if self._paintable is None or scale != self._scale:
            theme = Gtk.IconTheme.get_for_display(self.get_display())
            self._paintable = theme.lookup_icon(
                self._icon_name, None, self.LOAD_SIZE, scale,
                Gtk.TextDirection.NONE, Gtk.IconLookupFlags(0),
            )
            self._scale = scale
        offset = Graphene.Point()
        offset.init((width - size) / 2, (height - size) / 2)
        snapshot.save()
        snapshot.translate(offset)
        self._paintable.snapshot_symbolic(snapshot, size, size, [self.get_color()])
        snapshot.restore()


class MonitorCard(Gtk.Box):
    """One monitor: a screen with minus/plus halves and a caption."""

    def __init__(
        self,
        view: MonitorView,
        on_step: Callable[[int, int], None],
        on_select: Callable[[int], None],
    ) -> None:
        super().__init__(
            orientation=Gtk.Orientation.VERTICAL, spacing=6, hexpand=True, vexpand=True
        )
        self.bus = view.bus
        self._on_step = on_step

        self.screen = Gtk.Box(css_classes=["screen"], vexpand=True)
        minus = self._half(ICON_SUBTRACT, "minus", f"Dimmer by {STEP}%  (j)")
        plus = self._half(ICON_ADD, "plus", f"Brighter by {STEP}%  (k)")
        minus.connect("clicked", self._on_minus)
        plus.connect("clicked", self._on_plus)
        self.screen.append(minus)
        self.screen.append(
            Gtk.Separator(
                orientation=Gtk.Orientation.VERTICAL, margin_top=16, margin_bottom=16
            )
        )
        self.screen.append(plus)
        self.append(self.screen)

        d = view.display
        caption = Gtk.Box(spacing=8, halign=Gtk.Align.CENTER)
        self.level_icon = Gtk.Image(pixel_size=20)
        self.level = Gtk.Label(css_classes=["level"])
        caption.append(
            Gtk.Label(
                label=d.model or "unknown",
                css_classes=["model"],
                ellipsize=Pango.EllipsizeMode.END,
            )
        )
        caption.append(self.level_icon)
        caption.append(self.level)
        self.append(caption)
        bus_name = d.i2c_bus.removeprefix("/dev/")
        self.append(
            Gtk.Label(
                label=f"display {d.label} · {d.connector} · {bus_name}",
                css_classes=["info"],
                ellipsize=Pango.EllipsizeMode.END,
            )
        )

        click = Gtk.GestureClick()
        click.connect("pressed", lambda *_: on_select(self.bus))
        self.add_controller(click)
        self.update(view)

    def _on_minus(self, _button: Gtk.Button) -> None:
        self._on_step(self.bus, -1)

    def _on_plus(self, _button: Gtk.Button) -> None:
        self._on_step(self.bus, 1)

    @staticmethod
    def _half(icon: str, side: str, tooltip: str) -> Gtk.Button:
        return Gtk.Button(
            child=ScalableIcon(icon, fraction=0.5, max_size=96),
            css_classes=["step", side],
            hexpand=True,
            focus_on_click=False,
            tooltip_text=tooltip,
        )

    def update(self, view: MonitorView) -> None:
        value = view.value
        self.level.set_label(
            ("n/a" if value is None else f"{value}%") + (" …" if view.busy else "")
        )
        self.level_icon.set_from_icon_name(
            ICON_LOW if value is not None and value < 50 else ICON_HIGH
        )
        if view.selected:
            self.screen.add_css_class("selected")
        else:
            self.screen.remove_css_class("selected")


def _window_size(
    columns: int, rows: int, display: Optional[Gdk.Display] = None
) -> tuple[int, int]:
    """Window size that fits a grid of cards, capped to the screen."""
    width = columns * CARD_WIDTH + (columns + 1) * GAP
    height = rows * CARD_HEIGHT + (rows + 1) * GAP + CHROME_HEIGHT
    display = display or Gdk.Display.get_default()
    if display is not None:
        monitors = display.get_monitors()
        monitor = monitors.get_item(0)
        if isinstance(monitor, Gdk.Monitor):
            area = monitor.get_geometry()
            width = min(width, int(area.width * SCREEN_FRACTION))
            height = min(height, int(area.height * SCREEN_FRACTION))
    return width, height


class LightsWindow(Gtk.ApplicationWindow):
    def __init__(self, app: Gtk.Application) -> None:
        width, height = _window_size(*grid_shape(2))
        super().__init__(
            application=app, title="lights", default_width=width, default_height=height
        )
        self.controller = Controller(self._on_change, self._on_report)
        self.vim = VimKeys()
        self._layout = -1
        self._shape = (0, 0)
        self._cards: list[MonitorCard] = []
        self._sync_queued = False
        self._message_token = 0
        self._quitting = False

        header = Gtk.HeaderBar()
        self.lock_button = Gtk.ToggleButton(
            label="Unlocked", tooltip_text="Lock: sync and adjust all monitors together  (s)"
        )
        self.lock_button.connect("clicked", self._on_lock_clicked)
        header.pack_start(self.lock_button)
        about = Gtk.Button(icon_name="help-about-symbolic", tooltip_text="About and licences")
        about.connect("clicked", self._on_about_clicked)
        self.keys_button = Gtk.MenuButton(
            icon_name="input-keyboard-symbolic",
            tooltip_text="Keyboard shortcuts  (?)",
            popover=self._help_popover(),
        )
        refresh = Gtk.Button(icon_name="view-refresh-symbolic", tooltip_text="Re-detect monitors  (r)")
        refresh.connect("clicked", self._on_refresh_clicked)
        header.pack_end(about)
        header.pack_end(self.keys_button)
        header.pack_end(refresh)
        self.set_titlebar(header)

        self.stack = Gtk.Stack(vexpand=True)
        self.status = Gtk.Label(label="detecting monitors…", css_classes=["title-2"])
        self.stack.add_named(self.status, "status")
        self.monitors = Gtk.Grid(
            row_homogeneous=True, column_homogeneous=True,
            row_spacing=GAP, column_spacing=GAP,
            margin_top=GAP, margin_bottom=GAP, margin_start=GAP, margin_end=GAP,
        )
        self.stack.add_named(self.monitors, "monitors")

        bar = Gtk.Box(css_classes=["statusbar"])
        self.message = Gtk.Label(hexpand=True, xalign=0, ellipsize=Pango.EllipsizeMode.END)
        self.pending = Gtk.Label(css_classes=["pending"])
        bar.append(self.message)
        bar.append(self.pending)

        body = Gtk.Box(orientation=Gtk.Orientation.VERTICAL)
        body.append(self.stack)
        body.append(bar)
        self.set_child(body)

        keys = Gtk.EventControllerKey(propagation_phase=Gtk.PropagationPhase.CAPTURE)
        keys.connect("key-pressed", self._on_key)
        self.add_controller(keys)
        self.connect("close-request", self._on_close_request)

        self.controller.start()

    # -- controller callbacks (any thread) -------------------------------

    def _on_change(self) -> None:
        if not self._sync_queued:
            self._sync_queued = True
            GLib.idle_add(self._sync)

    def _on_report(self, message: str) -> None:
        GLib.idle_add(self._show_message, message)

    # -- drawing ---------------------------------------------------------

    def _sync(self) -> bool:
        self._sync_queued = False
        view = self.controller.view()
        self.lock_button.set_active(view.locked)
        self.lock_button.set_label("Locked · synced" if view.locked else "Unlocked")

        if not view.monitors:
            self.status.set_label(
                "detecting monitors…" if not view.detected
                else "no monitors responding  (r to retry)"
            )
            self.stack.set_visible_child_name("status")
        else:
            self.stack.set_visible_child_name("monitors")

        if view.layout != self._layout:
            self._layout = view.layout
            for card in self._cards:
                self.monitors.remove(card)
            self._cards = [
                MonitorCard(m, self._on_card_step, self.controller.select_bus)
                for m in view.monitors
            ]
            columns, rows = grid_shape(len(self._cards))
            for i, card in enumerate(self._cards):
                self.monitors.attach(card, i % columns, i // columns, 1, 1)
            if self._cards and (columns, rows) != self._shape:
                self._shape = (columns, rows)
                self.set_default_size(*_window_size(columns, rows, self.get_display()))
        for card, m in zip(self._cards, view.monitors):
            card.update(m)
        return GLib.SOURCE_REMOVE

    def _show_message(self, message: str) -> bool:
        self._message_token += 1
        token = self._message_token
        self.message.set_label(message)

        def clear() -> bool:
            if token == self._message_token:
                self.message.set_label("")
            return GLib.SOURCE_REMOVE

        GLib.timeout_add_seconds(MESSAGE_SECONDS, clear)
        return GLib.SOURCE_REMOVE

    def _help_popover(self) -> Gtk.Popover:
        grid = Gtk.Grid(
            column_spacing=24, row_spacing=6,
            margin_top=12, margin_bottom=12, margin_start=12, margin_end=12,
        )
        for row, (keys, what) in enumerate(HELP):
            grid.attach(Gtk.Label(label=keys, xalign=0, css_classes=["pending"]), 0, row, 1, 1)
            grid.attach(Gtk.Label(label=what, xalign=0), 1, row, 1, 1)
        return Gtk.Popover(child=grid)

    def show_about(self) -> None:
        dialog = Gtk.AboutDialog(
            transient_for=self,
            modal=True,
            program_name="lights",
            version=__version__,
            comments="Brightness controller for monitors via ddcutil",
            copyright="Copyright © 2026 Armand Burger",
            authors=["Armand Burger https://github.com/ArmandBurger"],
            website="https://github.com/ArmandBurger",
            website_label="GitHub @ArmandBurger · Twitter/X @ghastly_dev",
            logo_icon_name=ICON_HIGH,
            license_type=Gtk.License.CUSTOM,
            license="\n\n".join(
                [legal.notice_gui(), legal.notices_text(), legal.license_text()]
            ),
            wrap_license=False,
        )
        dialog.add_credit_section(
            "Icons",
            [
                "Fluent UI System Icons © Microsoft Corporation (MIT License) "
                "https://github.com/microsoft/fluentui-system-icons"
            ],
        )
        dialog.present()

    # -- input -----------------------------------------------------------

    def _on_lock_clicked(self, _button: Gtk.Button) -> None:
        self.controller.toggle_lock()

    def _on_about_clicked(self, _button: Gtk.Button) -> None:
        self.show_about()

    def _on_refresh_clicked(self, _button: Gtk.Button) -> None:
        self.controller.reload()

    def _on_card_step(self, bus: int, direction: int) -> None:
        self.controller.select_bus(bus)
        self.controller.step(direction, STEP, bus)

    def _on_key(
        self,
        _controller: Gtk.EventControllerKey,
        keyval: int,
        _keycode: int,
        state: Gdk.ModifierType,
    ) -> bool:
        if state & (Gdk.ModifierType.CONTROL_MASK | Gdk.ModifierType.ALT_MASK):
            return False
        name = _KEY_NAMES.get(keyval)
        if name is None:
            codepoint = Gdk.keyval_to_unicode(keyval)
            if not codepoint:
                return False
            name = chr(codepoint)
        action = self.vim.feed(name)
        self.pending.set_label(self.vim.pending)
        if action is None:
            return False
        self._run(action)
        return True

    def _run(self, action: Action) -> None:
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
            self.keys_button.popup()
        elif action.kind == "quit":
            self.quit()

    # -- quitting --------------------------------------------------------

    def _on_close_request(self, _window: Gtk.Window) -> bool:
        self.quit()
        return True  # closed by quit() once pending writes are done

    def quit(self) -> None:
        """Hide at once, write still-debouncing targets, then exit."""
        if self._quitting:
            return
        self._quitting = True
        self.set_visible(False)
        app = self.get_application()

        def work() -> None:
            self.controller.shutdown()
            if app is not None:
                GLib.idle_add(app.quit)

        threading.Thread(target=work, daemon=True).start()


class LightsApplication(Gtk.Application):
    def __init__(self) -> None:
        super().__init__(application_id=APP_ID, flags=Gio.ApplicationFlags.DEFAULT_FLAGS)
        self.window: Optional[LightsWindow] = None

    def do_startup(self) -> None:
        Gtk.Application.do_startup(self)
        display = Gdk.Display.get_default()
        assert display is not None
        Gtk.IconTheme.get_for_display(display).add_search_path(str(ICONS_DIR))
        provider = Gtk.CssProvider()
        provider.load_from_string(CSS)
        Gtk.StyleContext.add_provider_for_display(
            display, provider, Gtk.STYLE_PROVIDER_PRIORITY_APPLICATION
        )

    def do_activate(self) -> None:
        # A second launch re-activates the running instance.
        if self.window is None:
            self.window = LightsWindow(self)
        self.window.present()


def main() -> int:
    return LightsApplication().run(None)
