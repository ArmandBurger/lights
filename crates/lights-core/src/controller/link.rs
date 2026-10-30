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

//! Linked monitors move together and keep their differences in brightness.

use std::collections::HashMap;

use crate::brightness;

pub(super) struct Link {
    /// The reference brightness. It may lie past 0 or 100, so a monitor
    /// pushed against a limit gets its offset back on the way out.
    level: i32,
    /// Bus -> brightness relative to `level`.
    offsets: HashMap<u32, i32>,
}

impl Link {
    /// Link monitors at their current `values`, measured from `level`.
    pub fn new(values: impl IntoIterator<Item = (u32, i32)>, level: i32) -> Self {
        Self {
            level,
            offsets: values
                .into_iter()
                .map(|(bus, value)| (bus, value - level))
                .collect(),
        }
    }

    fn offset(&self, bus: u32) -> i32 {
        self.offsets.get(&bus).copied().unwrap_or(0)
    }

    /// `bus`'s brightness before clamping to 0-100.
    pub fn unclamped(&self, bus: u32) -> i32 {
        self.level + self.offset(bus)
    }

    /// Move everything so `bus` is at `value`, stopping once every monitor
    /// is at 0 (or 100).
    pub fn move_to(&mut self, bus: u32, value: i32) {
        let lowest = -self.offsets.values().copied().max().unwrap_or(0);
        let highest = 100 - self.offsets.values().copied().min().unwrap_or(0);
        self.level = (value - self.offset(bus)).clamp(lowest, highest);
    }

    /// Bus -> brightness to show.
    pub fn targets(&self) -> Vec<(u32, i32)> {
        self.offsets
            .iter()
            .map(|(&bus, &offset)| (bus, brightness::clamp(self.level + offset)))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sorted(link: &Link) -> Vec<(u32, i32)> {
        let mut targets = link.targets();
        targets.sort_unstable();
        targets
    }

    #[test]
    fn keeps_offsets_through_the_limits() {
        let mut link = Link::new([(1, 20), (2, 35)], 35);
        link.move_to(2, 100);
        assert_eq!(sorted(&link), [(1, 85), (2, 100)]);

        link.move_to(2, 140);
        assert_eq!(sorted(&link), [(1, 100), (2, 100)]);
        assert_eq!(link.unclamped(2), 115);

        link.move_to(2, 110);
        assert_eq!(sorted(&link), [(1, 95), (2, 100)]);

        link.move_to(1, -50);
        assert_eq!(sorted(&link), [(1, 0), (2, 0)]);
        assert_eq!(link.unclamped(1), -15);
    }
}
