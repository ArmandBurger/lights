# lights

A brightness controller for monitors, driven over `ddcutil` (VCP code
`0x10`, 0–100 scale). One Rust binary provides a small CLI, an interactive
terminal UI (ratatui) and a GTK 4 desktop UI, both fully keyboard-driven with
vim-style keys.

## Installation

Runtime requirements: `ddcutil` (≥ 1.4.1) with your monitors reachable on
their I2C/EDID DDC bus (e.g. `/dev/i2c-8`, `/dev/i2c-10`), and GTK 4 for the
desktop UI (preinstalled on GNOME, Cinnamon and most desktops).

Building needs Rust (≥ 1.92, e.g. via [rustup](https://rustup.rs)) and the
GTK 4 development files:

```bash
sudo apt install ddcutil libgtk-4-dev build-essential   # Debian / Ubuntu / Mint
cargo install --locked --path crates/lights
```

This installs `lights` and `lights-gui` into `~/.cargo/bin`. `--locked`
builds exactly the dependency versions recorded in `Cargo.lock` (direct
dependencies are also pinned to exact versions in `Cargo.toml`); without it,
`cargo install` would resolve newer ones. The icons and
licence texts are compiled into the binary, so it can be copied anywhere.
For a terminal-only build without GTK:

```bash
cargo install --locked --path crates/lights --no-default-features --features tui
```

## CLI

```bash
lights --help
```

| Command | Description |
|---------|-------------|
| `lights detect` | Raw `ddcutil detect` output (for debugging). |
| `lights info`   | Parsed list of detected displays (index, model, i2c bus, status). |
| `lights get [target] [--json]` | Show current brightness of one or all displays. |
| `lights set <target> <percent>` | Set brightness (0–100) on a display. |
| `lights up <target> [step]`    | Raise brightness by `step` (default 10). |
| `lights down <target> [step]`  | Lower brightness by `step` (default 10). |
| `lights app`     | Launch the interactive TUI. |
| `lights gui`     | Launch the desktop UI (also `lights-gui`). |

### Targeting displays

`target` can be:

- **numeric index** — `1`, `2` — the `Display N` number from `detect`
  (re-detected on every command; numbering can shift if a link drops).
- **keyword** — any substring of a display's *model* or *manufacturer*,
  case-insensitive. For the current setup: `dell` (→ DELL SE2219H),
  `msi` / `mag` (→ MAG 276CXF).
- **`all`** (default) or `*` — every addressable display.

Examples:

```bash
lights get                     # both
lights get --json              # machine-readable
lights get dell
lights get 2
lights set dell 70
lights up mag 15
lights down 1 5
```

An unknown target prints a clean `error: unknown target '...'` (exit 1).

## Desktop UI (`lights gui`) and TUI (`lights app`)

Both show each monitor as a screen, side by side, with its name and info
underneath (model, brightness, display number, connector, I2C bus), and
share the same behaviour and keys.

**Layout:** monitors are arranged in the most square grid that fits them,
up to 4×4 — 1 → 1×1, 2 → 2×1, 3–4 → 2×2, 5–6 → 3×2, 7–9 → 3×3,
10–12 → 4×3, 13–16 → 4×4. Beyond 16, the first 16 are shown. The desktop
window asks for a size that fits the grid (capped to 90% of the screen) and
the screens scale with the window; in the TUI they scale with the terminal,
switching to smaller − / + glyphs and a one-line licence notice when space
is tight.

**Mouse:** click the left half of a screen (**−**) or the right half (**+**)
to step its brightness by **5**. Off-grid values settle on the next multiple
of 5 first: 74 → 75 → 80 going up, 74 → 70 → 65 going down. Clicking a
screen also selects it.

**Lock:** the *Unlocked / Locked* button (or `s`) syncs every monitor to the
selected monitor's brightness. While locked, every change — mouse or
keyboard — applies the same value to all monitors.

Values refresh automatically every 1.5 s in the background. Input updates
the screen immediately; the monitor is written once input on it has been
quiet for 300 ms (debounce), so a burst of input costs a single ddcutil
call. Pending changes are still written if you quit within that window.

To try either UI without touching real monitors, set
`LIGHTS_FAKE_MONITORS=N` to show N simulated monitors.

**Keys** (vim-style; a count repeats a motion, as in vim):

| Key | Action |
|-----|--------|
| `h` / `l` (`←` / `→`) | Select the previous / next monitor (wraps). |
| `k` / `j` (`↑` / `↓`) | Brighter / dimmer by **5** (snaps to multiples of 5). |
| `K` / `J` | Brighter / dimmer by **10**. |
| `3k`, `2J`, … | Repeat a step. |
| `70%` | Set brightness to 70%. |
| `gg` / `G` | 100% / 0% (`40gg` also sets 40%). |
| `s` | Toggle the lock. |
| `r` | Re-detect monitors and refresh now. |
| `?` | Show the keys. |
| `q` | Quit. |
| `Esc` | Cancel a pending count. |

Both UIs show a pending count at the bottom right (like vim's showcmd).
The desktop UI's About dialog (ⓘ) shows the copyright, licence and
third-party notices. In the TUI, `w` / `c` / `n` show the warranty, licence
and third-party notices.

```bash
lights gui   # desktop UI (or lights-gui)
lights app   # TUI, from a real terminal
```

## Notes

- Brightness uses the standard **VCP code `0x10`** ("Brightness", 0–100).
  Some panels also expose a manufacturer-specific code (e.g. `0xE2`); this
  project deliberately sticks to the standard code for portability.
- `ddcutil` is flaky under load: reads of `getvcp 0x10` are retried (3×, 0.25 s)
  to handle transient "Maximum retries exceeded" responses, and failed
  writes are retried (2×, 0.25 s) since monitors occasionally drop one.
- Display **indexing is recomputed on every CLI command** (via `ddcutil detect`)
  so a monitor that reconnects is always addressable by its current number.
  The TUI and desktop UI detect once at startup and again on `r`.
- After detection, monitors are addressed by I2C bus (`ddcutil --bus N`)
  rather than `--display N`, which would make ddcutil re-probe every bus on
  each call (~0.75 s vs ~0.35 s per call). Commands on several displays run
  in parallel. Calls on the same bus are serialised, since overlapping
  calls make the monitor drop commands.

## File layout

A Cargo workspace:

```
lights/
├── Cargo.toml               # workspace
└── crates/
    ├── lights-core/         # shared library
    │   └── src/
    │       ├── brightness.rs  # get/set via ddcutil (VCP 0x10), retries, bus locks
    │       ├── monitors.rs    # `ddcutil detect` parser
    │       ├── backend.rs     # ddcutil or fake monitors
    │       ├── controller.rs  # shared state: debounce, lock, polling, writers
    │       ├── keys.rs        # vim-style key handling shared by both UIs
    │       └── legal.rs       # copyright, licence and notice texts
    ├── lights-tui/          # ratatui + crossterm terminal UI
    ├── lights-gui/          # GTK 4 desktop UI
    │   └── assets/          # style.css and Fluent UI System Icons (MIT), compiled in
    └── lights/              # the `lights` and `lights-gui` binaries (clap CLI)
```

Run the tests with `cargo test --locked --workspace`.

## Roadmap

Done: brightness CLI, interactive TUI and GTK 4 desktop UI, in Rust.

Planned: a Cinnamon panel applet with desktop shortcuts.

## License

Copyright (C) 2026 Armand Burger

This program is free software: you can redistribute it and/or modify it
under the terms of the GNU General Public License as published by the Free
Software Foundation, either version 3 of the License, or (at your option)
any later version. See [LICENSE](LICENSE) for the full text.

This program is distributed in the hope that it will be useful, but
WITHOUT ANY WARRANTY; without even the implied warranty of MERCHANTABILITY
or FITNESS FOR A PARTICULAR PURPOSE.

`lights show w` prints the warranty disclaimer, `lights show c` the license
conditions and `lights show n` the third-party notices; in the TUI, press
`w` / `c` / `n`. `lights --version` shows the copyright notice.

### Third-party material

The icons in `crates/lights-gui/assets/icons/` are from
[Fluent UI System Icons](https://github.com/microsoft/fluentui-system-icons),
Copyright (c) 2020 Microsoft Corporation, used under the MIT License
(recoloured). See [THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md) and
[crates/lights-gui/assets/icons/LICENSE-fluentui-system-icons](crates/lights-gui/assets/icons/LICENSE-fluentui-system-icons).
`lights` is not affiliated with or endorsed by Microsoft.

Contact: [@ArmandBurger](https://github.com/ArmandBurger) on GitHub, [@ghastly_dev](https://x.com/ghastly_dev) on Twitter/X.

`lights` runs [ddcutil](https://www.ddcutil.com/) (GPL-2.0-or-later) as a
separate program; it is not bundled.
