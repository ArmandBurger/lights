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

//! Clickable areas, recorded while drawing.

use ratatui::layout::{Position, Rect};

/// Something on screen that reacts to the mouse.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Hit {
    Lock,
    About,
    Select(u32),
    Step { bus: u32, steps: i32 },
}

/// The clickable areas of the last draw; later entries are on top.
#[derive(Default)]
pub(crate) struct Hits(Vec<(Rect, Hit)>);

impl Hits {
    pub fn clear(&mut self) {
        self.0.clear();
    }

    pub fn push(&mut self, area: Rect, hit: Hit) {
        self.0.push((area, hit));
    }

    /// The topmost hit at a cell.
    pub fn at(&self, column: u16, row: u16) -> Option<Hit> {
        let at = Position::new(column, row);
        self.0
            .iter()
            .rev()
            .find(|(area, _)| area.contains(at))
            .map(|&(_, hit)| hit)
    }

    #[cfg(test)]
    pub fn area(&self, hit: Hit) -> Option<Rect> {
        self.0
            .iter()
            .find(|(_, h)| *h == hit)
            .map(|&(area, _)| area)
    }
}
