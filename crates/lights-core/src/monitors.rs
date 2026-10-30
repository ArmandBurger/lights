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

//! Monitors as reported by `ddcutil detect`.
//!
//! The `--display N` index accepted by ddcutil equals the `Display N` label
//! printed by `ddcutil detect` for every monitor whose DDC link is currently
//! up. A monitor whose DDC link is down prints `Invalid display` (no number)
//! and is not addressable.
//!
//! Detection is slow (every bus is probed), so callers detect once and then
//! address each monitor by its I2C bus (`ddcutil --bus N`), which skips that
//! probe and is roughly twice as fast per call.

use crate::brightness::{DdcError, ddcutil};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Display {
    pub index: Option<u32>,
    pub i2c_bus: String,
    pub connector: String,
    pub mfg: String,
    pub model: String,
    pub ddc_failed: bool,
}

impl Display {
    /// Bus number for `ddcutil --bus`, e.g. 8 for `/dev/i2c-8`.
    pub fn bus(&self) -> Option<u32> {
        let (_, number) = self.i2c_bus.rsplit_once("i2c-")?;
        number.parse().ok()
    }

    pub fn addressable(&self) -> bool {
        self.index.is_some() && self.bus().is_some() && !self.ddc_failed
    }

    pub fn label(&self) -> String {
        self.index.map_or_else(|| "?".to_owned(), |i| i.to_string())
    }

    /// The bus name without `/dev/`, e.g. `i2c-8`.
    pub fn bus_name(&self) -> &str {
        self.i2c_bus.strip_prefix("/dev/").unwrap_or(&self.i2c_bus)
    }
}

pub fn detect_raw() -> Result<String, DdcError> {
    ddcutil(&["detect"])
}

pub fn detect() -> Result<Vec<Display>, DdcError> {
    Ok(parse(&detect_raw()?))
}

/// The text after `key` and at least one space, if the line contains `key`.
fn field<'a>(line: &'a str, key: &str) -> Option<&'a str> {
    let (_, rest) = line.split_once(key)?;
    rest.starts_with(char::is_whitespace).then(|| rest.trim())
}

/// Parse the output of `ddcutil detect`.
pub fn parse(text: &str) -> Vec<Display> {
    let mut displays: Vec<Display> = Vec::new();
    for line in text.lines() {
        let s = line.trim();
        if let Some(rest) = s.strip_prefix("Display ") {
            let digits: String = rest.chars().take_while(char::is_ascii_digit).collect();
            if let Ok(index) = digits.parse() {
                displays.push(Display {
                    index: Some(index),
                    ..Display::default()
                });
                continue;
            }
        }
        if s.starts_with("Invalid display") {
            displays.push(Display {
                ddc_failed: true,
                ..Display::default()
            });
            continue;
        }
        let Some(current) = displays.last_mut() else {
            continue;
        };
        if let Some(v) = field(s, "I2C bus:") {
            current.i2c_bus = v.split_whitespace().next().unwrap_or_default().to_owned();
        } else if let Some(v) = field(s, "DRM connector:") {
            current.connector = v.split_whitespace().next().unwrap_or_default().to_owned();
        } else if let Some(v) = field(s, "Mfg id:") {
            current.mfg = v.to_owned();
        } else if let Some(v) = field(s, "Model:") {
            current.model = v.to_owned();
        } else if s.contains("DDC communication failed") {
            current.ddc_failed = true;
        }
    }
    displays
}

#[cfg(test)]
mod tests {
    use super::*;

    const DETECT: &str = "\
Display 1
   I2C bus:  /dev/i2c-8
   DRM connector:           card1-HDMI-A-2
   EDID synopsis:
      Mfg id:               DEL - Dell Inc.
      Model:                DELL SE2219H
      Product code:         61711  (0xf10f)
   VCP version:         2.1

Invalid display
   I2C bus:  /dev/i2c-5
   DRM connector:           card1-DP-1
   DDC communication failed

Display 2
   I2C bus:  /dev/i2c-10
   DRM connector:           card1-DP-2
   EDID synopsis:
      Mfg id:               MSI - Microstep
      Model:                MAG 276CXF
   VCP version:         2.1
";

    #[test]
    fn parses_detect_output() {
        let displays = parse(DETECT);
        assert_eq!(displays.len(), 3);

        let dell = &displays[0];
        assert_eq!(dell.index, Some(1));
        assert_eq!(dell.bus(), Some(8));
        assert_eq!(dell.bus_name(), "i2c-8");
        assert_eq!(dell.connector, "card1-HDMI-A-2");
        assert_eq!(dell.mfg, "DEL - Dell Inc.");
        assert_eq!(dell.model, "DELL SE2219H");
        assert!(dell.addressable());

        let down = &displays[1];
        assert_eq!(down.index, None);
        assert_eq!(down.label(), "?");
        assert!(!down.addressable());

        assert_eq!(displays[2].model, "MAG 276CXF");
        assert_eq!(displays[2].bus(), Some(10));
    }
}
