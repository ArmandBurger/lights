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

//! Brightness control (VCP 0x10) over ddcutil.
//!
//! Displays are addressed by I2C bus number (see `Display::bus`) rather than
//! by `--display N`, which would make ddcutil re-probe every bus per call.

use std::collections::HashMap;
use std::fmt;
use std::process::Command;
use std::sync::{Arc, LazyLock, Mutex, PoisonError};
use std::thread;
use std::time::Duration;

pub const VCP_CODE: &str = "0x10"; // Brightness
pub const DEFAULT_STEP: i32 = 10;
const RETRY_DELAY: Duration = Duration::from_millis(250);
const READ_RETRIES: u32 = 3;
const WRITE_RETRIES: u32 = 2;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DdcError(pub String);

impl fmt::Display for DdcError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for DdcError {}

/// Run ddcutil and return its stdout and stderr, trimmed.
pub(crate) fn ddcutil(args: &[&str]) -> Result<String, DdcError> {
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

fn bus_lock(bus: u32) -> Arc<Mutex<()>> {
    let mut locks = BUS_LOCKS.lock().unwrap_or_else(PoisonError::into_inner);
    locks.entry(bus).or_default().clone()
}

fn run(args: &[&str], bus: u32) -> Result<String, DdcError> {
    let bus_arg = bus.to_string();
    let mut full = vec!["--bus", bus_arg.as_str()];
    full.extend_from_slice(args);
    let lock = bus_lock(bus);
    let _guard = lock.lock().unwrap_or_else(PoisonError::into_inner);
    ddcutil(&full)
}

const ERROR_MARKERS: [&str; 5] = [
    "failed",
    "not found",
    "unrecognized",
    "unknown option",
    "invalid",
];

fn set_ok(text: &str) -> bool {
    let low = text.to_lowercase();
    !ERROR_MARKERS.iter().any(|m| low.contains(m))
}

/// The current value from `getvcp` output: `current value =   75, max ...`.
fn parse_current(text: &str) -> Option<i32> {
    let (_, rest) = text.split_once("current value")?;
    let rest = rest.trim_start().strip_prefix('=')?.trim_start();
    let digits: String = rest.chars().take_while(char::is_ascii_digit).collect();
    let after = rest[digits.len()..].trim_start();
    after.starts_with(',').then_some(())?;
    digits.parse().ok()
}

pub fn clamp(value: i32) -> i32 {
    value.clamp(0, 100)
}

/// Step by `delta`, landing on a multiple of `|delta|`.
///
/// An off-grid value settles on the next grid point first, so with a step
/// of 5: 74 +5 -> 75, 75 +5 -> 80, 74 -5 -> 70, 70 -5 -> 65.
pub fn snap_step(value: i32, delta: i32) -> i32 {
    let step = delta.abs();
    if delta > 0 {
        clamp((value.div_euclid(step) + 1) * step)
    } else {
        clamp((-(-value).div_euclid(step) - 1) * step)
    }
}

pub fn get_value(bus: u32) -> Result<i32, DdcError> {
    let mut text = run(&["getvcp", VCP_CODE], bus)?;
    for _ in 0..READ_RETRIES {
        if parse_current(&text).is_some() {
            break;
        }
        thread::sleep(RETRY_DELAY);
        text = run(&["getvcp", VCP_CODE], bus)?;
    }
    parse_current(&text)
        .ok_or_else(|| DdcError(format!("cannot read brightness on bus {bus}: {text:?}")))
}

/// Set brightness and return the value written.
///
/// ddcutil verifies the write itself and reports a mismatch as an error, so
/// there is no separate read-back. Monitors occasionally drop a write, so
/// failures are retried like reads.
pub fn set_value(bus: u32, value: i32) -> Result<i32, DdcError> {
    let value = clamp(value);
    let value_arg = value.to_string();
    let mut text = String::new();
    for attempt in 0..=WRITE_RETRIES {
        if attempt > 0 {
            thread::sleep(RETRY_DELAY);
        }
        text = run(&["--verify", "setvcp", VCP_CODE, &value_arg], bus)?;
        if set_ok(&text) {
            return Ok(value);
        }
    }
    Err(DdcError(format!(
        "cannot set brightness on bus {bus}: {text:?}"
    )))
}

pub fn bump_value(bus: u32, delta: i32) -> Result<i32, DdcError> {
    set_value(bus, get_value(bus)? + delta)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snaps_to_the_step_grid() {
        assert_eq!(snap_step(74, 5), 75);
        assert_eq!(snap_step(75, 5), 80);
        assert_eq!(snap_step(74, -5), 70);
        assert_eq!(snap_step(70, -5), 65);
        assert_eq!(snap_step(98, 5), 100);
        assert_eq!(snap_step(100, 5), 100);
        assert_eq!(snap_step(3, -5), 0);
        assert_eq!(snap_step(0, -5), 0);
        assert_eq!(snap_step(33, 10), 40);
        assert_eq!(snap_step(33, -10), 30);
    }

    #[test]
    fn parses_getvcp_output() {
        let line = "VCP code 0x10 (Brightness                    ): current value =    75, max value =   100";
        assert_eq!(parse_current(line), Some(75));
        assert_eq!(parse_current("Maximum retries exceeded"), None);
    }

    #[test]
    fn detects_write_errors() {
        assert!(set_ok(""));
        assert!(!set_ok("Verification failed for feature 10"));
    }
}
