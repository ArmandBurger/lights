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

//! Where the controller gets monitors and brightness from: ddcutil, or a set
//! of fake monitors for trying the UIs without touching real ones.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, PoisonError};
use std::thread;
use std::time::Duration;

use crate::brightness::{self, DdcError};
use crate::monitors::{self, Display};

pub trait Backend: Send + Sync {
    fn detect(&self) -> Result<Vec<Display>, DdcError>;
    fn get(&self, bus: u32) -> Result<i32, DdcError>;
    fn set(&self, bus: u32, value: i32) -> Result<i32, DdcError>;
}

/// Real monitors, through ddcutil.
pub struct Ddcutil;

impl Backend for Ddcutil {
    fn detect(&self) -> Result<Vec<Display>, DdcError> {
        monitors::detect()
    }

    fn get(&self, bus: u32) -> Result<i32, DdcError> {
        brightness::get_value(bus)
    }

    fn set(&self, bus: u32, value: i32) -> Result<i32, DdcError> {
        brightness::set_value(bus, value)
    }
}

/// In-memory monitors that answer as slowly as real ones. Set
/// `LIGHTS_FAKE_MONITORS=N` to run the TUI or desktop UI against N of them.
pub struct Fake {
    count: u32,
    delay: Duration,
    values: Mutex<HashMap<u32, i32>>,
}

impl Fake {
    pub const ENV: &str = "LIGHTS_FAKE_MONITORS";

    pub fn new(count: u32, delay: Duration) -> Self {
        let values = (0..count)
            .map(|i| (Self::bus(i), 20 + (i as i32 * 15) % 80))
            .collect();
        Self {
            count,
            delay,
            values: Mutex::new(values),
        }
    }

    /// Fake monitors from `LIGHTS_FAKE_MONITORS`, if set to a number.
    pub fn from_env() -> Option<Self> {
        let count = std::env::var(Self::ENV).ok()?.parse().ok()?;
        Some(Self::new(count, Duration::from_millis(300)))
    }

    fn bus(i: u32) -> u32 {
        100 + i
    }

    /// The value last written to `bus` (for tests).
    pub fn value(&self, bus: u32) -> Option<i32> {
        self.values
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .get(&bus)
            .copied()
    }
}

impl Backend for Fake {
    fn detect(&self) -> Result<Vec<Display>, DdcError> {
        thread::sleep(self.delay);
        Ok((0..self.count)
            .map(|i| Display {
                index: Some(i + 1),
                i2c_bus: format!("/dev/i2c-{}", Self::bus(i)),
                connector: format!("card0-DP-{}", i + 1),
                mfg: "FAK - Fake Monitors".to_owned(),
                model: format!("Fake Monitor {}", i + 1),
                ddc_failed: false,
            })
            .collect())
    }

    fn get(&self, bus: u32) -> Result<i32, DdcError> {
        thread::sleep(self.delay);
        self.value(bus)
            .ok_or_else(|| DdcError(format!("no fake monitor on bus {bus}")))
    }

    fn set(&self, bus: u32, value: i32) -> Result<i32, DdcError> {
        thread::sleep(self.delay);
        let value = brightness::clamp(value);
        self.values
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .insert(bus, value);
        Ok(value)
    }
}

/// Fake monitors if `LIGHTS_FAKE_MONITORS` is set, else ddcutil.
pub fn from_env() -> Arc<dyn Backend> {
    match Fake::from_env() {
        Some(fake) => Arc::new(fake),
        None => Arc::new(Ddcutil),
    }
}
