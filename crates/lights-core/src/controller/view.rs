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

//! What a UI reads from the controller to draw itself.

use crate::monitors::Display;

#[derive(Debug, Clone)]
pub struct MonitorView {
    pub display: Display,
    pub bus: u32,
    /// Pending target if any, else the last read.
    pub value: Option<i32>,
    /// A change is waiting to be written or being written.
    pub busy: bool,
    pub selected: bool,
}

impl MonitorView {
    /// e.g. `75%`, `80% …` while a change is being written, or `n/a`.
    pub fn level_text(&self) -> String {
        let busy = if self.busy { " …" } else { "" };

        match self.value {
            Some(value) => format!("{value}%{busy}"),
            None => format!("n/a{busy}"),
        }
    }
}

#[derive(Debug, Clone)]
pub struct View {
    pub detected: bool,
    pub locked: bool,
    /// Changes whenever the set of monitors changes.
    pub layout: u64,
    pub monitors: Vec<MonitorView>,
}

impl View {
    /// Linked monitors that are not all at the same brightness.
    pub fn can_sync(&self) -> bool {
        self.locked && self.monitors.windows(2).any(|w| w[0].value != w[1].value)
    }

    /// What to show instead of the monitors, while there are none.
    pub fn status(&self) -> Option<&'static str> {
        if !self.monitors.is_empty() {
            None
        } else if self.detected {
            Some("no monitors responding  (r to retry)")
        } else {
            Some("detecting monitors…")
        }
    }
}
