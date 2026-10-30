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

//! Running the `ddcutil` program.

use std::collections::HashMap;
use std::fmt;
use std::process::Command;
use std::sync::{Arc, LazyLock, Mutex, PoisonError};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DdcError(pub String);

impl fmt::Display for DdcError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for DdcError {}

/// Run ddcutil and return its stdout and stderr, trimmed.
pub(crate) fn run(args: &[&str]) -> Result<String, DdcError> {
    let output = Command::new("ddcutil")
        .args(args)
        .output()
        .map_err(|e| DdcError(format!("cannot run ddcutil ({e}); is ddcutil installed?")))?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    Ok(format!("{stdout}\n{stderr}").trim().to_owned())
}

// One ddcutil call per bus at a time: overlapping calls on the same bus
// (e.g. a background read during a write) make the monitor drop commands.
static BUS_LOCKS: LazyLock<Mutex<HashMap<u32, Arc<Mutex<()>>>>> = LazyLock::new(Mutex::default);

/// Run ddcutil against one I2C bus (`ddcutil --bus N ...`).
pub(crate) fn run_on_bus(bus: u32, args: &[&str]) -> Result<String, DdcError> {
    let bus_arg = bus.to_string();
    let mut full = vec!["--bus", bus_arg.as_str()];
    full.extend_from_slice(args);
    let lock = BUS_LOCKS
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .entry(bus)
        .or_default()
        .clone();
    let _guard = lock.lock().unwrap_or_else(PoisonError::into_inner);
    run(&full)
}
