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

mod fake;

use std::sync::Arc;

use crate::brightness;
use crate::ddcutil::DdcError;
use crate::monitors::{self, Display};

pub use fake::Fake;

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

/// Fake monitors if `LIGHTS_FAKE_MONITORS` is set, else ddcutil.
pub fn from_env() -> Arc<dyn Backend> {
    match Fake::from_env() {
        Some(fake) => Arc::new(fake),
        None => Arc::new(Ddcutil),
    }
}
