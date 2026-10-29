# lights

A self-contained Python brightness controller for two monitors, driven over
`ddcutil` (VCP code `0x10`, 0–100 scale). Provides a small CLI and an
interactive Textual TUI.

## Installation

Requires Python ≥ 3.10 and the `lights` entry point, with a single dependency
(`textual`). Manage with `uv`:

```bash
cd lights
uv sync
# or, for a fresh venv:
uv venv && uv sync
```

After `uv sync`, the console command `lights` is on the path inside the venv
(activated via `uv run lights ...` or `.venv/bin/lights`).

Requires `ddcutil` (≥ 1.4.1) installed and your monitors reachable on their
I2C/EDID DDC bus (e.g. `/dev/i2c-8`, `/dev/i2c-10`).

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

## TUI (`lights app`)

Each monitor is drawn as a screen, side by side, with its name and info
underneath (model, brightness, display number, connector, I2C bus).

**Mouse:** click the left half of a screen (**−**) or the right half (**+**)
to step its brightness by **5**. Off-grid values settle on the next multiple
of 5 first: 74 → 75 → 80 going up, 74 → 70 → 65 going down. Clicking a
screen also selects it.

**Lock:** the *Unlocked / Locked* button at the top (or `l`) syncs every
monitor to the selected monitor's brightness. While locked, every step —
mouse or keyboard — applies the same value to all monitors.

Values refresh automatically every 1.5 s in the background. Input updates
the screen immediately; the monitor is written once input on it has been
quiet for 500 ms (debounce), so a burst of clicks costs a single ddcutil
call. Pending changes are still written if you quit within that window.

| Key | Action |
|-----|--------|
| `←` / `→` (or `↑` / `↓`) | Select the previous / next monitor (wraps). |
| `j` | Raise the selected monitor by **10**. |
| `k` | Lower the selected monitor by **10**. |
| `shift+j` | Raise the selected monitor by **15**. |
| `shift+k` | Lower the selected monitor by **15**. |
| `l` | Toggle the lock. |
| `r` | Re-detect monitors and refresh now. |
| `q` / `Enter` | Quit. |

Run it from a real terminal (e.g. a Cinnamon terminal):

```bash
uv run lights app   # or: .venv/bin/lights app
```

## Notes

- Brightness uses the standard **VCP code `0x10`** ("Brightness", 0–100).
  Some panels also expose a manufacturer-specific code (e.g. `0xE2`); this
  project deliberately sticks to the standard code for portability.
- `ddcutil` is flaky under load: reads of `getvcp 0x10` are retried (3×, 0.25 s)
  to handle transient "Maximum retries exceeded" responses.
- Display **indexing is recomputed on every CLI command** (via `ddcutil detect`)
  so a monitor that reconnects is always addressable by its current number.
  The TUI detects once at startup and again on `r`.
- After detection, monitors are addressed by I2C bus (`ddcutil --bus N`)
  rather than `--display N`, which would make ddcutil re-probe every bus on
  each call (~0.75 s vs ~0.35 s per call). Commands on several displays run
  in parallel. Calls on the same bus are serialised, since overlapping
  calls make the monitor drop commands.

## File layout

```
lights/
├── pyproject.toml
├── uv.lock
└── src/lights/
    ├── __init__.py      # package (version 0.1.0)
    ├── brightness.py    # get/set/bump via ddcutil (VCP 0x10)
    ├── monitors.py      # `ddcutil detect` parser
    ├── cli.py           # argparse subcommands + target resolution
    ├── app.py           # Textual TUI
    └── widgets.py       # clickable monitor screens for the TUI
```

## Roadmap

Phase 1 (this release): brightness-only CLI + interactive TUI.

Planned: a general (non-terminal) UI, and a Cinnamon panel applet with
desktop shortcuts.

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

The icons in `assets/` are from
[Fluent UI System Icons](https://github.com/microsoft/fluentui-system-icons),
Copyright (c) 2020 Microsoft Corporation, used under the MIT License
(recoloured). See [THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md) and
[assets/LICENSE-fluentui-system-icons](assets/LICENSE-fluentui-system-icons).
`lights` is not affiliated with or endorsed by Microsoft.

Contact: [@ArmandBurger](https://github.com/ArmandBurger) on GitHub, [@ghastly_dev](https://x.com/ghastly_dev) on Twitter/X.

`lights` runs [ddcutil](https://www.ddcutil.com/) (GPL-2.0-or-later) as a
separate program; it is not bundled.
