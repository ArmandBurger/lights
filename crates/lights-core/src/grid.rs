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

//! How monitors are arranged on screen in both UIs.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Grid {
    pub columns: usize,
    pub rows: usize,
}

impl Grid {
    /// The largest grid shown; further monitors are left out.
    pub const MAX: Self = Self {
        columns: 4,
        rows: 4,
    };

    /// The most square grid holding `count` monitors.
    ///
    /// 1 -> 1x1, 2 -> 2x1, 3-4 -> 2x2, 5-6 -> 3x2, 7-9 -> 3x3, 10-12 -> 4x3,
    /// 13-16 -> 4x4.
    pub fn fit(count: usize) -> Self {
        let count = count.clamp(1, Self::MAX.cells());
        let mut columns = 1;

        while columns * columns < count {
            columns += 1;
        }

        Self {
            columns,
            rows: count.div_ceil(columns),
        }
    }

    pub const fn cells(self) -> usize {
        self.columns * self.rows
    }

    /// (column, row) of the `index`th monitor.
    pub const fn position(self, index: usize) -> (usize, usize) {
        (index % self.columns, index / self.columns)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn grid_is_most_square() {
        let shapes: Vec<_> = [1, 2, 3, 4, 5, 6, 7, 9, 10, 12, 13, 16, 17]
            .map(|n| {
                let grid = Grid::fit(n);
                (grid.columns, grid.rows)
            })
            .into();
        assert_eq!(
            shapes,
            [
                (1, 1),
                (2, 1),
                (2, 2),
                (2, 2),
                (3, 2),
                (3, 2),
                (3, 3),
                (3, 3),
                (4, 3),
                (4, 3),
                (4, 4),
                (4, 4),
                (4, 4)
            ]
        );
    }
}
