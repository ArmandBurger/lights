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

use std::thread;
use std::time::Duration;

use crate::ddcutil::{self, DdcError};

const VCP_CODE: &str = "0x10"; // Brightness
const RETRY_DELAY: Duration = Duration::from_millis(250);
const READ_RETRIES: u32 = 3;
const WRITE_RETRIES: u32 = 2;

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

/// Step by `delta`, landing on a multiple of `|delta|`; not clamped.
///
/// An off-grid value settles on the next grid point first, so with a step
/// of 5: 74 +5 -> 75, 75 +5 -> 80, 74 -5 -> 70, 70 -5 -> 65.
pub fn snap_step(value: i32, delta: i32) -> i32 {
    let step = delta.abs();

    if delta > 0 {
        (value.div_euclid(step) + 1) * step
    } else {
        (-(-value).div_euclid(step) - 1) * step
    }
}

/// The mean of `values` on the grid of `step`, halves rounding up: with a
/// step of 5, 82 and 70 -> 76 -> 75, 82 and 73 -> 77.5 -> 80.
pub fn snapped_mean(values: &[i32], step: i32) -> Option<i32> {
    let count = i32::try_from(values.len()).ok().filter(|&n| n > 0)?;
    let sum: i32 = values.iter().sum();
    // round(sum / count / step) with halves up, in integers.
    let grid = (2 * sum + count * step).div_euclid(2 * count * step);
    Some(clamp(grid * step))
}

pub fn get_value(bus: u32) -> Result<i32, DdcError> {
    let mut text = ddcutil::run_on_bus(bus, &["getvcp", VCP_CODE])?;

    for _ in 0..READ_RETRIES {
        if parse_current(&text).is_some() {
            break;
        }

        thread::sleep(RETRY_DELAY);
        text = ddcutil::run_on_bus(bus, &["getvcp", VCP_CODE])?;
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

        text = ddcutil::run_on_bus(bus, &["--verify", "setvcp", VCP_CODE, &value_arg])?;

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
        assert_eq!(snap_step(100, 5), 105);
        assert_eq!(snap_step(3, -5), 0);
        assert_eq!(snap_step(-3, 5), 0);
        assert_eq!(snap_step(0, -5), -5);
        assert_eq!(snap_step(33, 10), 40);
        assert_eq!(snap_step(33, -10), 30);
    }

    #[test]
    fn averages_onto_the_grid() {
        assert_eq!(snapped_mean(&[82, 70], 5), Some(75));
        assert_eq!(snapped_mean(&[82, 73], 5), Some(80));
        assert_eq!(snapped_mean(&[77], 5), Some(75));
        assert_eq!(snapped_mean(&[78], 5), Some(80));
        assert_eq!(snapped_mean(&[20, 35, 41], 5), Some(30));
        assert_eq!(snapped_mean(&[], 5), None);
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
