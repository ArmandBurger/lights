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

//! The large minus and plus drawn on each half of a screen.
//!
//! Terminal cells are about twice as tall as wide, so glyphs are drawn wide.
//! Each half uses the largest glyph that fits.

use ratatui::layout::Rect;

pub(crate) struct Glyph {
    min_width: u16,
    min_height: u16,
    pub rows: &'static [&'static str],
}

impl Glyph {
    pub const PLUS: [Self; 3] = [
        Self {
            min_width: 12,
            min_height: 5,
            rows: &[
                "    ██    ",
                "    ██    ",
                "██████████",
                "    ██    ",
                "    ██    ",
            ],
        },
        Self {
            min_width: 10,
            min_height: 3,
            rows: &["   ██   ", "████████", "   ██   "],
        },
        Self {
            min_width: 0,
            min_height: 0,
            rows: &["+"],
        },
    ];

    pub const MINUS: [Self; 3] = [
        Self {
            min_width: 12,
            min_height: 5,
            rows: &["██████████"],
        },
        Self {
            min_width: 10,
            min_height: 3,
            rows: &["████████"],
        },
        Self {
            min_width: 0,
            min_height: 0,
            rows: &["−"],
        },
    ];

    /// The largest of `sizes` (largest first) that fits in `area`.
    pub fn fit(sizes: &[Self], area: Rect) -> &Self {
        sizes
            .iter()
            .find(|g| area.width >= g.min_width && area.height >= g.min_height)
            .unwrap_or(&sizes[sizes.len() - 1])
    }
}
