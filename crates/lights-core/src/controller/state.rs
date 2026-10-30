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

//! The brightness state behind the controller's mutex.

use std::collections::{HashMap, HashSet};

use crate::monitors::Display;

/// Bus -> brightness (`None`: unreadable).
pub(super) type Readings = HashMap<u32, Option<i32>>;

#[derive(Default)]
pub(super) struct State {
    /// Addressable displays only.
    pub displays: Vec<Display>,
    pub detected: bool,
    pub layout: u64,
    pub selected: usize,
    pub locked: bool,
    pub values: Readings,
    /// Targets waiting to be written.
    pub pending: HashMap<u32, i32>,
    /// Buses whose writer is running.
    pub writing: HashSet<u32>,
    /// Buses whose debounce is running. A debounce must still hold its
    /// bus's token when it elapses, so one replaced by newer input does
    /// nothing.
    pub timers: HashSet<u32>,
    pub tokens: HashMap<u32, u64>,
    /// Bumped per completed write so reads that started earlier are dropped.
    pub write_gen: HashMap<u32, u64>,
}

pub(super) fn bus_of(d: &Display) -> u32 {
    d.bus().expect("only addressable displays are kept")
}

impl State {
    pub fn shown(&self, bus: u32) -> Option<i32> {
        self.pending
            .get(&bus)
            .copied()
            .or_else(|| self.values.get(&bus).copied().flatten())
    }

    pub fn buses(&self) -> Vec<u32> {
        self.displays.iter().map(bus_of).collect()
    }

    /// The selected monitor's bus; there must be at least one monitor.
    pub fn selected_bus(&self) -> u32 {
        bus_of(&self.displays[self.selected])
    }

    pub fn busy(&self, bus: u32) -> bool {
        self.pending.contains_key(&bus) || self.writing.contains(&bus)
    }

    pub fn write_gen(&self, bus: u32) -> u64 {
        self.write_gen.get(&bus).copied().unwrap_or(0)
    }

    pub fn label(&self, bus: u32) -> String {
        self.displays
            .iter()
            .find(|d| d.bus() == Some(bus))
            .map_or_else(|| "?".to_owned(), Display::label)
    }

    /// Readings, unless the bus has a write queued or running.
    pub fn apply_reads(&mut self, values: Readings) {
        for (bus, value) in values {
            if !self.busy(bus) {
                self.values.insert(bus, value);
            }
        }
    }
}
