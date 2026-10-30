// lights - brightness controller for monitors via ddcutil
// Copyright (C) 2026  Armand Burger
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
//
// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License
// along with this program.  If not, see <https://www.gnu.org/licenses/>.

// A panel applet for lights: a slider per monitor and the presets, all run
// through the `lights` command. Scroll on the icon to step every monitor.

const Applet = imports.ui.applet;
const PopupMenu = imports.ui.popupMenu;
const Clutter = imports.gi.Clutter;
const Gio = imports.gi.Gio;
const GLib = imports.gi.GLib;

const STEP = 5;
const QUIET_MS = 400;

function findLights() {
    return GLib.find_program_in_path("lights")
        || GLib.build_filenamev([GLib.get_home_dir(), ".cargo", "bin", "lights"]);
}

class LightsApplet extends Applet.IconApplet {
    constructor(metadata, orientation, panelHeight, instanceId) {
        super(orientation, panelHeight, instanceId);
        this.set_applet_icon_symbolic_name("display-brightness-symbolic");
        this.set_applet_tooltip("Brightness");

        this.lights = findLights();
        // One lights command at a time: ddcutil calls on the same monitor
        // must not overlap.
        this.queue = [];
        this.running = false;
        this.timers = new Map();
        this.scrolled = 0;

        this.menuManager = new PopupMenu.PopupMenuManager(this);
        this.menu = new Applet.AppletPopupMenu(this, orientation);
        this.menuManager.addMenu(this.menu);

        this.monitors = new PopupMenu.PopupMenuSection();
        this.presets = new PopupMenu.PopupMenuSection();
        this.menu.addMenuItem(this.monitors);
        this.menu.addMenuItem(new PopupMenu.PopupSeparatorMenuItem());
        this.menu.addMenuItem(this.presets);
        this.menu.addMenuItem(new PopupMenu.PopupSeparatorMenuItem());
        this.menu.addAction("Open lights", () => this.spawn(["gui"]));

        this.actor.connect("scroll-event", (actor, event) => this.onScroll(event));
    }

    on_applet_clicked() {
        if (!this.menu.isOpen) {
            this.refresh();
        }

        this.menu.toggle();
    }

    on_applet_removed_from_panel() {
        for (const timer of this.timers.values()) {
            GLib.source_remove(timer);
        }

        this.timers.clear();
    }

    refresh() {
        this.monitors.removeAll();
        this.monitors.addMenuItem(this.note("Reading monitors…"));
        this.run(["get", "--json"], (ok, out, err) => this.showMonitors(ok ? out : null, err));
        this.run(["preset", "--json"], (ok, out) => this.showPresets(ok ? out : null));
    }

    note(text) {
        return new PopupMenu.PopupMenuItem(text, { reactive: false });
    }

    showMonitors(json, err) {
        this.monitors.removeAll();
        const monitors = json ? JSON.parse(json) : [];

        if (monitors.length === 0) {
            this.monitors.addMenuItem(this.note(err.trim() || "No monitors responding"));
            return;
        }

        for (const m of monitors) {
            const label = this.note(`${m.model || "display " + m.display}   ${m.brightness}%`);
            const slider = new PopupMenu.PopupSliderMenuItem(m.brightness / 100);

            slider.connect("value-changed", (item, value) => {
                const level = Math.round(value * 100);
                label.label.text = `${m.model || "display " + m.display}   ${level}%`;
                this.later(m.display, ["set", String(m.display), String(level)]);
            });

            this.monitors.addMenuItem(label);
            this.monitors.addMenuItem(slider);
        }
    }

    showPresets(json) {
        this.presets.removeAll();

        for (const p of json ? JSON.parse(json) : []) {
            this.presets.addAction(`${p.name}   ${p.brightness}%`, () => {
                this.enqueue(["preset", p.name], () => this.refresh());
            });
        }
    }

    onScroll(event) {
        const direction = event.get_scroll_direction();

        if (direction === Clutter.ScrollDirection.UP) {
            this.scrolled += STEP;
        } else if (direction === Clutter.ScrollDirection.DOWN) {
            this.scrolled -= STEP;
        } else {
            return Clutter.EVENT_PROPAGATE;
        }

        this.later("scroll", () => {
            const delta = this.scrolled;
            this.scrolled = 0;

            if (delta === 0) {
                return null;
            }

            return delta > 0 ? ["up", "all", String(delta)] : ["down", "all", String(-delta)];
        });

        return Clutter.EVENT_STOP;
    }

    // Run `args` once input on `key` has been quiet for QUIET_MS; `args`
    // may be a function giving them then, or null for nothing to do.
    later(key, args) {
        if (this.timers.has(key)) {
            GLib.source_remove(this.timers.get(key));
        }

        this.timers.set(key, GLib.timeout_add(GLib.PRIORITY_DEFAULT, QUIET_MS, () => {
            this.timers.delete(key);
            const now = typeof args === "function" ? args() : args;

            if (now) {
                this.enqueue(now, () => {});
            }

            return GLib.SOURCE_REMOVE;
        }));
    }

    enqueue(args, done) {
        this.queue.push([args, done]);
        this.next();
    }

    next() {
        if (this.running || this.queue.length === 0) {
            return;
        }

        const [args, done] = this.queue.shift();
        this.running = true;

        this.run(args, (...result) => {
            this.running = false;
            done(...result);
            this.next();
        });
    }

    run(args, done) {
        try {
            const proc = Gio.Subprocess.new(
                [this.lights, ...args],
                Gio.SubprocessFlags.STDOUT_PIPE | Gio.SubprocessFlags.STDERR_PIPE,
            );

            proc.communicate_utf8_async(null, null, (p, result) => {
                try {
                    const [, out, err] = p.communicate_utf8_finish(result);
                    done(p.get_successful(), out || "", err || "");
                } catch (e) {
                    done(false, "", e.message);
                }
            });
        } catch (e) {
            done(false, "", `cannot run ${this.lights}: ${e.message}`);
        }
    }

    spawn(args) {
        try {
            Gio.Subprocess.new([this.lights, ...args], Gio.SubprocessFlags.NONE);
        } catch (e) {
            global.logError(`lights: ${e.message}`);
        }
    }
}

function main(metadata, orientation, panelHeight, instanceId) {
    return new LightsApplet(metadata, orientation, panelHeight, instanceId);
}
